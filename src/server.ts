import { sql } from "drizzle-orm";
import { buildApp } from "./app.ts";
import { config } from "./config.ts";
import { createDb } from "./db/client.ts";

const db = createDb(config.databaseUrl);

// Fail fast: se o banco não responder na inicialização, o servidor não sobe.
try {
  await db.execute(sql`SELECT 1`);
} catch (error) {
  console.error("Falha ao conectar no banco de dados:", error);
  process.exit(1);
}

const app = buildApp(db);
await app.listen({ host: "0.0.0.0", port: config.port });
