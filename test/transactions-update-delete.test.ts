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

const userA = { username: "joao", email: "joao@email.com", password: "senha123" };
const userB = { username: "maria", email: "maria@email.com", password: "senha123" };
const validBody = { type: "expense", amount: 1990, description: "Almoço", date: "2026-09-29" };
const NOT_FOUND = { error: "Registro não encontrado" };
const INVALID_BODY = { error: "Corpo da requisição inválido: envie um objeto JSON" };
const JSON_HEADERS = { "content-type": "application/json" };
const REQUIRED = "Campo obrigatório ausente ou inválido";
const NO_FIELDS = "Envie ao menos um campo para alterar";
const TYPE_ERROR = "O tipo deve ser income ou expense";
const AMOUNT_ERROR = "O valor deve ser um número inteiro de centavos maior que zero";
const DESCRIPTION_ERROR = "A descrição deve ter entre 1 e 200 caracteres";
const DATE_ERROR = "Data inválida (use AAAA-MM-DD)";

beforeEach(async () => {
  await resetDatabase(db);
});

afterAll(async () => {
  await closeTestApp(app, db);
});

// Atalhos do "Arrange": cadastram, fazem login e criam registros pelas rotas de verdade.
async function createUserWithToken(user: typeof userA) {
  await registerUser(app, user);
  return loginUser(app, user);
}

async function createTransaction(token: string, payload: Record<string, unknown> = validBody) {
  const response = await app.inject({
    method: "POST",
    url: "/transactions",
    headers: { authorization: bearer(token) },
    payload,
  });
  return response.json<{ id: string }>();
}

// POST e PATCH no mesmo milissegundo tornariam a comparação de updatedAt instável: recua as duas datas no banco.
const past = new Date("2026-01-01T00:00:00.000Z");

async function createBackdatedTransaction(token: string) {
  const { id } = await createTransaction(token);
  await db.update(transactions).set({ createdAt: past, updatedAt: past }).where(eq(transactions.id, id));
  return id;
}

function patchTransaction(id: string, payload: Record<string, unknown>, token?: string) {
  return app.inject({
    method: "PATCH",
    url: `/transactions/${id}`,
    headers: token === undefined ? {} : { authorization: bearer(token) },
    payload,
  });
}

function deleteTransaction(id: string, token?: string) {
  return app.inject({
    method: "DELETE",
    url: `/transactions/${id}`,
    headers: token === undefined ? {} : { authorization: bearer(token) },
  });
}

async function listTransactions(token: string) {
  const response = await app.inject({
    method: "GET",
    url: "/transactions",
    headers: { authorization: bearer(token) },
  });
  return response.json<{
    summary: { income: number; expense: number; balance: number };
    transactions: { id: string }[];
  }>();
}

describe("DELETE /transactions/:id", () => {
  test("registro do próprio usuário → 204 com corpo vazio", async () => {
    // Arrange
    const token = await createUserWithToken(userA);
    const { id } = await createTransaction(token);

    // Act
    const response = await deleteTransaction(id, token);

    // Assert
    expect(response.statusCode).toBe(204);
    expect(response.body).toBe("");
  });

  test("o registro excluído some do GET e dos totais", async () => {
    // Arrange: dois registros; só o de saída é excluído.
    const token = await createUserWithToken(userA);
    const expense = await createTransaction(token);
    const income = await createTransaction(token, { ...validBody, type: "income", amount: 5000 });

    // Act
    await deleteTransaction(expense.id, token);

    // Assert
    const list = await listTransactions(token);
    expect(list.transactions.map((t) => t.id)).toEqual([income.id]);
    expect(list.summary).toEqual({ income: 5000, expense: 0, balance: 5000 });
  });

  test("id que não existe → 404", async () => {
    const token = await createUserWithToken(userA);

    const response = await deleteTransaction("00000000-0000-4000-8000-000000000000", token);

    expect(response.statusCode).toBe(404);
    expect(response.json()).toEqual(NOT_FOUND);
  });

  test("registro de outro usuário → 404 igual ao inexistente, e o registro do outro continua no GET dele", async () => {
    // Arrange: o A tem um registro; o B tenta excluí-lo.
    const tokenA = await createUserWithToken(userA);
    const tokenB = await createUserWithToken(userB);
    const { id } = await createTransaction(tokenA);

    // Act
    const response = await deleteTransaction(id, tokenB);

    // Assert
    expect(response.statusCode).toBe(404);
    expect(response.json()).toEqual(NOT_FOUND);
    const list = await listTransactions(tokenA);
    expect(list.transactions.map((t) => t.id)).toEqual([id]);
  });

  test("id que não é UUID → 404 (sem ir ao banco)", async () => {
    const token = await createUserWithToken(userA);

    const response = await deleteTransaction("abc", token);

    expect(response.statusCode).toBe(404);
    expect(response.json()).toEqual(NOT_FOUND);
  });

  test("excluir de novo o mesmo id → 404", async () => {
    // Arrange
    const token = await createUserWithToken(userA);
    const { id } = await createTransaction(token);
    await deleteTransaction(id, token);

    // Act
    const response = await deleteTransaction(id, token);

    // Assert
    expect(response.statusCode).toBe(404);
    expect(response.json()).toEqual(NOT_FOUND);
  });

  test("sem token → 401", async () => {
    const response = await deleteTransaction("00000000-0000-4000-8000-000000000000");

    expectUnauthorized(response);
  });

  test("sem token e com id que não é UUID → 401 (a autenticação vem antes do 404)", async () => {
    const response = await deleteTransaction("abc");

    expectUnauthorized(response);
  });

  test("token revogado por logout → 401, e o registro continua existindo", async () => {
    // Arrange
    const token = await createUserWithToken(userA);
    const { id } = await createTransaction(token);
    await app.inject({ method: "POST", url: "/auth/logout", headers: { authorization: bearer(token) } });

    // Act
    const response = await deleteTransaction(id, token);

    // Assert
    expectUnauthorized(response);
    const newToken = await loginUser(app, userA);
    expect((await listTransactions(newToken)).transactions.map((t) => t.id)).toEqual([id]);
  });
});

