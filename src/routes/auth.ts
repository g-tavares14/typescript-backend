import { eq } from "drizzle-orm";
import type { FastifyPluginAsync } from "fastify";
import { z } from "zod";
import type { Db } from "../db/client.ts";
import { users } from "../db/schema.ts";
import { hashPassword, simulatePasswordVerification, verifyPassword } from "../lib/password.ts";
import { ACCESS_TOKEN_TTL_SECONDS, createAccessToken, verifyAccessToken } from "../lib/token.ts";
import * as repl from "node:repl";

// Validação e normalização do corpo da requisição.
// O trim/toLowerCase roda antes da validação do email; a senha não é alterada.
const required = { error: "Campo obrigatório ausente ou inválido" };

const emailSchema = z.string(required).trim().toLowerCase().pipe(z.email("Email inválido"));

const registerSchema = z.object({
  username: z
    .string(required)
    .trim()
    .min(3, "O username deve ter entre 3 e 50 caracteres")
    .max(50, "O username deve ter entre 3 e 50 caracteres"),
  email: emailSchema,
  password: z.string(required).min(8, "A senha deve ter no mínimo 8 caracteres"),
});

// No login a senha não tem tamanho mínimo: a regra de 8 caracteres é do cadastro.
// Se ela mudar no futuro, contas antigas com senhas menores continuam conseguindo entrar.
const loginSchema = z.object({
  email: emailSchema,
  password: z.string(required).min(1, "Campo obrigatório ausente ou inválido"),
});

// A mesma mensagem para "email não existe" e "senha errada": não revela quais emails têm conta.
const INVALID_CREDENTIALS = "Email ou senha inválidos";

export const authRoutes: FastifyPluginAsync<{ db: Db }> = async (app, { db }) => {
  app.post("/register", async (request, reply) => {
    const parsed = registerSchema.safeParse(request.body);
    if (!parsed.success) {
      return reply.code(400).send({ error: parsed.error.issues[0]?.message ?? "Dados inválidos" });
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

  app.post("/login", async (request, reply) => {
    const parsed = loginSchema.safeParse(request.body);
    if (!parsed.success) {
      return reply.code(400).send({ error: parsed.error.issues[0]?.message ?? "Dados inválidos" });
    }

    const { email, password } = parsed.data;

    const [user] = await db
      .select({ id: users.id, role: users.role, passwordHash: users.passwordHash })
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
    return reply.send({ token, tokenType: "Bearer ", expiresIn: ACCESS_TOKEN_TTL_SECONDS });
  });

  app.get("/me", async (request, reply) => {
    const header = request.headers.authorization;

    if (!header || !header.startsWith("Bearer ")) {
      return reply.code(401).header("WWW-Authenticate", "Bearer ").send({ error: "Não autenticado"})
    }

    const token = header.slice("Bearer ".length);

    try {
      const { userId, role } = await verifyAccessToken(token);
      return reply.send({ userId, role})
  } catch {
      return reply.code(401).header("WWW-Authenticate", "Bearer ").send({ error: "Não autenticado"})
  }
});

// 23505 é o código do Postgres para violação de UNIQUE.
// O Drizzle embrulha o erro do driver, então o código fica em error.cause.
function isUniqueViolation(error: unknown): boolean {
  const pgError = error instanceof Error && error.cause ? error.cause : error;
  return (
    typeof pgError === "object" && pgError !== null && "code" in pgError && pgError.code === "23505"
  );
}}