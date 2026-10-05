import { drizzle } from "drizzle-orm/node-postgres";
import { migrate } from "drizzle-orm/node-postgres/migrator";
import { spawn } from "node:child_process";
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

  // Modo paridade: sobe o servidor Rust (já compilado pelo npm run test:parity) com o ambiente dos testes
  // (banco _test e JWT_SECRET do .env.test, que o vitest.config.ts já carregou) e espera o /health responder.
  // A função devolvida é o teardown do vitest: derruba o servidor no fim.
  const apiUrl = process.env.API_URL;
  if (apiUrl) {
    const server = spawn("rust/target/debug/meu-backend", {
      env: { ...process.env, RUST_PORT: new URL(apiUrl).port, RUST_LOG: "warn" },
      stdio: ["ignore", "ignore", "inherit"],
    });
    await waitForHealth(apiUrl);
    return () => {
      server.kill();
    };
  }
}

async function waitForHealth(apiUrl: string) {
  for (let attempt = 0; attempt < 50; attempt++) {
    try {
      if ((await fetch(new URL("/health", apiUrl))).ok) {
        return;
      }
    } catch {
      // ainda subindo
    }
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  throw new Error(`O servidor de paridade não respondeu em ${apiUrl}/health`);
}