describe("PATCH /transactions/:id", () => {
  const original = {
    id: expect.any(String),
    type: "expense",
    amount: 1990,
    description: "Almoço",
    date: "2026-09-29",
    createdAt: expect.any(String),
    updatedAt: expect.any(String),
  };

  // Cada campo sozinho muda só ele: os outros três continuam como no registro criado.
  test.each([
    ["description", "Almoço (corrigido)"],
    ["type", "income"],
    ["amount", 2500],
    ["date", "2026-09-30"],
  ])("só %s → 200 com o registro inteiro, mudando apenas esse campo", async (field, value) => {
    // Arrange
    const token = await createUserWithToken(userA);
    const { id } = await createTransaction(token);

    // Act
    const response = await patchTransaction(id, { [field]: value }, token);

    // Assert
    expect(response.statusCode).toBe(200);
    expect(response.json()).toEqual({ ...original, id, [field]: value });
  });

  test("os quatro campos juntos → 200 com todos alterados", async () => {
    // Arrange
    const token = await createUserWithToken(userA);
    const { id } = await createTransaction(token);
    const changes = { type: "income", amount: 5000, description: "Salário", date: "2026-10-01" };

    // Act
    const response = await patchTransaction(id, changes, token);

    // Assert
    expect(response.statusCode).toBe(200);
    expect(response.json()).toEqual({ ...original, id, ...changes });
  });

  test("o GET e os totais refletem a mudança de amount e de type", async () => {
    // Arrange: uma saída de R$ 19,90.
    const token = await createUserWithToken(userA);
    const { id } = await createTransaction(token);

    // Act + Assert: muda o valor da saída.
    await patchTransaction(id, { amount: 2500 }, token);
    let list = await listTransactions(token);
    expect(list.summary).toEqual({ income: 0, expense: 2500, balance: -2500 });

    // Act + Assert: vira uma entrada.
    await patchTransaction(id, { type: "income" }, token);
    list = await listTransactions(token);
    expect(list.summary).toEqual({ income: 2500, expense: 0, balance: 2500 });
    expect(list.transactions).toHaveLength(1);
    expect(list.transactions[0]).toMatchObject({ id, type: "income", amount: 2500 });
  });

  describe("updatedAt", () => {
    test("passa a ser posterior ao valor anterior e o createdAt não muda", async () => {
      // Arrange
      const token = await createUserWithToken(userA);
      const id = await createBackdatedTransaction(token);

      // Act
      const response = await patchTransaction(id, { description: "Almoço (corrigido)" }, token);

      // Assert
      const { createdAt, updatedAt } = response.json();
      expect(createdAt).toBe(past.toISOString());
      expect(new Date(updatedAt).getTime()).toBeGreaterThan(past.getTime());
      const [row] = await listTransactions(token).then((list) => list.transactions);
      expect(row).toMatchObject({ id, createdAt: past.toISOString(), updatedAt });
    });

    test("PATCH com os mesmos valores atuais também atualiza o updatedAt", async () => {
      // Arrange
      const token = await createUserWithToken(userA);
      const id = await createBackdatedTransaction(token);

      // Act
      const response = await patchTransaction(id, { description: "Almoço" }, token);

      // Assert
      expect(response.statusCode).toBe(200);
      expect(response.json().description).toBe("Almoço");
      expect(response.json().createdAt).toBe(past.toISOString());
      expect(new Date(response.json().updatedAt).getTime()).toBeGreaterThan(past.getTime());
    });
  });

  test("id que não existe → 404", async () => {
    const token = await createUserWithToken(userA);

    const response = await patchTransaction("00000000-0000-4000-8000-000000000000", { amount: 2500 }, token);

    expect(response.statusCode).toBe(404);
    expect(response.json()).toEqual(NOT_FOUND);
  });

  test("registro de outro usuário → 404 igual ao inexistente, e o registro do outro fica intacto", async () => {
    // Arrange: o A tem um registro; o B tenta editá-lo.
    const tokenA = await createUserWithToken(userA);
    const tokenB = await createUserWithToken(userB);
    const { id } = await createTransaction(tokenA);
    const before = await listTransactions(tokenA);

    // Act
    const response = await patchTransaction(id, { amount: 999_999, description: "invadido" }, tokenB);

    // Assert
    expect(response.statusCode).toBe(404);
    expect(response.json()).toEqual(NOT_FOUND);
    expect(await listTransactions(tokenA)).toEqual(before);
  });

  test("id que não é UUID → 404 (sem ir ao banco)", async () => {
    const token = await createUserWithToken(userA);

    const response = await patchTransaction("abc", { amount: 2500 }, token);

    expect(response.statusCode).toBe(404);
    expect(response.json()).toEqual(NOT_FOUND);
  });

  test("sem token → 401", async () => {
    const response = await patchTransaction("00000000-0000-4000-8000-000000000000", { amount: 2500 });

    expectUnauthorized(response);
  });

  test("token revogado por logout → 401, e o registro não é alterado", async () => {
    // Arrange
    const token = await createUserWithToken(userA);
    const { id } = await createTransaction(token);
    await app.inject({ method: "POST", url: "/auth/logout", headers: { authorization: bearer(token) } });

    // Act
    const response = await patchTransaction(id, { amount: 2500 }, token);

    // Assert
    expectUnauthorized(response);
    const newToken = await loginUser(app, userA);
    expect((await listTransactions(newToken)).summary.expense).toBe(1990);
  });
});

