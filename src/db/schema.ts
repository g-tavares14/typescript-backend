import { sql } from "drizzle-orm";
import { check, integer, pgTable, text, timestamp, uuid, varchar } from "drizzle-orm/pg-core";

// A tabela é definida aqui, em TypeScript. O drizzle-kit gera as migrations SQL a partir deste arquivo.
export const users = pgTable(
  "users",
  {
    id: uuid("id").primaryKey().defaultRandom(),
    username: varchar("username", { length: 50 }).notNull().unique(),
    email: text("email").notNull().unique(),
    passwordHash: text("password_hash").notNull(),
    role: text("role", { enum: ["user", "admin"] }).notNull().default("user"),
    // Versão dos tokens do usuário: o JWT carrega o valor de quando foi emitido e só vale se for igual ao atual.
    // Incrementar esta coluna invalida todos os tokens já emitidos (usado no logout).
    tokenVersion: integer("token_version").notNull().default(0),
    createdAt: timestamp("created_at", { withTimezone: true }).notNull().defaultNow(),
  },
  (table) => [
    check("users_role_check", sql`${table.role} IN ('user', 'admin')`),
    // Defesa em profundidade: a API já normaliza o username, mas o banco garante o formato
    // mesmo para quem inserir sem passar por ela. Como só aceita minúsculas, o UNIQUE vira case-insensitive.
    check("users_username_format_check", sql`${table.username} ~ '^[a-z0-9_]{3,50}$'`),
  ],
);

export type User = typeof users.$inferSelect;
