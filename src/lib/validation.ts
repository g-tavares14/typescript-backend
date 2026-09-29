import type { FastifyReply } from "fastify";
import type { z } from "zod";

// Mensagem para campo ausente ou com tipo errado. É o parâmetro `error` do Zod: z.string(required).
// Fica aqui porque cadastro, login e registros financeiros usam exatamente a mesma.
export const required = { error: "Campo obrigatório ausente ou inválido" };

// Corpo que não é um objeto JSON utilizável: ausente, vazio, malformado, `null`, array, texto...
// Usada de dois jeitos: na raiz dos z.object de corpo (o `error` de z.object só vale para o tipo da raiz; as
// mensagens dos campos continuam as de cada campo) e no mapeamento dos erros de parse do Fastify (src/lib/errors.ts).
export const INVALID_BODY = "Corpo da requisição inválido: envie um objeto JSON";

// A API responde só a primeira mensagem de validação.
export function badRequest(reply: FastifyReply, error: z.ZodError) {
  return reply.code(400).send({ error: error.issues[0]?.message ?? "Dados inválidos" });
}
