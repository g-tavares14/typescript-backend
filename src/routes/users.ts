import type { FastifyPluginAsync } from "fastify";
import type { Db } from "../db/client.ts";
import { authenticate, unauthorized } from "../lib/authenticate.ts";

export const usersRoutes: FastifyPluginAsync<{ db: Db }> = async (app, { db }) => {
  app.get("/me", async (request, reply) => {
    const user = await authenticate(request, db);
    if (!user) {
      return unauthorized(reply);
    }
    return reply.send(user);
  });
};
