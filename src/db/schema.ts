import { sql } from "drizzle-orm";
import {
  bigint,
  check,
  date,
  index,
  integer,
  pgTable,
  text,
  timestamp,
  uuid,
  varchar,
} from "drizzle-orm/pg-core";

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

// Registros financeiros (entradas e saídas). Cada registro pertence a um usuário e some junto com ele.
export const transactions = pgTable(
  "transactions",
  {
    id: uuid("id").primaryKey().defaultRandom(),
    userId: uuid("user_id")
      .notNull()
      .references(() => users.id, { onDelete: "cascade" }),
    type: text("type", { enum: ["income", "expense"] }).notNull(),
    // Dinheiro em centavos inteiros, nunca float. mode "number": seguro até 2^53, bem acima do limite da API.
    amountCents: bigint("amount_cents", { mode: "number" }).notNull(),
    description: varchar("description", { length: 200 }).notNull(),
    // Só o dia ("AAAA-MM-DD"), sem hora nem fuso. O modo padrão devolve string, sem conversão para Date.
    occurredOn: date("occurred_on").notNull(),
    createdAt: timestamp("created_at", { withTimezone: true }).notNull().defaultNow(),
    // Última alteração do registro. O banco só preenche na criação; a API é quem atualiza a cada edição.
    updatedAt: timestamp("updated_at", { withTimezone: true }).notNull().defaultNow(),
  },
  (table) => [
    // Defesa em profundidade: repetem regras que a API já valida.
    check("transactions_type_check", sql`${table.type} IN ('income', 'expense')`),
    check("transactions_amount_cents_check", sql`${table.amountCents} > 0`),
    // Toda consulta filtra por usuário e filtra/ordena por data.
    index("transactions_user_id_occurred_on_idx").on(table.userId, table.occurredOn),
  ],
);

export type Transaction = typeof transactions.$inferSelect;
