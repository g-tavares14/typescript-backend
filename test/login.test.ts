import { decodeJwt } from "jose";
import { afterAll, beforeEach, describe, expect, test } from "vitest";
import { closeTestApp, createTestApp, defaultUser, registerUser, resetDatabase } from "./helpers.ts";

const { app, db } = createTestApp();

beforeEach(async () => {
  await resetDatabase(db);
});

afterAll(async () => {
  await closeTestApp(app, db);
});

describe("POST /auth/login", () => {
  test("responde 200 com um JWT de 1 hora para o usuário", async () => {
    // Arrange
    const { id } = await registerUser(app);

    // Act
    const response = await app.inject({
      method: "POST",
      url: "/auth/login",
      payload: { email: defaultUser.email, password: defaultUser.password },
    });

    // Assert
    expect(response.statusCode).toBe(200);
    const body = response.json();
    expect(body).toEqual({ token: expect.any(String), tokenType: "Bearer", expiresIn: 3600 });

    // decodeJwt só lê o payload (não verifica a assinatura): serve para conferir o conteúdo.
    const claims = decodeJwt(body.token);
    expect(claims.sub).toBe(id);
    expect(claims.role).toBe("user");
    expect(claims.exp! - claims.iat!).toBe(3600);
    expect(claims).not.toHaveProperty("password");
    expect(claims).not.toHaveProperty("passwordHash");
  });

  test("aceita o email com maiúsculas e espaços (mesma normalização do cadastro)", async () => {
    // Arrange
    await registerUser(app);

    // Act
    const response = await app.inject({
      method: "POST",
      url: "/auth/login",
      payload: { email: "  JOAO@Email.COM ", password: defaultUser.password },
    });

    // Assert
    expect(response.statusCode).toBe(200);
  });

  test("responde 401 quando a senha está errada", async () => {
    // Arrange
    await registerUser(app);

    // Act
    const response = await app.inject({
      method: "POST",
      url: "/auth/login",
      payload: { email: defaultUser.email, password: "senha-errada" },
    });

    // Assert
    expect(response.statusCode).toBe(401);
    expect(response.json()).toEqual({ error: "Email ou senha inválidos" });
  });

  test("responde 401 com a MESMA mensagem quando o email não existe", async () => {
    // Arrange
    await registerUser(app);
    const wrongPassword = await app.inject({
      method: "POST",
      url: "/auth/login",
      payload: { email: defaultUser.email, password: "senha-errada" },
    });

    // Act
    const unknownEmail = await app.inject({
      method: "POST",
      url: "/auth/login",
      payload: { email: "ninguem@email.com", password: "senha-errada" },
    });

    // Assert: a resposta não pode revelar se o email tem conta.
    expect(unknownEmail.statusCode).toBe(wrongPassword.statusCode);
    expect(unknownEmail.body).toBe(wrongPassword.body);
  });

  test("a senha diferencia maiúsculas de minúsculas", async () => {
    // Arrange
    await registerUser(app);

    // Act
    const response = await app.inject({
      method: "POST",
      url: "/auth/login",
      payload: { email: defaultUser.email, password: defaultUser.password.toUpperCase() },
    });

    // Assert
    expect(response.statusCode).toBe(401);
  });

  test.each([
    ["corpo vazio", {}, "Campo obrigatório ausente ou inválido"],
    ["senha vazia", { email: "joao@email.com", password: "" }, "Campo obrigatório ausente ou inválido"],
    ["email inválido", { email: "nao-e-email", password: "senha123" }, "Email inválido"],
  ])("responde 400 com %s", async (_caso, payload, error) => {
    const response = await app.inject({ method: "POST", url: "/auth/login", payload });

    expect(response.statusCode).toBe(400);
    expect(response.json()).toEqual({ error });
  });
});
