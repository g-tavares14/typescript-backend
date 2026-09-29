import rateLimit from "@fastify/rate-limit";
import Fastify, { type FastifyError } from "fastify";
import type { Db } from "./db/client.ts";
import { sendError } from "./lib/errors.ts";
import { authRoutes } from "./routes/auth.ts";
import { healthRoutes } from "./routes/health.ts";
import { transactionsRoutes } from "./routes/transactions.ts";
import { usersRoutes } from "./routes/users.ts";

// rateLimit: false existe só para os testes (ver test/helpers.ts). O server.ts não passa a opção: fica ligado.
export function buildApp(db: Db, options: { logger?: boolean; rateLimit?: boolean } = {}) {
  // Todo erro sai por sendError (src/lib/errors.ts): 4xx com mensagem em português; 5xx completo no log e
  // mensagem genérica para o cliente (sem detalhes internos).
  // O frameworkErrors cobre os erros que o Fastify gera antes de escolher a rota (URL malformada, constraint
  // assíncrona com falha): eles não passam pelo setErrorHandler.
  const app = Fastify({ logger: options.logger ?? true, frameworkErrors: sendError });
  app.setErrorHandler<FastifyError>(sendError);

  // Rota ou método inexistente. Não repete a URL pedida na resposta.
  app.setNotFoundHandler((_request, reply) => reply.code(404).send({ error: "Rota não encontrada" }));

  // Campo do usuário autenticado em toda requisição, preenchido pelo requireAuth (src/lib/authenticate.ts).
  // Começa como null (valor simples): o Fastify proíbe objeto aqui porque seria compartilhado entre requisições.
  app.decorateRequest("user", null);

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
  app.register(transactionsRoutes, { prefix: "/transactions", db });

  return app;
}
