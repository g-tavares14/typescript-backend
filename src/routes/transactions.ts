import { and, desc, eq, gte, lte, sql } from "drizzle-orm";
import type { FastifyPluginAsync } from "fastify";
import { z } from "zod";
import type { Db } from "../db/client.ts";
import { transactions } from "../db/schema.ts";
import { currentUser, requireAuth } from "../lib/authenticate.ts";
import { badRequest, required } from "../lib/validation.ts";

const AMOUNT_ERROR = "O valor deve ser um número inteiro de centavos maior que zero";
const DESCRIPTION_ERROR = "A descrição deve ter entre 1 e 200 caracteres";
const DATE_ERROR = "Data inválida (use AAAA-MM-DD)";

// AAAA-MM-DD válida (rejeita 2026-02-30). Continua string: sem Date, sem problema de fuso.
// Usada no corpo do POST e na query do GET.
const dateField = z.string(required).pipe(z.iso.date(DATE_ERROR));

// Regras da spec (SPEC-transactions.md), com a mesma convenção do cadastro:
// campo ausente ou com tipo JSON errado -> `required`; tipo certo mas valor fora da regra -> mensagem do campo.
// Nos campos de texto isso vem do z.string(required).pipe(...): o pipe só chega na segunda etapa se o valor
// for uma string, então a mensagem específica nunca aparece para `type: 123` ou `date: 20260929`.
// O z.object ignora campos que não estão aqui: um userId, id ou createdAt no corpo nunca chega ao insert.
const createTransactionSchema = z.object({
  type: z.string(required).pipe(z.enum(["income", "expense"], "O tipo deve ser income ou expense")),
  // Centavos inteiros: 1990 = R$ 19,90. Rejeita 19.9, 0 e negativos; teto de R$ 1 bilhão.
  // "1990" (string) cai no z.number(required): é tipo errado, não valor inválido.
  amount: z
    .number(required)
    .int(AMOUNT_ERROR)
    .positive(AMOUNT_ERROR)
    .max(100_000_000_000, AMOUNT_ERROR),
  description: z
    .string(required)
    .trim()
    .min(1, DESCRIPTION_ERROR)
    .max(200, DESCRIPTION_ERROR),
  date: dateField,
});

// Filtro do GET: as duas datas são opcionais e inclusivas. Mesma convenção do corpo do POST: tipo errado
// (ex.: ?from=a&from=b, que o Fastify entrega como array) -> `required`; string que não é data -> DATE_ERROR.
// O refine só roda depois que from e to passaram no dateField (com um campo inválido sai o DATE_ERROR dele),
// e AAAA-MM-DD ordena igual como texto e como data: por isso a comparação from <= to é direta, sem Date.
const listQuerySchema = z
  .object({ from: dateField.optional(), to: dateField.optional() })
  .refine(({ from, to }) => !from || !to || from <= to, {
    error: "A data inicial deve ser anterior ou igual à final",
  });

// Colunas que a API expõe (nunca o user_id). Aqui os nomes do banco viram os da API.
const publicColumns = {
  id: transactions.id,
  type: transactions.type,
  amount: transactions.amountCents,
  description: transactions.description,
  date: transactions.occurredOn,
  createdAt: transactions.createdAt,
};

// Soma, no banco, os valores de um tipo. O sum de bigint volta como numeric, que o driver pg entrega como
// string ("525000"), e sem linhas o sum é NULL. Por isso: coalesce(..., 0) e mapWith(Number) para virar number.
// Number() é exato aqui: a soma só perde precisão acima de 2^53 centavos (~R$ 90 trilhões), e cada registro
// tem no máximo R$ 1 bilhão.
function totalOf(type: "income" | "expense") {
  const total = sql`coalesce(sum(${transactions.amountCents}) filter (where ${transactions.type} = ${type}), 0)`;
  return total.mapWith(Number);
}

export const transactionsRoutes: FastifyPluginAsync<{ db: Db }> = async (app, { db }) => {
  // As duas rotas exigem login: o hook autentica antes de cada handler e o 401 sai dele.
  app.addHook("onRequest", requireAuth(db));

  app.post("/", async (request, reply) => {
    const user = currentUser(request);

    const parsed = createTransactionSchema.safeParse(request.body);
    if (!parsed.success) {
      return badRequest(reply, parsed.error);
    }

    const { type, amount, description, date } = parsed.data;

    // userId vem do token, nunca do corpo. id e createdAt ficam com os valores padrão do banco.
    const [transaction] = await db
      .insert(transactions)
      .values({ userId: user.id, type, amountCents: amount, description, occurredOn: date })
      .returning(publicColumns);

    return reply.code(201).send(transaction);
  });

  app.get("/", async (request, reply) => {
    const user = currentUser(request);

    const parsed = listQuerySchema.safeParse(request.query);
    if (!parsed.success) {
      return badRequest(reply, parsed.error);
    }
    const { from, to } = parsed.data;

    // Uma só condição para a lista e para os totais, então os dois sempre usam o mesmo filtro.
    // Toda consulta filtra por user_id (vindo do token): é o que isola um usuário do outro.
    // O and() ignora os undefined: sem from/to, sobra só o filtro por usuário.
    const filter = and(
      eq(transactions.userId, user.id),
      from ? gte(transactions.occurredOn, from) : undefined,
      to ? lte(transactions.occurredOn, to) : undefined,
    );

    const [list, [totals]] = await Promise.all([
      db
        .select(publicColumns)
        .from(transactions)
        .where(filter)
        .orderBy(desc(transactions.occurredOn), desc(transactions.createdAt)),
      db
        .select({ income: totalOf("income"), expense: totalOf("expense") })
        .from(transactions)
        .where(filter),
    ]);

    // Sem GROUP BY, o agregado sempre devolve exatamente uma linha (com 0 quando não há registros).
    const { income, expense } = totals!;
    return reply.send({
      summary: { income, expense, balance: income - expense },
      transactions: list,
    });
  });
};
