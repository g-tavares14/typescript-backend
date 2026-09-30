import { DrizzleQueryError } from "drizzle-orm";
import pg from "pg";

// 23505 é o código do Postgres para violação de UNIQUE.
// O Drizzle embrulha o erro do driver em DrizzleQueryError, e o erro original fica em .cause.
export function isUniqueViolation(error: unknown): boolean {
  return (
    error instanceof DrizzleQueryError &&
    error.cause instanceof pg.DatabaseError &&
    error.cause.code === "23505"
  );
}
