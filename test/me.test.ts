import { eq } from "drizzle-orm";
import { SignJWT } from "jose";
import { afterAll, beforeEach, describe, expect, test } from "vitest";
import { users } from "../src/db/schema.ts";
import { closeTestApp, createTestApp, loginUser, registerUser, resetDatabase } from "./helpers.ts";

const { app, db } = createTestApp();

// O mesmo segredo que o app usa nos testes (vem do .env.test): permite montar tokens "quase válidos".
const secretKey = new TextEncoder().encode(process.env.JWT_SECRET);

function getMe(authorization?: string) {
  return app.inject({
    method: "GET",
    url: "/auth/me",
    headers: authorization === undefined ? {} : { authorization },
  });
}

function base64url(value: object) {
  return Buffer.from(JSON.stringify(value)).toString("base64url");
}

function expectUnauthorized(response: Awaited<ReturnType<typeof getMe>>) {
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

describe("GET /auth/me", () => {
  test("responde 200 com os dados do usuário dono do token", async () => {
    // Arrange
    const { id } = await registerUser(app);
    const token = await loginUser(app);

    // Act
    const response = await getMe(`Bearer ${token}`);

    // Assert
    expect(response.statusCode).toBe(200);
    expect(response.json()).toEqual({
      id,
      username: "joao",
      email: "joao@email.com",
      role: "user",
      createdAt: expect.any(String),
    });
  });

  test("aceita o esquema 'bearer' em minúsculas (o esquema não diferencia maiúsculas)", async () => {
    // Arrange
    await registerUser(app);
    const token = await loginUser(app);

    // Act
    const response = await getMe(`bearer ${token}`);

    // Assert
    expect(response.statusCode).toBe(200);
  });

  test("devolve a role que está no banco", async () => {
    // Arrange
    const { id } = await registerUser(app);
    const token = await loginUser(app);
    await db.update(users).set({ role: "admin" }).where(eq(users.id, id));

    // Act
    const response = await getMe(`Bearer ${token}`);

    // Assert
    expect(response.json()).toMatchObject({ role: "admin" });
  });

  test("responde 401 sem o header Authorization", async () => {
    expectUnauthorized(await getMe());
  });

  test.each([
    ["esquema Basic", "Basic am9hbzpzZW5oYTEyMw=="],
    ["só a palavra Bearer", "Bearer"],
    ["Bearer sem token", "Bearer "],
    ["token que não é JWT", "Bearer nao-e-um-jwt"],
  ])("responde 401 com %s", async (_caso, authorization) => {
    expectUnauthorized(await getMe(authorization));
  });

  test("responde 401 quando o payload do token foi alterado (assinatura não bate)", async () => {
    // Arrange: troca a role do payload para "admin" e mantém a assinatura original.
    const { id } = await registerUser(app);
    const token = await loginUser(app);
    const [header, , signature] = token.split(".");
    const forgedPayload = base64url({ sub: id, ver: 0, role: "admin", exp: Math.floor(Date.now() / 1000) + 3600 });

    // Act
    const response = await getMe(`Bearer ${header}.${forgedPayload}.${signature}`);

    // Assert
    expectUnauthorized(response);
  });

  test("responde 401 quando o token foi assinado com outro segredo", async () => {
    // Arrange
    const { id } = await registerUser(app);
    const token = await new SignJWT({ ver: 0 })
      .setProtectedHeader({ alg: "HS256" })
      .setSubject(id)
      .setExpirationTime("1h")
      .sign(new TextEncoder().encode("outro-segredo-qualquer-com-mais-de-32-caracteres"));

    // Act + Assert
    expectUnauthorized(await getMe(`Bearer ${token}`));
  });

  test("responde 401 com token sem assinatura (alg: none)", async () => {
    // Arrange
    const { id } = await registerUser(app);
    const payload = { sub: id, ver: 0, role: "admin", exp: Math.floor(Date.now() / 1000) + 3600 };
    const token = `${base64url({ alg: "none", typ: "JWT" })}.${base64url(payload)}.`;

    // Act + Assert
    expectUnauthorized(await getMe(`Bearer ${token}`));
  });

  test("responde 401 com token expirado", async () => {
    // Arrange: token assinado com o segredo certo, mas vencido há 1 minuto.
    const { id } = await registerUser(app);
    const now = Math.floor(Date.now() / 1000);
    const token = await new SignJWT({ ver: 0 })
      .setProtectedHeader({ alg: "HS256" })
      .setSubject(id)
      .setIssuedAt(now - 3660)
      .setExpirationTime(now - 60)
      .sign(secretKey);

    // Act + Assert
    expectUnauthorized(await getMe(`Bearer ${token}`));
  });

  test("responde 401 com token sem expiração", async () => {
    // Arrange: assinado com o segredo certo, mas sem "exp" (valeria para sempre).
    const { id } = await registerUser(app);
    const token = await new SignJWT({ ver: 0 })
      .setProtectedHeader({ alg: "HS256" })
      .setSubject(id)
      .sign(secretKey);

    // Act + Assert
    expectUnauthorized(await getMe(`Bearer ${token}`));
  });

  test("responde 401 quando o 'sub' do token não é um id válido", async () => {
    // Arrange
    const token = await new SignJWT({ ver: 0 })
      .setProtectedHeader({ alg: "HS256" })
      .setSubject("nao-e-uuid")
      .setExpirationTime("1h")
      .sign(secretKey);

    // Act + Assert
    expectUnauthorized(await getMe(`Bearer ${token}`));
  });

  test("responde 401 quando a versão do token não é a atual", async () => {
    // Arrange: o token foi emitido com ver 0; o banco passa a exigir a versão 1 (como faz o logout).
    const { id } = await registerUser(app);
    const token = await loginUser(app);
    await db.update(users).set({ tokenVersion: 1 }).where(eq(users.id, id));

    // Act + Assert
    expectUnauthorized(await getMe(`Bearer ${token}`));
  });

  test("responde 401 com token válido e bem assinado, mas sem 'ver'", async () => {
    // Arrange
    const { id } = await registerUser(app);
    const token = await new SignJWT({})
      .setProtectedHeader({ alg: "HS256" })
      .setSubject(id)
      .setExpirationTime("1h")
      .sign(secretKey);

    // Act + Assert
    expectUnauthorized(await getMe(`Bearer ${token}`));
  });

  test("responde 401 quando o usuário foi apagado depois do login", async () => {
    // Arrange
    const { id } = await registerUser(app);
    const token = await loginUser(app);
    await db.delete(users).where(eq(users.id, id));

    // Act + Assert
    expectUnauthorized(await getMe(`Bearer ${token}`));
  });
});
