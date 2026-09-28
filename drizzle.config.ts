import { defineConfig } from "drizzle-kit";

// O drizzle-kit não lê o .env sozinho.
try {
  process.loadEnvFile();
} catch {
  // Sem .env: usa as variáveis já definidas no ambiente.
}

const databaseUrl = process.env.DATABASE_URL;
if (!databaseUrl) {
  throw new Error("Variável de ambiente DATABASE_URL não definida");
}

export default defineConfig({
  dialect: "postgresql",
  schema: "./src/db/schema.ts",
  out: "./drizzle",
  dbCredentials: { url: databaseUrl },
});
