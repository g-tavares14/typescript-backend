import { eq } from "drizzle-orm";
import { isParity } from "./parity.ts";
import { afterAll, beforeEach, describe, expect, test } from "vitest";
import { transactions, users } from "../src/db/schema.ts";
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

const userA = { username: "joao", email: "joao@email.com", password: "senha123" };
const userB = { username: "maria", email: "maria@email.com", password: "senha123" };
const REQUIRED = { error: "Campo obrigatório ausente ou inválido" };
const NO_FIELDS = { error: "Envie ao menos um campo para alterar" };
const INVALID_BODY = { error: "Corpo da requisição inválido: envie um objeto JSON" };
const USERNAME_LENGTH = { error: "O username deve ter entre 3 e 50 caracteres" };
const USERNAME_CHARS = { error: "O username só pode ter letras sem acento, números e _" };
const CONFLICT = { error: "Email ou username já cadastrado" };

beforeEach(async () => {
  await resetDatabase(db);
});

afterAll(async () => {
  await closeTestApp(app, db);
});

// Atalho do "Arrange": cadastra e faz login pelas rotas de verdade.
async function createUserWithToken(user: typeof userA) {
  await registerUser(app, user);
  return loginUser(app, user);
}

function patchMe(token: string | undefined, payload: unknown) {
  return app.inject({
    method: "PATCH",
    url: "/users/me",
    headers: token === undefined ? {} : { authorization: bearer(token) },
    payload: payload as object,
  });
}

function getMe(token: string) {
  return app.inject({ method: "GET", url: "/users/me", headers: { authorization: bearer(token) } });
}

