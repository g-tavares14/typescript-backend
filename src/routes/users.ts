import { eq } from "drizzle-orm";
import type { FastifyPluginAsync } from "fastify";
import { z } from "zod";
import type { Db } from "../db/client.ts";
import { users } from "../db/schema.ts";
import { currentUser, publicUserColumns, requireAuth, unauthorized } from "../lib/authenticate.ts";
import { isUniqueViolation } from "../lib/db-errors.ts";
import { DUPLICATE_USER, emailSchema, usernameSchema } from "../lib/user-fields.ts";
import { badRequest, INVALID_BODY } from "../lib/validation.ts";

// Só username e email podem mudar aqui: chaves desconhecidas (role, password...) são descartadas pelo z.object.
// `.partial()` deixa cada campo opcional; o `.refine` barra o corpo sem nenhum, porque `.set({})` do Drizzle lança erro.
const updateUserSchema = z
  .object({ username: usernameSchema, email: emailSchema }, INVALID_BODY)
  .partial()
  .refine((body) => Object.keys(body).length > 0, "Envie ao menos um campo para alterar");

export const usersRoutes: FastifyPluginAsync<{ db: Db }> = async (app, { db }) => {
  // Todas as rotas deste plugin exigem login. Hook em plugin sem fastify-plugin fica encapsulado nele.
  app.addHook("onRequest", requireAuth(db));

  app.get("/me", async (request, reply) => {
    return reply.send(currentUser(request));
  });

  app.patch("/me", async (request, reply) => {
    const user = currentUser(request);

    const parsed = updateUserSchema.safeParse(request.body);
    if (!parsed.success) {
      return badRequest(reply, parsed.error);
    }

    try {
      // Uma consulta só: o id vem do token, nunca do corpo. `returning` já devolve o usuário no formato do GET.
      const [updated] = await db
        .update(users)
        .set(parsed.data)
        .where(eq(users.id, user.id))
        .returning(publicUserColumns);

      // 0 linhas: a conta foi apagada entre o requireAuth e este UPDATE. Para o cliente, é o mesmo 401 de sempre.
      if (!updated) {
        return unauthorized(reply);
      }
      return reply.send(updated);
    } catch (error) {
      if (isUniqueViolation(error)) {
        return reply.code(409).send({ error: DUPLICATE_USER });
      }
      throw error; // vira 500 genérico no error handler do app.ts
    }
  });
};
