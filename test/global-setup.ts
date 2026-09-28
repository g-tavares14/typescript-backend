import { drizzle } from "drizzle-orm/node-postgres";
import { migrate } from "drizzle-orm/node-postgres/migrator";
import pg from "pg";

// Roda uma vez antes de todos os testes: cria o banco de testes (se não existir) e aplica as migrations.
export default async function setup() {
  const url = new URL(process.env.DATABASE_URL ?? "");
  const databaseName = url.pathname.slice(1);
  if (!databaseName.endsWith("_test")) {
    throw new Error(`Os testes só podem rodar num banco terminado em "_test" (recebido: ${databaseName})`);
  }

  // Conecta no banco padrão "postgres" só para criar o banco de testes.
  const adminUrl = new URL(url);
  adminUrl.pathname = "/postgres";
  const admin = new pg.Client({ connectionString: adminUrl.toString() });
  await admin.connect();
  const { rowCount } = await admin.query("SELECT 1 FROM pg_database WHERE datname = $1", [databaseName]);
  if (rowCount === 0) {
    await admin.query(`CREATE DATABASE "${databaseName}"`);
  }
  await admin.end();

  const pool = new pg.Pool({ connectionString: url.toString() });
  await migrate(drizzle(pool), { migrationsFolder: "./drizzle" });
  await pool.end();
}