describe("PATCH /users/me", () => {
  test("altera só o username: 200 no formato do GET /users/me, normalizado, e o mesmo token continua valendo", async () => {
    // Arrange
    const { id } = await registerUser(app, userA);
    const token = await loginUser(app, userA);

    // Act
    const response = await patchMe(token, { username: "  Joao_Silva " });

    // Assert
    expect(response.statusCode).toBe(200);
    expect(response.json()).toEqual({
      id,
      username: "joao_silva",
      email: "joao@email.com",
      role: "user",
      createdAt: expect.any(String),
    });
    const me = await getMe(token);
    expect(me.statusCode).toBe(200);
    expect(me.json()).toMatchObject({ username: "joao_silva", email: "joao@email.com" });
  });

  test("altera só o email: normalizado em minúsculas, sem mexer no username", async () => {
    // Arrange
    const token = await createUserWithToken(userA);

    // Act
    const response = await patchMe(token, { email: "  Novo@Email.COM " });

    // Assert
    expect(response.statusCode).toBe(200);
    expect(response.json()).toMatchObject({ username: "joao", email: "novo@email.com" });
    expect((await getMe(token)).json()).toMatchObject({ username: "joao", email: "novo@email.com" });
  });

  test("altera os dois campos de uma vez", async () => {
    // Arrange
    const token = await createUserWithToken(userA);

    // Act
    const response = await patchMe(token, { username: "novo_nome", email: "novo@email.com" });

    // Assert
    expect(response.statusCode).toBe(200);
    expect(response.json()).toMatchObject({ username: "novo_nome", email: "novo@email.com" });
  });

  test("depois de trocar o email, o login com o novo funciona e com o antigo dá 401", async () => {
    // Arrange
    const token = await createUserWithToken(userA);
    await patchMe(token, { email: "novo@email.com" });

    // Act
    const withNew = await app.inject({
      method: "POST",
      url: "/auth/login",
      payload: { email: "novo@email.com", password: userA.password },
    });
    const withOld = await app.inject({
      method: "POST",
      url: "/auth/login",
      payload: { email: userA.email, password: userA.password },
    });

    // Assert
    expect(withNew.statusCode).toBe(200);
    expect(withOld.statusCode).toBe(401);
    expect(withOld.json()).toEqual({ error: "Email ou senha inválidos" });
  });

  test("enviar o próprio valor atual responde 200", async () => {
    // Arrange
    const token = await createUserWithToken(userA);

    // Act
    const response = await patchMe(token, { username: "joao", email: "joao@email.com" });

    // Assert
    expect(response.statusCode).toBe(200);
    expect(response.json()).toMatchObject({ username: "joao", email: "joao@email.com" });
  });

  test("username de outra conta: 409, mesmo em maiúsculas", async () => {
    // Arrange
    await registerUser(app, userB);
    const token = await createUserWithToken(userA);

    // Act
    const response = await patchMe(token, { username: "MARIA" });

    // Assert
    expect(response.statusCode).toBe(409);
    expect(response.json()).toEqual(CONFLICT);
  });

  test("email de outra conta: 409, e o username enviado junto também não muda", async () => {
    // Arrange
    await registerUser(app, userB);
    const token = await createUserWithToken(userA);

    // Act
    const response = await patchMe(token, { username: "outro_nome", email: userB.email });

    // Assert
    expect(response.statusCode).toBe(409);
    expect(response.json()).toEqual(CONFLICT);
    expect((await getMe(token)).json()).toMatchObject({ username: "joao", email: "joao@email.com" });
  });

  test("não altera a conta de outro usuário", async () => {
    // Arrange
    const { id: otherId } = await registerUser(app, userB);
    const token = await createUserWithToken(userA);

    // Act
    await patchMe(token, { username: "novo_nome", email: "novo@email.com" });

    // Assert: busca pelo id, para conferir os dois campos da outra conta.
    const [other] = await db.select().from(users).where(eq(users.id, otherId));
    expect(other).toMatchObject({ username: "maria", email: userB.email });
  });

  test("responde 401 sem token", async () => {
    expectUnauthorized(await patchMe(undefined, { username: "novo_nome" }));
  });

  test("responde 401 com token revogado por logout", async () => {
    // Arrange
    const token = await createUserWithToken(userA);
    await app.inject({ method: "POST", url: "/auth/logout", headers: { authorization: bearer(token) } });

    // Act + Assert
    expectUnauthorized(await patchMe(token, { username: "novo_nome" }));
  });

  // Só-TS: injeta um hook no Fastify. Equivalente em Rust: tests/users.rs (update_profile com conta inexistente).
  test.skipIf(isParity)("conta apagada entre o requireAuth e o UPDATE: 401 padrão", async () => {
    // Arrange: app próprio, com um hook preHandler (roda depois do requireAuth, antes da rota) que apaga a conta.
    // O Fastify aceita hooks novos até o ready, que o primeiro inject dispara.
    const { app: raceApp, db: raceDb } = createTestApp();
    raceApp.addHook("preHandler", async (request) => {
      if (request.user) {
        await raceDb.delete(users).where(eq(users.id, request.user.id));
      }
    });
    try {
      await registerUser(raceApp, userA);
      const token = await loginUser(raceApp, userA);

      // Act
      const response = await raceApp.inject({
        method: "PATCH",
        url: "/users/me",
        headers: { authorization: bearer(token) },
        payload: { username: "novo_nome" },
      });

      // Assert
      expectUnauthorized(response);
    } finally {
      await closeTestApp(raceApp, raceDb);
    }
  });
});

