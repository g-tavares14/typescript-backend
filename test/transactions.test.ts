import { eq } from "drizzle-orm";
import { afterAll, beforeEach, describe, expect, test } from "vitest";
import { transactions } from "../src/db/schema.ts";
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

const validBody = { type: "expense", amount: 1990, description: "Almoço", date: "2026-09-29" };

function postTransaction(payload: unknown, authorization?: string) {
  return app.inject({
    method: "POST",
    url: "/transactions",
    headers: authorization === undefined ? {} : { authorization },
    payload: payload as Record<string, unknown>,
  });
}

beforeEach(async () => {
  await resetDatabase(db);
});

afterAll(async () => {
  await closeTestApp(app, db);
});

describe("POST /transactions", () => {
  test("responde 201 com o registro criado, sem expor o userId", async () => {
    // Arrange
    await registerUser(app);
    const token = await loginUser(app);

    // Act
    const response = await postTransaction(validBody, bearer(token));

    // Assert
    expect(response.statusCode).toBe(201);
    expect(response.json()).toEqual({
      id: expect.any(String),
      type: "expense",
      amount: 1990,
      description: "Almoço",
      date: "2026-09-29",
      createdAt: expect.any(String),
    });
  });

  test("salva o registro no banco com o user_id do token", async () => {
    // Arrange
    const { id: userId } = await registerUser(app);
    const token = await loginUser(app);

    // Act
    const response = await postTransaction(validBody, bearer(token));

    // Assert
    const rows = await db.select().from(transactions);
    expect(rows).toHaveLength(1);
    expect(rows[0]).toMatchObject({
      id: response.json().id,
      userId,
      type: "expense",
      amountCents: 1990,
      description: "Almoço",
      occurredOn: "2026-09-29",
    });
  });

  test("ignora userId, id e createdAt enviados no corpo (o dono vem sempre do token)", async () => {
    // Arrange: dois usuários; o A tenta criar um registro em nome do B.
    const a = await registerUser(app);
    const b = await registerUser(app, { username: "maria", email: "maria@email.com", password: "senha123" });
    const tokenA = await loginUser(app);
    const forgedId = "00000000-0000-4000-8000-000000000000";

    // Act
    const response = await postTransaction(
      { ...validBody, userId: b.id, id: forgedId, createdAt: "2000-01-01T00:00:00.000Z" },
      bearer(tokenA),
    );

    // Assert
    expect(response.statusCode).toBe(201);
    expect(response.json().id).not.toBe(forgedId);
    expect(response.json().createdAt).not.toBe("2000-01-01T00:00:00.000Z");
    const [row] = await db.select().from(transactions).where(eq(transactions.id, response.json().id));
    expect(row?.userId).toBe(a.id);
    expect(await db.select().from(transactions).where(eq(transactions.userId, b.id))).toHaveLength(0);
  });

  test("responde 400 genérico quando o corpo não atende ao formato (validação detalhada é da T3)", async () => {
    // Arrange
    await registerUser(app);
    const token = await loginUser(app);

    // Act
    const response = await postTransaction({ type: "expense" }, bearer(token));

    // Assert
    expect(response.statusCode).toBe(400);
    expect(response.json()).toEqual({ error: "Campo obrigatório ausente ou inválido" });
    expect(await db.select().from(transactions)).toHaveLength(0);
  });

  test("responde 401 sem o header Authorization e não grava nada", async () => {
    expectUnauthorized(await postTransaction(validBody));
    expect(await db.select().from(transactions)).toHaveLength(0);
  });

  test("responde 401 com token inválido", async () => {
    expectUnauthorized(await postTransaction(validBody, bearer("nao-e-um-jwt")));
  });

  test("responde 401 com token revogado pelo logout", async () => {
    // Arrange
    await registerUser(app);
    const token = await loginUser(app);
    await app.inject({ method: "POST", url: "/auth/logout", headers: { authorization: bearer(token) } });

    // Act + Assert
    expectUnauthorized(await postTransaction(validBody, bearer(token)));
    expect(await db.select().from(transactions)).toHaveLength(0);
  });
});
