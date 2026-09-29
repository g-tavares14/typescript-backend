import { DrizzleQueryError } from "drizzle-orm";
import type { FastifyError, FastifyReply, FastifyRequest } from "fastify";
import { INVALID_BODY } from "./validation.ts";

// Erros 4xx do Fastify (parse do corpo, tipo de conteúdo, tamanho, URL) chegam com mensagem em inglês.
// Aqui cada um vira uma mensagem em português, escolhida pelo error.code (estável entre versões), nunca pelo
// texto da mensagem. Nenhuma mensagem repete dados da requisição (URL, corpo).
const MESSAGES_BY_CODE = new Map([
  ["FST_ERR_CTP_EMPTY_JSON_BODY", INVALID_BODY],
  ["FST_ERR_CTP_INVALID_JSON_BODY", INVALID_BODY],
  ["FST_ERR_CTP_INVALID_CONTENT_LENGTH", INVALID_BODY],
  ["FST_ERR_CTP_INVALID_MEDIA_TYPE", "Tipo de conteúdo não suportado (use application/json)"], // 415
  ["FST_ERR_CTP_BODY_TOO_LARGE", "Corpo da requisição muito grande"], // 413
  ["FST_ERR_BAD_URL", "URL inválida"], // 400
]);

// Mensagem que o cliente recebe num erro 4xx.
// - code conhecido: a mensagem em português da tabela acima;
// - outro code FST_*: genérica, mantendo o status (o texto do Fastify é inglês e pode citar detalhes).
//   Só o código vai para o log, para descobrir qual erro merece uma mensagem própria (nunca a URL nem o corpo);
// - sem code FST_* (ex.: o 429 do rate limit, com a mensagem em português do errorResponseBuilder): a própria.
function clientErrorMessage(error: FastifyError, request: FastifyRequest): string {
  const known = MESSAGES_BY_CODE.get(error.code);
  if (known) {
    return known;
  }
  if (error.code?.startsWith("FST_")) {
    request.log.warn({ code: error.code }, "Erro 4xx sem mensagem mapeada");
    return "Requisição inválida";
  }
  return error.message;
}

// A resposta de todo erro da API, no formato { error }. É usada nas duas portas por onde um erro chega:
// o setErrorHandler (erros das rotas e dos hooks) e o frameworkErrors (erros que o Fastify gera antes de escolher
// a rota), que não passam pelo setErrorHandler. Uma função só garante a mesma regra nas duas:
// 5xx: log completo e mensagem genérica; 4xx: mensagem em português (sem log, salvo o caso acima).
export function sendError(error: FastifyError, request: FastifyRequest, reply: FastifyReply) {
  const statusCode = error.statusCode ?? 500;
  if (statusCode >= 500) {
    // O DrizzleQueryError traz os parâmetros da consulta (email, hash da senha...).
    // Para o log, registra só a consulta e o erro original do banco.
    if (error instanceof DrizzleQueryError) {
      request.log.error({ query: error.query, err: error.cause }, "Erro no banco de dados");
    } else {
      request.log.error(error);
    }
    return reply.code(500).send({ error: "Erro interno do servidor" });
  }
  return reply.code(statusCode).send({ error: clientErrorMessage(error, request) });
}