describe("PATCH /users/me: validação", () => {
  test.each([
    ["{}", {}],
    ["só campo desconhecido", { foo: 1 }],
    ["só password", { password: "x" }],
  ])("corpo sem campo editável (%s): 400 e nada muda", async (_caso, payload) => {
    // Arrange
    const token = await createUserWithToken(userA);

    // Act
    const response = await patchMe(token, payload);

    // Assert
    expect(response.statusCode).toBe(400);
    expect(response.json()).toEqual(NO_FIELDS);
    expect((await getMe(token)).json()).toMatchObject({ username: "joao", email: "joao@email.com" });
  });

  test.each([
    ["email null", { email: null }],
    ["username null", { username: null }],
    ["username número", { username: 123 }],
    ["email número", { email: 123 }],
  ])("%s: 400 de campo obrigatório", async (_caso, payload) => {
    const token = await createUserWithToken(userA);

    const response = await patchMe(token, payload);

    expect(response.statusCode).toBe(400);
    expect(response.json()).toEqual(REQUIRED);
  });

  test.each([
    ["curto", "ab", USERNAME_LENGTH],
    ["longo", "a".repeat(51), USERNAME_LENGTH],
    ["com acento", "joão", USERNAME_CHARS],
    ["com espaço no meio", "joao silva", USERNAME_CHARS],
  ])("username %s: 400 com a mensagem do username, e nada muda", async (_caso, username, expected) => {
    const token = await createUserWithToken(userA);

    const response = await patchMe(token, { username });

    expect(response.statusCode).toBe(400);
    expect(response.json()).toEqual(expected);
    expect((await getMe(token)).json()).toMatchObject({ username: "joao" });
  });

  test("email inválido: 400 Email inválido", async () => {
    const token = await createUserWithToken(userA);

    const response = await patchMe(token, { email: "nao-e-email" });

    expect(response.statusCode).toBe(400);
    expect(response.json()).toEqual({ error: "Email inválido" });
  });

  test("um campo válido e outro inválido: 400, e nem o válido é gravado", async () => {
    const token = await createUserWithToken(userA);

    const response = await patchMe(token, { email: "novo@email.com", username: "ab" });

    expect(response.statusCode).toBe(400);
    expect(response.json()).toEqual(USERNAME_LENGTH);
    expect((await getMe(token)).json()).toMatchObject({ username: "joao", email: userA.email });
  });

  test.each([
    ["null", "null"],
    ["array", "[]"],
    ["texto", '"oi"'],
  ])("corpo JSON %s na raiz: 400 de corpo inválido", async (_caso, raw) => {
    const token = await createUserWithToken(userA);

    const response = await app.inject({
      method: "PATCH",
      url: "/users/me",
      headers: { authorization: bearer(token), "content-type": "application/json" },
      payload: raw,
    });

    expect(response.statusCode).toBe(400);
    expect(response.json()).toEqual(INVALID_BODY);
  });

  test("role e password no corpo são ignorados: 200, role continua user e a senha antiga entra", async () => {
    // Arrange
    const token = await createUserWithToken(userA);

    // Act
    const response = await patchMe(token, { username: "novo_nome", role: "admin", password: "outrasenha1" });

    // Assert
    expect(response.statusCode).toBe(200);
    expect(response.json()).toMatchObject({ username: "novo_nome", role: "user" });
    const [row] = await db.select().from(users).where(eq(users.email, userA.email));
    expect(row?.role).toBe("user");
    expect(await loginUser(app, userA)).toEqual(expect.any(String));
  });
});

