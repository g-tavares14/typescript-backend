import rateLimit from "@fastify/rate-limit";
import { DrizzleQueryError } from "drizzle-orm";
import Fastify, { type FastifyError } from "fastify";
import type { Db } from "./db/client.ts";
import { authRoutes } from "./routes/auth.ts";
import { healthRoutes } from "./routes/health.ts";
import { usersRoutes } from "./routes/users.ts";

// rateLimit: false existe só para os testes (ver test/helpers.ts). O server.ts não passa a opção: fica ligado.
export function buildApp(db: Db, options: { logger?: boolean; rateLimit?: boolean } = {}) {
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

  // global: false = nenhuma rota é limitada por padrão; só as que declaram config.rateLimit (login e cadastro).
  // O plugin lança o objeto do errorResponseBuilder como erro, então o 429 passa pelo error handler acima
  // e sai no formato { error }: só a mensagem é trocada para português.
  // Precisa ser registrado ANTES das rotas: o plugin usa o hook onRoute para ativar o limite em cada rota
  // no momento em que ela é declarada, então rotas declaradas antes de o plugin carregar ficam sem limite.
  if (options.rateLimit !== false) {
    app.register(rateLimit, {
      global: false,
      errorResponseBuilder: () => ({
        statusCode: 429,
        message: "Muitas tentativas. Tente novamente mais tarde.",
      }),
    });
  }

  app.register(healthRoutes, { prefix: "/health", db });
  app.register(authRoutes, { prefix: "/auth", db });
  app.register(usersRoutes, { prefix: "/users", db });

  return app;
}
