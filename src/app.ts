import { DrizzleQueryError } from "drizzle-orm";
import Fastify, { type FastifyError } from "fastify";
import type { Db } from "./db/client.ts";
import { authRoutes } from "./routes/auth.ts";
import { healthRoutes } from "./routes/health.ts";

export function buildApp(db: Db, options: { logger?: boolean } = {}) {
  const app = Fastify({ logger: options.logger ?? true });

  // Erros 4xx mostram a mensagem; erros 5xx vão completos para o log
  // e o cliente recebe só uma mensagem genérica (sem detalhes internos).
  app.setErrorHandler<FastifyError>((error, request, reply) => {
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
    return reply.code(statusCode).send({ error: error.message });
  });

  app.register(healthRoutes, { prefix: "/health", db });
  app.register(authRoutes, { prefix: "/auth", db });

  return app;
}