describe("DELETE /users/me", () => {
  const WRONG_PASSWORD = { error: "Senha incorreta" };
  const UNSUPPORTED_MEDIA = { error: "Tipo de conteúdo não suportado (use application/json)" };

  function deleteMe(token: string | undefined, payload?: unknown, headers: Record<string, string> = {}) {
    return app.inject({
      method: "DELETE",
      url: "/users/me",
      headers: token === undefined ? headers : { authorization: bearer(token), ...headers },
      payload: payload as object | undefined,
    });
  }

  function createTransaction(token: string) {
    return app.inject({
      method: "POST",
      url: "/transactions",
      headers: { authorization: bearer(token) },
      payload: { type: "expense", amount: 1990, description: "Almoço", date: "2026-09-29" },
    });
  }

  test("com a senha certa: 204 sem corpo; o token, o login e a conta somem", async () => {
    // Arrange
    const token = await createUserWithToken(userA);

    // Act
    const response = await deleteMe(token, { password: userA.password });

    // Assert
    expect(response.statusCode).toBe(204);
    expect(response.body).toBe("");
    expectUnauthorized(await getMe(token));
    const login = await app.inject({
      method: "POST",
      url: "/auth/login",
      payload: { email: userA.email, password: userA.password },
    });
    expect(login.statusCode).toBe(401);
  });

  test("email e username podem ser cadastrados de novo", async () => {
    // Arrange
    const token = await createUserWithToken(userA);
    await deleteMe(token, { password: userA.password });

    // Act
    const response = await app.inject({ method: "POST", url: "/auth/register", payload: userA });

    // Assert
    expect(response.statusCode).toBe(201);
  });

  test("apaga os registros financeiros do usuário e mantém os de outro", async () => {
    // Arrange
    const tokenA = await createUserWithToken(userA);
    const tokenB = await createUserWithToken(userB);
    await createTransaction(tokenA);
    await createTransaction(tokenB);

    // Act
    const response = await deleteMe(tokenA, { password: userA.password });

    // Assert
    expect(response.statusCode).toBe(204);
    expect(await db.select().from(transactions)).toHaveLength(1);
    const list = await app.inject({ method: "GET", url: "/transactions", headers: { authorization: bearer(tokenB) } });
    expect(list.json().transactions).toHaveLength(1);
  });

  test("senha errada: 403 Senha incorreta, e a conta continua", async () => {
    // Arrange
    const token = await createUserWithToken(userA);

    // Act
    const response = await deleteMe(token, { password: "errada123" });

    // Assert
    expect(response.statusCode).toBe(403);
    expect(response.json()).toEqual(WRONG_PASSWORD);
    expect((await getMe(token)).statusCode).toBe(200);
    expect(await loginUser(app, userA)).toEqual(expect.any(String));
  });

  test.each([
    ["sem password", {}],
    ["password vazio", { password: "" }],
    ["password número", { password: 123 }],
  ])("%s: 400 de campo obrigatório", async (_caso, payload) => {
    const token = await createUserWithToken(userA);

    const response = await deleteMe(token, payload);

    expect(response.statusCode).toBe(400);
    expect(response.json()).toEqual(REQUIRED);
    expect((await getMe(token)).statusCode).toBe(200);
  });

  test.each([
    ["sem corpo", undefined],
    ["null", "null"],
    ["array", "[]"],
  ])("corpo %s: 400 de corpo inválido", async (_caso, raw) => {
    const token = await createUserWithToken(userA);

    const response = await deleteMe(token, raw, { "content-type": "application/json" });

    expect(response.statusCode).toBe(400);
    expect(response.json()).toEqual(INVALID_BODY);
  });

  test("sem corpo e sem Content-Type (o DELETE comum do front): 400 de corpo inválido, e a conta continua", async () => {
    const token = await createUserWithToken(userA);

    const response = await deleteMe(token);

    expect(response.statusCode).toBe(400);
    expect(response.json()).toEqual(INVALID_BODY);
    expect((await getMe(token)).statusCode).toBe(200);
  });

  test("Content-Type text/plain: o Fastify lê como texto, que não é objeto JSON: 400", async () => {
    const token = await createUserWithToken(userA);

    const response = await deleteMe(token, "senha123", { "content-type": "text/plain" });

    expect(response.statusCode).toBe(400);
    expect(response.json()).toEqual(INVALID_BODY);
  });

  test("Content-Type sem parser (application/xml): 415", async () => {
    const token = await createUserWithToken(userA);

    const response = await deleteMe(token, "<a/>", { "content-type": "application/xml" });

    expect(response.statusCode).toBe(415);
    expect(response.json()).toEqual(UNSUPPORTED_MEDIA);
  });

  test("responde 401 sem token", async () => {
    expectUnauthorized(await deleteMe(undefined, { password: userA.password }));
  });

  test("responde 401 com token revogado por logout", async () => {
    // Arrange
    const token = await createUserWithToken(userA);
    await app.inject({ method: "POST", url: "/auth/logout", headers: { authorization: bearer(token) } });

    // Act + Assert
    expectUnauthorized(await deleteMe(token, { password: userA.password }));
  });

  // Só-TS: injeta um hook no Fastify. Equivalente em Rust: tests/users.rs.
  test.skipIf(isParity)("conta apagada depois do requireAuth (hook preHandler): 401 padrão", async () => {
    // Arrange: o hook apaga a conta antes do handler, então aqui é o SELECT do hash que volta vazio.
    const { app: raceApp, db: raceDb } = createTestApp();
    raceApp.addHook("preHandler", async (request) => {
      if (request.user) {
        await raceDb.delete(users).where(eq(users.id, request.user.id));
      }
    });
    try {
      await registerUser(raceApp, userA);
      const token = await loginUser(raceApp, userA);

      // Act
      const response = await raceApp.inject({
        method: "DELETE",
        url: "/users/me",
        headers: { authorization: bearer(token) },
        payload: { password: userA.password },
      });

      // Assert
      expectUnauthorized(response);
    } finally {
      await closeTestApp(raceApp, raceDb);
    }
  });

  test("o hash da senha não aparece em nenhuma resposta (inclusive o GET /users/me, que devolve o currentUser())", async () => {
    // Arrange
    const token = await createUserWithToken(userA);

    // Act
    const me = await getMe(token);
    const wrong = await deleteMe(token, { password: "errada123" });
    const ok = await deleteMe(token, { password: userA.password });

    // Assert
    for (const response of [me, wrong, ok]) {
      expect(response.body).not.toMatch(/passwordHash|password_hash|argon2/);
    }
  });
});
