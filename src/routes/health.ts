import { sql } from "drizzle-orm";
import type { FastifyPluginAsync } from "fastify";
import type { Db } from "../db/client.ts";

export const healthRoutes: FastifyPluginAsync<{ db: Db }> = async (app, { db }) => {
  // 200 se o banco responde, 503 se não.
  app.get("/", async (_request, reply) => {
    try {
      await db.execute(sql`SELECT 1`);
      return reply.code(200).send();
    } catch {
      return reply.code(503).send();
    }
  });
};
