import { afterAll, beforeEach, describe, expect, test } from "vitest";
import { buildApp } from "../src/app.ts";
import { createDb } from "../src/db/client.ts";
import { currentUser } from "../src/lib/authenticate.ts";
import {
  bearer,
  closeTestApp,
  createTestApp,
  expectUnauthorized,
  loginUser,
  registerUser,
  resetDatabase,
} from "./helpers.ts";

const { app, db } = createTestApp();

// Rota de teste SEM o requireAuth, que chama currentUser(). O Fastify aceita rotas novas até o ready (que o
// primeiro inject dispara), então dá para adicioná-la aqui sem alterar o helper nem as rotas reais.
app.get("/teste-sem-hook", async (request) => currentUser(request));

beforeEach(async () => {
  await resetDatabase(db);
});

afterAll(async () => {
  await closeTestApp(app, db);
});

// O requireAuth roda em onRequest: antes de o Fastify ler e interpretar o corpo. Quem não está autenticado
// recebe 401 sem que o corpo seja processado, mesmo quando ele é inválido.
describe("requireAuth roda antes do parse do corpo", () => {
  const malformedJson = "{ruim";

  test("POST /transactions sem token e com JSON malformado responde 401 (e não 400)", async () => {
    const response = await app.inject({
      method: "POST",
      url: "/transactions",
      headers: { "content-type": "application/json" },
      payload: malformedJson,
    });

    expectUnauthorized(response);
  });

  test("POST /auth/logout sem token e com JSON malformado responde 401 (e não 400)", async () => {
    const response = await app.inject({
      method: "POST",
      url: "/auth/logout",
      headers: { "content-type": "application/json" },
      payload: malformedJson,
    });

    expectUnauthorized(response);
  });

  test("com token válido, JSON malformado continua sendo 400 (o 401 só vale para quem não autenticou)", async () => {
    // Arrange
    await registerUser(app);
    const token = await loginUser(app);

    // Act
    const response = await app.inject({
      method: "POST",
      url: "/transactions",
      headers: { "content-type": "application/json", authorization: bearer(token) },
      payload: malformedJson,
    });

    // Assert
    expect(response.statusCode).toBe(400);
  });
});

describe("currentUser", () => {
  test("numa rota sem o requireAuth falha alto: 500 genérico, sem detalhes na resposta", async () => {
    const response = await app.inject({ method: "GET", url: "/teste-sem-hook" });

    expect(response.statusCode).toBe(500);
    expect(response.json()).toEqual({ error: "Erro interno do servidor" });
  });
});

describe("falha do banco dentro do requireAuth", () => {
  test("responde 500 genérico, sem vazar a mensagem do erro do banco", async () => {
    // Arrange: o token é real (gerado pelo app saudável), mas o app testado aponta para uma porta sem banco.
    // A assinatura do token passa; o erro acontece na consulta de token_version, dentro do hook.
    await registerUser(app);
    const token = await loginUser(app);
    // Credenciais fictícias: a senha só existe para o teste conferir que ela não vaza na resposta.
    const brokenDb = createDb("postgres://usuario:senha-secreta@127.0.0.1:1/inexistente");
    const brokenApp = buildApp(brokenDb, { logger: false, rateLimit: false });

    try {
      // Act
      const response = await brokenApp.inject({
        method: "GET",
        url: "/users/me",
        headers: { authorization: bearer(token) },
      });

      // Assert
      expect(response.statusCode).toBe(500);
      expect(response.json()).toEqual({ error: "Erro interno do servidor" });
      expect(response.body).not.toMatch(/ECONNREFUSED|senha-secreta|127\.0\.0\.1/);
    } finally {
      await closeTestApp(brokenApp, brokenDb);
    }
  });
});
