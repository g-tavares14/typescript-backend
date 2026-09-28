import { afterAll, beforeEach, describe, expect, test } from "vitest";
import { closeTestApp, createTestApp, loginUser, registerUser, resetDatabase } from "./helpers.ts";

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

function expectUnauthorized(response: Awaited<ReturnType<typeof postLogout>>) {
  expect(response.statusCode).toBe(401);
  expect(response.headers["www-authenticate"]).toBe("Bearer");
  expect(response.json()).toEqual({ error: "Não autenticado" });
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
    const response = await postLogout(`Bearer ${token}`);

    // Assert
    expect(response.statusCode).toBe(204);
    expect(response.body).toBe("");
  });

  test("invalida o token: /auth/me e um segundo /auth/logout respondem 401", async () => {
    // Arrange
    await registerUser(app);
    const token = await loginUser(app);
    await postLogout(`Bearer ${token}`);

    // Act
    const me = await getMe(`Bearer ${token}`);
    const secondLogout = await postLogout(`Bearer ${token}`);

    // Assert
    expectUnauthorized(me);
    expectUnauthorized(secondLogout);
  });

  test("um novo login depois do logout gera um token que funciona", async () => {
    // Arrange
    await registerUser(app);
    const oldToken = await loginUser(app);
    await postLogout(`Bearer ${oldToken}`);

    // Act
    const newToken = await loginUser(app);
    const response = await getMe(`Bearer ${newToken}`);

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
    await postLogout(`Bearer ${tokenA}`);

    // Assert
    expect((await getMe(`Bearer ${tokenA}`)).statusCode).toBe(401);
    expect((await getMe(`Bearer ${tokenB}`)).statusCode).toBe(200);
  });

  test("sai de todos os dispositivos: dois tokens do mesmo usuário morrem com um logout", async () => {
    // Arrange: dois logins = dois "dispositivos". Ambos valem antes do logout.
    await registerUser(app);
    const tokenCelular = await loginUser(app);
    const tokenNotebook = await loginUser(app);
    expect((await getMe(`Bearer ${tokenCelular}`)).statusCode).toBe(200);
    expect((await getMe(`Bearer ${tokenNotebook}`)).statusCode).toBe(200);

    // Act: sai por um dos dispositivos.
    await postLogout(`Bearer ${tokenCelular}`);

    // Assert
    expectUnauthorized(await getMe(`Bearer ${tokenCelular}`));
    expectUnauthorized(await getMe(`Bearer ${tokenNotebook}`));
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
