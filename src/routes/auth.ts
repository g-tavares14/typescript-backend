import { eq, sql } from "drizzle-orm";
import type { FastifyPluginAsync } from "fastify";
import { z } from "zod";
import type { Db } from "../db/client.ts";
import { users } from "../db/schema.ts";
import { currentUser, requireAuth } from "../lib/authenticate.ts";
import { isUniqueViolation } from "../lib/db-errors.ts";
import { hashPassword, simulatePasswordVerification, verifyPassword } from "../lib/password.ts";
import { ACCESS_TOKEN_TTL_SECONDS, createAccessToken } from "../lib/token.ts";
import { DUPLICATE_USER, emailSchema, passwordSchema, usernameSchema } from "../lib/user-fields.ts";
import { badRequest, INVALID_BODY, required } from "../lib/validation.ts";

// Validação do corpo da requisição. As regras de username, email e senha são compartilhadas (src/lib/user-fields.ts).
const registerSchema = z.object({
  username: usernameSchema,
  email: emailSchema,
  password: passwordSchema,
}, INVALID_BODY);

// No login a senha não tem tamanho mínimo: a regra de 8 caracteres é do cadastro.
// Se ela mudar no futuro, contas antigas com senhas menores continuam conseguindo entrar.
const loginSchema = z.object({
  email: emailSchema,
  password: z.string(required).min(1, "Campo obrigatório ausente ou inválido"),
}, INVALID_BODY);

// Limites por IP em login e cadastro, contra força bruta e consumo de memória:
// cada hash argon2 usa 64 MiB de RAM, então muitas requisições simultâneas derrubariam o servidor.
// O cadastro é mais restrito porque sempre calcula um hash e criar contas em massa não tem uso legítimo.
const LOGIN_RATE_LIMIT = { max: 5, timeWindow: "1 minute" };
const REGISTER_RATE_LIMIT = { max: 3, timeWindow: "1 minute" };

// A mesma mensagem para "email não existe" e "senha errada": não revela quais emails têm conta.
const INVALID_CREDENTIALS = "Email ou senha inválidos";

export const authRoutes: FastifyPluginAsync<{ db: Db }> = async (app, { db }) => {
  app.post("/register", { config: { rateLimit: REGISTER_RATE_LIMIT } }, async (request, reply) => {
    const parsed = registerSchema.safeParse(request.body);
    if (!parsed.success) {
      return badRequest(reply, parsed.error);
    }

    const { username, email, password } = parsed.data;
    const passwordHash = await hashPassword(password);

    try {
      // id, role e createdAt ficam com os valores padrão do banco.
      const [user] = await db
        .insert(users)
        .values({ username, email, passwordHash })
        .returning({ id: users.id, username: users.username });

      return reply.code(201).send(user);
    } catch (error) {
      if (isUniqueViolation(error)) {
        return reply.code(409).send({ error: DUPLICATE_USER });
      }
      throw error; // vira 500 genérico no error handler do app.ts
    }
  });

  app.post("/login", { config: { rateLimit: LOGIN_RATE_LIMIT } }, async (request, reply) => {
    const parsed = loginSchema.safeParse(request.body);
    if (!parsed.success) {
      return badRequest(reply, parsed.error);
    }

    const { email, password } = parsed.data;

    const [user] = await db
      .select({ id: users.id, tokenVersion: users.tokenVersion, passwordHash: users.passwordHash })
      .from(users)
      .where(eq(users.email, email))
      .limit(1);

    if (!user) {
      await simulatePasswordVerification(password);
      return reply.code(401).send({ error: INVALID_CREDENTIALS });
    }

    if (!(await verifyPassword(user.passwordHash, password))) {
      return reply.code(401).send({ error: INVALID_CREDENTIALS });
    }

    const token = await createAccessToken(user);
    return reply.send({ token, tokenType: "Bearer", expiresIn: ACCESS_TOKEN_TTL_SECONDS });
  });

  // Logout em todos os dispositivos: subir a versão invalida todos os tokens já emitidos para o usuário.
  // O requireAuth é só desta rota: register e login continuam públicos.
  app.post("/logout", { onRequest: requireAuth(db) }, async (request, reply) => {
    const user = currentUser(request);

    // O incremento é feito pelo banco (token_version + 1), não lendo o valor e somando em JS:
    // assim, dois logouts simultâneos não se atropelam e cada um conta.
    await db
      .update(users)
      .set({ tokenVersion: sql`${users.tokenVersion} + 1` })
      .where(eq(users.id, user.id));

    return reply.code(204).send();
  });
};
