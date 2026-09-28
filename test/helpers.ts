import { sql } from "drizzle-orm";
import { buildApp } from "../src/app.ts";
import { createDb, type Db } from "../src/db/client.ts";

// Cria o app de verdade (mesmas rotas, mesmo error handler), ligado ao banco de testes.
// Os testes chamam as rotas com app.inject(), sem abrir porta de rede.
export function createTestApp() {
  const db = createDb(process.env.DATABASE_URL ?? "");
  const app = buildApp(db, { logger: false });
  return { app, db };
}

// Libera o servidor e as conexões com o banco no fim dos testes.
export async function closeTestApp(app: ReturnType<typeof buildApp>, db: Db) {
  await app.close();
  await db.$client.end();
}

// Apaga todos os usuários, para cada teste começar do zero.
export async function resetDatabase(db: Db) {
  await db.execute(sql`TRUNCATE TABLE users`);
}
