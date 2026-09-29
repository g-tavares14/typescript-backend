import type { FastifyPluginAsync } from "fastify";
import type { Db } from "../db/client.ts";
import { currentUser, requireAuth } from "../lib/authenticate.ts";

export const usersRoutes: FastifyPluginAsync<{ db: Db }> = async (app, { db }) => {
  // Todas as rotas deste plugin exigem login. Hook em plugin sem fastify-plugin fica encapsulado nele.
  app.addHook("onRequest", requireAuth(db));

  app.get("/me", async (request, reply) => {
    return reply.send(currentUser(request));
  });
};
