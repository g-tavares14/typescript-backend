import { DrizzleQueryError, eq, sql } from "drizzle-orm";
import type { FastifyPluginAsync, FastifyReply } from "fastify";
import pg from "pg";
import { z } from "zod";
import type { Db } from "../db/client.ts";
import { users } from "../db/schema.ts";
import { authenticate, unauthorized } from "../lib/authenticate.ts";
import { hashPassword, simulatePasswordVerification, verifyPassword } from "../lib/password.ts";
import { ACCESS_TOKEN_TTL_SECONDS, createAccessToken } from "../lib/token.ts";

// Validação e normalização do corpo da requisição.
// O trim/toLowerCase roda antes da validação do email e do username; a senha não é alterada.
const required = { error: "Campo obrigatório ausente ou inválido" };

const emailSchema = z.string(required).trim().toLowerCase().pipe(z.email("Email inválido"));

const registerSchema = z.object({
  username: z
    .string(required)
    .trim()
    .toLowerCase()
    .min(3, "O username deve ter entre 3 e 50 caracteres")
    .max(50, "O username deve ter entre 3 e 50 caracteres")
    // Só a-z, 0-9 e _: barra acento, espaço, letras de outros alfabetos e caracteres invisíveis.
    // Junto com o toLowerCase, "Joao" e "joao" viram o mesmo username (o UNIQUE do banco pega o duplicado).
    // Vem depois dos checks de tamanho, então um username curto continua recebendo a mensagem de tamanho.
    .regex(/^[a-z0-9_]+$/, "O username só pode ter letras sem acento, números e _"),
  email: emailSchema,
  password: z.string(required).min(8, "A senha deve ter no mínimo 8 caracteres"),
});

// No login a senha não tem tamanho mínimo: a regra de 8 caracteres é do cadastro.
// Se ela mudar no futuro, contas antigas com senhas menores continuam conseguindo entrar.
const loginSchema = z.object({
  email: emailSchema,
  password: z.string(required).min(1, "Campo obrigatório ausente ou inválido"),
});

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
        return reply.code(409).send({ error: "Email ou username já cadastrado" });
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
  app.post("/logout", async (request, reply) => {
    const user = await authenticate(request, db);
    if (!user) {
      return unauthorized(reply);
    }

    // O incremento é feito pelo banco (token_version + 1), não lendo o valor e somando em JS:
    // assim, dois logouts simultâneos não se atropelam e cada um conta.
    await db
      .update(users)
      .set({ tokenVersion: sql`${users.tokenVersion} + 1` })
      .where(eq(users.id, user.id));

    return reply.code(204).send();
  });
};

// 23505 é o código do Postgres para violação de UNIQUE.
// O Drizzle embrulha o erro do driver em DrizzleQueryError, e o erro original fica em .cause.
function isUniqueViolation(error: unknown): boolean {
  return (
    error instanceof DrizzleQueryError &&
    error.cause instanceof pg.DatabaseError &&
    error.cause.code === "23505"
  );
}

// A API responde só a primeira mensagem de validação.
function badRequest(reply: FastifyReply, error: z.ZodError) {
  return reply.code(400).send({ error: error.issues[0]?.message ?? "Dados inválidos" });
}
