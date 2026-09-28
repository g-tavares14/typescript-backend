import type { FastifyPluginAsync } from "fastify";
import { z } from "zod";
import type { Db } from "../db/client.ts";
import { users } from "../db/schema.ts";
import { hashPassword } from "../lib/password.ts";

// Validação e normalização do corpo da requisição.
// O trim/toLowerCase roda antes da validação do email; a senha não é alterada.
const required = { error: "Campo obrigatório ausente ou inválido" };

const registerSchema = z.object({
  username: z
    .string(required)
    .trim()
    .min(3, "O username deve ter entre 3 e 50 caracteres")
    .max(50, "O username deve ter entre 3 e 50 caracteres"),
  email: z.string(required).trim().toLowerCase().pipe(z.email("Email inválido")),
  password: z.string(required).min(8, "A senha deve ter no mínimo 8 caracteres"),
});

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
};

// 23505 é o código do Postgres para violação de UNIQUE.
// O Drizzle embrulha o erro do driver, então o código fica em error.cause.
function isUniqueViolation(error: unknown): boolean {
  const pgError = error instanceof Error && error.cause ? error.cause : error;
  return (
    typeof pgError === "object" && pgError !== null && "code" in pgError && pgError.code === "23505"
  );
}
