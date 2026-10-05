import { eq, sql } from "drizzle-orm";
import { isParity } from "./parity.ts";
import { afterAll, beforeEach, describe, expect, test } from "vitest";
import { users } from "../src/db/schema.ts";
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
const NEW_PASSWORD = "outraSenha456";
const WRONG_PASSWORD = { error: "Senha incorreta" };
const INVALID_CREDENTIALS = { error: "Email ou senha inválidos" };

beforeEach(async () => {
  await resetDatabase(db);
});

afterAll(async () => {
  await closeTestApp(app, db);
});

function putPassword(token: string | undefined, payload: object, target = app) {
  return target.inject({
    method: "PUT",
    url: "/users/me/password",
    headers: token ? { authorization: bearer(token) } : {},
    payload,
  });
}

function getMe(token: string) {
  return app.inject({ method: "GET", url: "/users/me", headers: { authorization: bearer(token) } });
}

function login(password: string) {
  return app.inject({ method: "POST", url: "/auth/login", payload: { email: userA.email, password } });
}

describe("PUT /users/me/password: caminho feliz", () => {
  test("senha atual certa: 200 com token novo no formato do login", async () => {
    // Arrange
    await registerUser(app, userA);
    const token = await loginUser(app, userA);

    // Act
    const response = await putPassword(token, { currentPassword: userA.password, newPassword: NEW_PASSWORD });

    // Assert
    expect(response.statusCode).toBe(200);
    const body = response.json<{ token: string }>();
    expect(body).toEqual({ token: expect.any(String), tokenType: "Bearer", expiresIn: 3600 });
    expect(body.token).not.toBe(token);
    expect((await getMe(body.token)).statusCode).toBe(200);
  });

  test("depois da troca: tokens antigos (de todos os dispositivos) dão 401", async () => {
    // Arrange: dois logins = dois dispositivos.
    await registerUser(app, userA);
    const token = await loginUser(app, userA);
    const otherDevice = await loginUser(app, userA);

    // Act
    await putPassword(token, { currentPassword: userA.password, newPassword: NEW_PASSWORD });

    // Assert
    expectUnauthorized(await getMe(token));
    expectUnauthorized(await getMe(otherDevice));
  });

  test("depois da troca: login com a senha nova entra e com a antiga não", async () => {
    // Arrange
    await registerUser(app, userA);
    const token = await loginUser(app, userA);

    // Act
    await putPassword(token, { currentPassword: userA.password, newPassword: NEW_PASSWORD });

    // Assert
    expect((await login(NEW_PASSWORD)).statusCode).toBe(200);
    const old = await login(userA.password);
    expect(old.statusCode).toBe(401);
    expect(old.json()).toEqual(INVALID_CREDENTIALS);
  });

  test("a nova senha pode ser igual à atual: 200, e os tokens antigos caem do mesmo jeito", async () => {
    // Arrange
    await registerUser(app, userA);
    const token = await loginUser(app, userA);

    // Act
    const response = await putPassword(token, { currentPassword: userA.password, newPassword: userA.password });

    // Assert
    expect(response.statusCode).toBe(200);
    expectUnauthorized(await getMe(token));
  });

  test("o hash gravado nunca é a senha em texto puro", async () => {
    // Arrange
    const { id } = await registerUser(app, userA);
    const token = await loginUser(app, userA);

    // Act
    await putPassword(token, { currentPassword: userA.password, newPassword: NEW_PASSWORD });

    // Assert
    const [row] = await db.select({ hash: users.passwordHash }).from(users).where(eq(users.id, id));
    expect(row?.hash).toMatch(/^\$argon2id\$/);
    expect(row?.hash).not.toContain(NEW_PASSWORD);
  });
});

describe("PUT /users/me/password: senha atual errada", () => {
  test("403 Senha incorreta, e a senha e os tokens continuam valendo", async () => {
    // Arrange
    await registerUser(app, userA);
    const token = await loginUser(app, userA);

    // Act
    const response = await putPassword(token, { currentPassword: "senha-errada", newPassword: NEW_PASSWORD });

    // Assert
    expect(response.statusCode).toBe(403);
    expect(response.json()).toEqual(WRONG_PASSWORD);
    expect((await getMe(token)).statusCode).toBe(200);
    expect((await login(userA.password)).statusCode).toBe(200);
    expect((await login(NEW_PASSWORD)).statusCode).toBe(401);
  });
});

