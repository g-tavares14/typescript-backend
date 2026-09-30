import { eq } from "drizzle-orm";
import type { FastifyPluginAsync } from "fastify";
import { z } from "zod";
import type { Db } from "../db/client.ts";
import { users } from "../db/schema.ts";
import { currentUser, publicUserColumns, requireAuth, unauthorized } from "../lib/authenticate.ts";
import { isUniqueViolation } from "../lib/db-errors.ts";
import { verifyPassword } from "../lib/password.ts";
import { DUPLICATE_USER, emailSchema, usernameSchema } from "../lib/user-fields.ts";
import { badRequest, INVALID_BODY, required } from "../lib/validation.ts";

// Só username e email podem mudar aqui: chaves desconhecidas (role, password...) são descartadas pelo z.object.
// `.partial()` deixa cada campo opcional; o `.refine` barra o corpo sem nenhum, porque `.set({})` do Drizzle lança erro.
const updateUserSchema = z
  .object({ username: usernameSchema, email: emailSchema }, INVALID_BODY)
  .partial()
  .refine((body) => Object.keys(body).length > 0, "Envie ao menos um campo para alterar");

// Confirmação de senha para excluir a conta. Sem tamanho mínimo, como no login: a regra de 8 é só do cadastro.
const deleteUserSchema = z.object({ password: z.string(required).min(1, required.error) }, INVALID_BODY);

// Limites por IP (contadores em memória, como no auth.ts). O DELETE é o mais restrito: com token roubado, é a rota
// que permite adivinhar a senha, e cada tentativa custa um hash argon2 (64 MiB). O hook do limite é da rota, então
// roda depois do requireAuth do plugin: sem token vem 401 e a requisição não consome o limite.
const DELETE_RATE_LIMIT = { max: 5, timeWindow: "1 minute" };
const UPDATE_RATE_LIMIT = { max: 10, timeWindow: "1 minute" };

export const usersRoutes: FastifyPluginAsync<{ db: Db }> = async (app, { db }) => {
  // Todas as rotas deste plugin exigem login. Hook em plugin sem fastify-plugin fica encapsulado nele.
  app.addHook("onRequest", requireAuth(db));

  app.get("/me", async (request, reply) => {
    return reply.send(currentUser(request));
  });

  app.patch("/me", { config: { rateLimit: UPDATE_RATE_LIMIT } }, async (request, reply) => {
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

  // Exclusão definitiva, com a senha como confirmação (um token roubado sozinho não apaga a conta).
  // As transações do usuário somem junto (ON DELETE CASCADE no banco).
  app.delete("/me", { config: { rateLimit: DELETE_RATE_LIMIT } }, async (request, reply) => {
    const user = currentUser(request);

    const parsed = deleteUserSchema.safeParse(request.body);
    if (!parsed.success) {
      return badRequest(reply, parsed.error);
    }

    // O hash é lido só aqui: o currentUser() não o carrega, para ele nunca chegar perto de uma resposta.
    const [row] = await db
      .select({ passwordHash: users.passwordHash })
      .from(users)
      .where(eq(users.id, user.id))
      .limit(1);
    if (!row) {
      return unauthorized(reply); // conta apagada depois do requireAuth
    }

    // 403 (e não 401): o token é válido, o que falhou foi a confirmação. O 401 faria o front deslogar o usuário.
    if (!(await verifyPassword(row.passwordHash, parsed.data.password))) {
      return reply.code(403).send({ error: "Senha incorreta" });
    }

    const deleted = await db.delete(users).where(eq(users.id, user.id)).returning({ id: users.id });
    if (deleted.length === 0) {
      return unauthorized(reply); // apagada por outra requisição entre o SELECT e o DELETE
    }
    return reply.code(204).send();
  });
};
