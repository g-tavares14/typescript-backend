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

  describe("validação do corpo", () => {
    const REQUIRED = "Campo obrigatório ausente ou inválido";
    const TYPE_ERROR = "O tipo deve ser income ou expense";
    const AMOUNT_ERROR = "O valor deve ser um número inteiro de centavos maior que zero";
    const DESCRIPTION_ERROR = "A descrição deve ter entre 1 e 200 caracteres";
    const DATE_ERROR = "Data inválida (use AAAA-MM-DD)";

    // Um único usuário logado por teste; o "Arrange" dos casos abaixo é só montar o corpo.
    async function postAsUser(payload: unknown) {
      await registerUser(app);
      const token = await loginUser(app);
      return postTransaction(payload, bearer(token));
    }

    // Regra única, igual ao cadastro: campo ausente ou com tipo JSON errado -> `required`;
    // tipo certo mas valor fora da regra -> mensagem específica do campo.
    test.each([
      ["type ausente", { ...validBody, type: undefined }],
      ["amount ausente", { ...validBody, amount: undefined }],
      ["description ausente", { ...validBody, description: undefined }],
      ["date ausente", { ...validBody, date: undefined }],
      ["corpo vazio", {}],
      ["type numérico", { ...validBody, type: 123 }],
      ["type nulo", { ...validBody, type: null }],
      ["amount em string", { ...validBody, amount: "1990" }],
      ["amount nulo", { ...validBody, amount: null }],
      ["description numérica", { ...validBody, description: 5 }],
      ["date numérica", { ...validBody, date: 20260929 }],
    ])("responde 400 required e não grava nada: %s", async (_caso, payload) => {
      // Act
      const response = await postAsUser(payload);

      // Assert
      expect(response.statusCode).toBe(400);
      expect(response.json()).toEqual({ error: REQUIRED });
      expect(await db.select().from(transactions)).toHaveLength(0);
    });

    test.each([
      ["type fora de income/expense", { ...validBody, type: "foo" }, TYPE_ERROR],
      ["type com maiúscula (o valor é exato)", { ...validBody, type: "Income" }, TYPE_ERROR],
      ["amount 0", { ...validBody, amount: 0 }, AMOUNT_ERROR],
      ["amount negativo", { ...validBody, amount: -1 }, AMOUNT_ERROR],
      ["amount em reais (19.9)", { ...validBody, amount: 19.9 }, AMOUNT_ERROR],
      ["amount acima de R$ 1 bilhão", { ...validBody, amount: 100_000_000_001 }, AMOUNT_ERROR],
      ["description vazia", { ...validBody, description: "" }, DESCRIPTION_ERROR],
      ["description só com espaços", { ...validBody, description: "   " }, DESCRIPTION_ERROR],
      ["description com 201 caracteres", { ...validBody, description: "a".repeat(201) }, DESCRIPTION_ERROR],
      ["date inexistente (2026-02-30)", { ...validBody, date: "2026-02-30" }, DATE_ERROR],
      ["date no formato brasileiro", { ...validBody, date: "29/09/2026" }, DATE_ERROR],
    ])("responde 400 com a mensagem do campo e não grava nada: %s", async (_caso, payload, message) => {
      // Act
      const response = await postAsUser(payload);

      // Assert
      expect(response.statusCode).toBe(400);
      expect(response.json()).toEqual({ error: message });
      expect(await db.select().from(transactions)).toHaveLength(0);
    });

    test.each([
      ["amount igual ao teto (R$ 1 bilhão)", { amount: 100_000_000_000 }],
      ["amount 1 centavo", { amount: 1 }],
      ["description com exatamente 200 caracteres", { description: "a".repeat(200) }],
      ["date no futuro", { date: "2099-12-31" }],
      ["type income", { type: "income" }],
    ])("aceita o limite: %s", async (_caso, override) => {
      // Act
      const response = await postAsUser({ ...validBody, ...override });

      // Assert
      expect(response.statusCode).toBe(201);
      expect(response.json()).toMatchObject(override);
      expect(await db.select().from(transactions)).toHaveLength(1);
    });

    test("aplica trim na descrição antes de validar o tamanho e antes de salvar", async () => {
      // Arrange: 200 caracteres úteis + espaços nas pontas só passam se o trim vier antes do max(200).
      const description = "a".repeat(200);

      // Act
      const response = await postAsUser({ ...validBody, description: `  ${description}  ` });

      // Assert
      expect(response.statusCode).toBe(201);
      expect(response.json().description).toBe(description);
      const [row] = await db.select().from(transactions);
      expect(row?.description).toBe(description);
    });

    test("salva a descrição sem os espaços das pontas", async () => {
      // Act
      const response = await postAsUser({ ...validBody, description: "  Almoço  " });

      // Assert
      expect(response.statusCode).toBe(201);
      expect(response.json().description).toBe("Almoço");
      const [row] = await db.select().from(transactions);
      expect(row?.description).toBe("Almoço");
    });
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