describe("PATCH /transactions/:id: validação do corpo", () => {
  // Um usuário com um registro recuado no banco; o teste só monta o corpo. Devolve também a lista de antes,
  // para provar que um PATCH recusado não alterou nada (nem o updatedAt).
  async function arrange() {
    const token = await createUserWithToken(userA);
    const id = await createBackdatedTransaction(token);
    return { token, id, before: await listTransactions(token) };
  }

  test.each([
    ["corpo vazio", {}],
    ["só campos desconhecidos", { foo: 1, bar: "x" }],
  ])("%s → 400 Envie ao menos um campo para alterar", async (_name, payload) => {
    const { token, id, before } = await arrange();

    const response = await patchTransaction(id, payload, token);

    expect(response.statusCode).toBe(400);
    expect(response.json()).toEqual({ error: NO_FIELDS });
    expect(await listTransactions(token)).toEqual(before);
  });

  // Tipo JSON errado, inclusive null (que não apaga nada): mesma regra do POST -> `required`.
  test.each([
    ["amount null", { amount: null }],
    ["amount string", { amount: "1990" }],
    ["type número", { type: 123 }],
    ["type null", { type: null }],
    ["description null", { description: null }],
    ["date número", { date: 20260929 }],
  ])("%s → 400 Campo obrigatório ausente ou inválido", async (_name, payload) => {
    const { token, id, before } = await arrange();

    const response = await patchTransaction(id, payload, token);

    expect(response.statusCode).toBe(400);
    expect(response.json()).toEqual({ error: REQUIRED });
    expect(await listTransactions(token)).toEqual(before);
  });

  // Tipo certo, valor fora da regra -> a mensagem do campo, a mesma do POST.
  test.each([
    ["type inválido", { type: "foo" }, TYPE_ERROR],
    ["amount decimal", { amount: 19.9 }, AMOUNT_ERROR],
    ["amount zero", { amount: 0 }, AMOUNT_ERROR],
    ["amount acima do teto", { amount: 100_000_000_001 }, AMOUNT_ERROR],
    ["description só com espaços", { description: "   " }, DESCRIPTION_ERROR],
    ["description com 201 caracteres", { description: "a".repeat(201) }, DESCRIPTION_ERROR],
    ["date inexistente", { date: "2026-02-30" }, DATE_ERROR],
    ["date em outro formato", { date: "29/09/2026" }, DATE_ERROR],
  ])("%s → 400 com a mensagem do campo", async (_name, payload, message) => {
    const { token, id, before } = await arrange();

    const response = await patchTransaction(id, payload, token);

    expect(response.statusCode).toBe(400);
    expect(response.json()).toEqual({ error: message });
    expect(await listTransactions(token)).toEqual(before);
  });

  // Corpo que não é um objeto JSON: mesma resposta do POST e das rotas de auth (ver test/errors.test.ts).
  test.each([
    ["sem corpo", { headers: {}, payload: undefined }],
    ["application/json vazio", { headers: JSON_HEADERS, payload: "" }],
    ["JSON malformado", { headers: JSON_HEADERS, payload: "{ruim" }],
    ["null", { headers: JSON_HEADERS, payload: "null" }],
    ["array", { headers: JSON_HEADERS, payload: "[]" }],
    ["text/plain", { headers: { "content-type": "text/plain" }, payload: "oi" }],
  ])("corpo raiz inválido (%s) → 400 Corpo da requisição inválido", async (_name, { headers, payload }) => {
    const { token, id, before } = await arrange();

    const response = await app.inject({
      method: "PATCH",
      url: `/transactions/${id}`,
      headers: { ...headers, authorization: bearer(token) },
      payload,
    });

    expect(response.statusCode).toBe(400);
    expect(response.json()).toEqual(INVALID_BODY);
    expect(await listTransactions(token)).toEqual(before);
  });

  test("sem token e com corpo malformado → 401 (a autenticação vem antes do parse do corpo)", async () => {
    const response = await app.inject({
      method: "PATCH",
      url: "/transactions/00000000-0000-4000-8000-000000000000",
      headers: JSON_HEADERS,
      payload: "{ruim",
    });

    expectUnauthorized(response);
  });

  test("a descrição é salva sem os espaços das pontas (trim)", async () => {
    const { token, id } = await arrange();

    const response = await patchTransaction(id, { description: "  Almoço novo  " }, token);

    expect(response.statusCode).toBe(200);
    expect(response.json().description).toBe("Almoço novo");
    expect((await listTransactions(token)).transactions[0]).toMatchObject({ description: "Almoço novo" });
  });

  test("id, userId, createdAt e updatedAt no corpo são ignorados junto de um campo válido", async () => {
    // Arrange: o A tem o registro; o corpo tenta trocar o dono para o B, o id e as duas datas.
    const a = await registerUser(app, userA);
    const tokenA = await loginUser(app, userA);
    const b = await registerUser(app, userB);
    const id = await createBackdatedTransaction(tokenA);
    const forgedId = "00000000-0000-4000-8000-000000000000";

    // Act
    const response = await patchTransaction(
      id,
      {
        amount: 2500,
        id: forgedId,
        userId: b.id,
        createdAt: "2000-01-01T00:00:00.000Z",
        updatedAt: "2000-01-01T00:00:00.000Z",
      },
      tokenA,
    );

    // Assert: só o amount mudou; o id e o createdAt são os de antes, e o updatedAt veio do banco (now()).
    expect(response.statusCode).toBe(200);
    expect(response.json()).toMatchObject({ id, amount: 2500, createdAt: past.toISOString() });
    expect(new Date(response.json().updatedAt).getTime()).toBeGreaterThan(past.getTime());
    const [row] = await db.select().from(transactions).where(eq(transactions.id, id));
    expect(row?.userId).toBe(a.id);
    expect(await db.select().from(transactions).where(eq(transactions.id, forgedId))).toHaveLength(0);
  });

  test("PATCH recusado por 404 (registro de outro usuário) não altera nada, nem o updatedAt", async () => {
    // Arrange
    const tokenA = await createUserWithToken(userA);
    const tokenB = await createUserWithToken(userB);
    const id = await createBackdatedTransaction(tokenA);
    const before = await listTransactions(tokenA);

    // Act
    const response = await patchTransaction(id, { amount: 2500 }, tokenB);

    // Assert
    expect(response.statusCode).toBe(404);
    expect(await listTransactions(tokenA)).toEqual(before);
    expect(before.transactions[0]).toMatchObject({ createdAt: past.toISOString(), updatedAt: past.toISOString() });
  });

  describe("ordem dos erros", () => {
    test(":id que não é UUID com corpo inválido → 404 (o id vem antes do corpo)", async () => {
      const token = await createUserWithToken(userA);

      const response = await patchTransaction("abc", { amount: 19.9 }, token);

      expect(response.statusCode).toBe(404);
      expect(response.json()).toEqual(NOT_FOUND);
    });

    test("id válido de outro usuário com corpo inválido → 400 (o corpo vem antes do dono)", async () => {
      // Arrange: o registro é do A; o B manda um corpo inválido.
      const tokenA = await createUserWithToken(userA);
      const tokenB = await createUserWithToken(userB);
      const { id } = await createTransaction(tokenA);

      // Act
      const response = await patchTransaction(id, { amount: 19.9 }, tokenB);

      // Assert
      expect(response.statusCode).toBe(400);
      expect(response.json()).toEqual({ error: AMOUNT_ERROR });
    });
  });
});