describe("PUT /users/me/password: autenticação", () => {
  test("sem token: 401 padrão", async () => {
    expectUnauthorized(await putPassword(undefined, { currentPassword: "x", newPassword: NEW_PASSWORD }));
  });

  test("token revogado por logout: 401 padrão", async () => {
    // Arrange
    await registerUser(app, userA);
    const token = await loginUser(app, userA);
    await app.inject({ method: "POST", url: "/auth/logout", headers: { authorization: bearer(token) } });

    // Act + Assert
    expectUnauthorized(await putPassword(token, { currentPassword: userA.password, newPassword: NEW_PASSWORD }));
  });

  // Só-TS: injeta um hook no Fastify. Equivalente em Rust: tests/users.rs (change_password com versão antiga).
  test.skipIf(isParity)("logout entre o requireAuth e o UPDATE: 401 padrão, e a senha não muda", async () => {
    // Arrange: o hook preHandler (depois do requireAuth, antes da rota) revoga os tokens, como um logout em outro
    // dispositivo no meio da requisição. O UPDATE com `token_version` na condição não pode achar a linha.
    const { app: raceApp, db: raceDb } = createTestApp();
    raceApp.addHook("preHandler", async (request) => {
      if (request.user) {
        await raceDb
          .update(users)
          .set({ tokenVersion: sql`${users.tokenVersion} + 1` })
          .where(eq(users.id, request.user.id));
      }
    });
    try {
      await registerUser(raceApp, userA);
      const token = await loginUser(raceApp, userA);

      // Act
      const response = await putPassword(
        token,
        { currentPassword: userA.password, newPassword: NEW_PASSWORD },
        raceApp,
      );

      // Assert
      expectUnauthorized(response);
      expect((await login(userA.password)).statusCode).toBe(200);
    } finally {
      await closeTestApp(raceApp, raceDb);
    }
  });
});

describe("PUT /users/me/password: validação", () => {
  const REQUIRED = { error: "Campo obrigatório ausente ou inválido" };
  const SHORT = { error: "A senha deve ter no mínimo 8 caracteres" };
  const INVALID_BODY = { error: "Corpo da requisição inválido: envie um objeto JSON" };

  async function tokenForUserA() {
    await registerUser(app, userA);
    return loginUser(app, userA);
  }

  test.each([
    ["ausente", { newPassword: NEW_PASSWORD }],
    ["vazia", { currentPassword: "", newPassword: NEW_PASSWORD }],
    ["null", { currentPassword: null, newPassword: NEW_PASSWORD }],
    ["número", { currentPassword: 123, newPassword: NEW_PASSWORD }],
  ])("currentPassword %s: 400 campo obrigatório", async (_caso, payload) => {
    const response = await putPassword(await tokenForUserA(), payload);
    expect(response.statusCode).toBe(400);
    expect(response.json()).toEqual(REQUIRED);
  });

  test.each([
    ["ausente", { currentPassword: userA.password }, REQUIRED],
    ["null", { currentPassword: userA.password, newPassword: null }, REQUIRED],
    ["número", { currentPassword: userA.password, newPassword: 12345678 }, REQUIRED],
    ["com 7 caracteres", { currentPassword: userA.password, newPassword: "1234567" }, SHORT],
  ])("newPassword %s: 400", async (_caso, payload, expected) => {
    const response = await putPassword(await tokenForUserA(), payload);
    expect(response.statusCode).toBe(400);
    expect(response.json()).toEqual(expected);
  });

  test("os dois inválidos: a mensagem é a do currentPassword, e a senha não muda", async () => {
    const token = await tokenForUserA();

    const response = await putPassword(token, { currentPassword: "", newPassword: "123" });

    expect(response.json()).toEqual(REQUIRED);
    expect((await login(userA.password)).statusCode).toBe(200);
  });

  test("newPassword curta com currentPassword errada: 400 (a validação vem antes da senha)", async () => {
    const response = await putPassword(await tokenForUserA(), { currentPassword: "errada", newPassword: "123" });
    expect(response.statusCode).toBe(400);
    expect(response.json()).toEqual(SHORT);
  });

  test.each([
    ["null", "null"],
    ["array", "[]"],
    ["texto", '"senha"'],
  ])("corpo %s: 400 INVALID_BODY", async (_caso, payload) => {
    const response = await app.inject({
      method: "PUT",
      url: "/users/me/password",
      headers: { authorization: bearer(await tokenForUserA()), "content-type": "application/json" },
      payload,
    });
    expect(response.statusCode).toBe(400);
    expect(response.json()).toEqual(INVALID_BODY);
  });

  test("campos a mais são ignorados", async () => {
    const response = await putPassword(await tokenForUserA(), {
      currentPassword: userA.password,
      newPassword: NEW_PASSWORD,
      role: "admin",
      tokenVersion: 99,
    });
    expect(response.statusCode).toBe(200);
  });

  test("sem token e corpo inválido: 401 antes do 400", async () => {
    expectUnauthorized(await putPassword(undefined, { currentPassword: "" }));
  });
});
