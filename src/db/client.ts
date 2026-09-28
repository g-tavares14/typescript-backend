import { drizzle } from "drizzle-orm/node-postgres";
import pg from "pg";
import * as schema from "./schema.ts";

export function createDb(databaseUrl: string) {
  // O Pool reaproveita conexões, como o PgPool do sqlx.
  // connectionTimeoutMillis: desiste em 3 s em vez de esperar indefinidamente.
  const pool = new pg.Pool({ connectionString: databaseUrl, connectionTimeoutMillis: 3000 });

  // Se o banco cair, o pg emite "error" nas conexões ociosas.
  // Sem este handler, o Node encerra o processo inteiro.
  pool.on("error", (error) => {
    console.error("Conexão ociosa com o banco foi perdida:", error.message);
  });

  return drizzle(pool, { schema });
}

export type Db = ReturnType<typeof createDb>;
