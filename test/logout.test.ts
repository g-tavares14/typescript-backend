import { afterAll, beforeEach, describe, expect, test } from "vitest";
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

const outroUsuario = { username: "maria", email: "maria@email.com", password: "senha456" };

function postLogout(authorization?: string) {
  return app.inject({
    method: "POST",
    url: "/auth/logout",
    headers: authorization === undefined ? {} : { authorization },
  });
}

function getMe(authorization: string) {
  return app.inject({ method: "GET", url: "/auth/me", headers: { authorization } });
}

beforeEach(async () => {
  await resetDatabase(db);
});

afterAll(async () => {
  await closeTestApp(app, db);
});

describe("POST /auth/logout", () => {
  test("responde 204 sem corpo", async () => {
    // Arrange
    await registerUser(app);
    const token = await loginUser(app);

    // Act
    const response = await postLogout(bearer(token));

    // Assert
    expect(response.statusCode).toBe(204);
    expect(response.body).toBe("");
  });

  test("invalida o token: /auth/me e um segundo /auth/logout respondem 401", async () => {
    // Arrange
    await registerUser(app);
    const token = await loginUser(app);
    await postLogout(bearer(token));

    // Act
    const me = await getMe(bearer(token));
    const secondLogout = await postLogout(bearer(token));

    // Assert
    expectUnauthorized(me);
    expectUnauthorized(secondLogout);
  });

  test("um novo login depois do logout gera um token que funciona", async () => {
    // Arrange
    await registerUser(app);
    const oldToken = await loginUser(app);
    await postLogout(bearer(oldToken));

    // Act
    const newToken = await loginUser(app);
    const response = await getMe(bearer(newToken));

    // Assert
    expect(response.statusCode).toBe(200);
  });

  test("o logout de um usuário não afeta o token de outro", async () => {
    // Arrange
    await registerUser(app);
    await registerUser(app, outroUsuario);
    const tokenA = await loginUser(app);
    const tokenB = await loginUser(app, outroUsuario);

    // Act
    await postLogout(bearer(tokenA));

    // Assert
    expect((await getMe(bearer(tokenA))).statusCode).toBe(401);
    expect((await getMe(bearer(tokenB))).statusCode).toBe(200);
  });

  test("sai de todos os dispositivos: dois tokens do mesmo usuário morrem com um logout", async () => {
    // Arrange: dois logins = dois "dispositivos". Ambos valem antes do logout.
    await registerUser(app);
    const tokenCelular = await loginUser(app);
    const tokenNotebook = await loginUser(app);
    expect((await getMe(bearer(tokenCelular))).statusCode).toBe(200);
    expect((await getMe(bearer(tokenNotebook))).statusCode).toBe(200);

    // Act: sai por um dos dispositivos.
    await postLogout(bearer(tokenCelular));

    // Assert
    expectUnauthorized(await getMe(bearer(tokenCelular)));
    expectUnauthorized(await getMe(bearer(tokenNotebook)));
  });

  test("responde 401 sem o header Authorization", async () => {
    expectUnauthorized(await postLogout());
  });

  test.each([
    ["esquema Basic", "Basic am9hbzpzZW5oYTEyMw=="],
    ["Bearer sem token", "Bearer "],
    ["token que não é JWT", "Bearer nao-e-um-jwt"],
  ])("responde 401 com %s", async (_caso, authorization) => {
    expectUnauthorized(await postLogout(authorization));
  });
});
