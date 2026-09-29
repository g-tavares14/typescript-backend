import type { FastifyPluginAsync } from "fastify";
import { z } from "zod";
import type { Db } from "../db/client.ts";
import { transactions } from "../db/schema.ts";
import { authenticate, unauthorized } from "../lib/authenticate.ts";
import { badRequest, required } from "../lib/validation.ts";

// Regras da spec (SPEC-transactions.md). Por enquanto toda regra quebrada responde a mensagem genérica
// `required`; as mensagens específicas de cada campo entram na T3.
// O z.object ignora campos que não estão aqui: um userId, id ou createdAt no corpo nunca chega ao insert.
const createTransactionSchema = z.object({
  type: z.enum(["income", "expense"], required),
  // Centavos inteiros: 1990 = R$ 19,90. Rejeita 19.9, "1990", 0 e negativos; teto de R$ 1 bilhão.
  amount: z
    .number(required)
    .int(required.error)
    .positive(required.error)
    .max(100_000_000_000, required.error),
  description: z.string(required).trim().min(1, required.error).max(200, required.error),
  // AAAA-MM-DD válida (rejeita 2026-02-30). Continua string: sem Date, sem problema de fuso.
  date: z.iso.date(required),
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

export const transactionsRoutes: FastifyPluginAsync<{ db: Db }> = async (app, { db }) => {
  app.post("/", async (request, reply) => {
    const user = await authenticate(request, db);
    if (!user) {
      return unauthorized(reply);
    }

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
};
