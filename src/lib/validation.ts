import type { FastifyReply } from "fastify";
import type { z } from "zod";

// Mensagem para campo ausente ou com tipo errado. É o parâmetro `error` do Zod: z.string(required).
// Fica aqui porque cadastro, login e registros financeiros usam exatamente a mesma.
export const required = { error: "Campo obrigatório ausente ou inválido" };

// A API responde só a primeira mensagem de validação.
export function badRequest(reply: FastifyReply, error: z.ZodError) {
  return reply.code(400).send({ error: error.issues[0]?.message ?? "Dados inválidos" });
}
