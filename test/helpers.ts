import { sql } from "drizzle-orm";
import type { LightMyRequestResponse } from "fastify";
import { expect } from "vitest";
import { buildApp } from "../src/app.ts";
import { createDb, type Db } from "../src/db/client.ts";

type TestApp = ReturnType<typeof buildApp>;

// Cria o app de verdade (mesmas rotas, mesmo error handler), ligado ao banco de testes.
// Os testes chamam as rotas com app.inject(), sem abrir porta de rede.
// O rate limit fica DESLIGADO por padrão: todo inject vem do mesmo IP, e os testes fazem mais
// de 5 logins por minuto. Só o test/rate-limit.test.ts liga (rateLimit: true), com os limites reais.
export function createTestApp({ rateLimit = false }: { rateLimit?: boolean } = {}) {
  const db = createDb(process.env.DATABASE_URL ?? "");
  const app = buildApp(db, { logger: false, rateLimit });
  return { app, db };
}

// Libera o servidor e as conexões com o banco no fim dos testes.
export async function closeTestApp(app: TestApp, db: Db) {
  await app.close();
  await db.$client.end();
}

// Apaga todos os usuários, para cada teste começar do zero.
export async function resetDatabase(db: Db) {
  await db.execute(sql`TRUNCATE TABLE users`);
}

export const defaultUser = { username: "joao", email: "joao@email.com", password: "senha123" };

// Atalhos para o "Arrange" dos testes: cadastram e fazem login pelas rotas de verdade.
export async function registerUser(app: TestApp, user: Record<string, unknown> = defaultUser) {
  const response = await app.inject({ method: "POST", url: "/auth/register", payload: user });
  if (response.statusCode !== 201) {
    throw new Error(`Cadastro falhou no arrange do teste: ${response.statusCode} ${response.body}`);
  }
  return response.json<{ id: string; username: string }>();
}

export async function loginUser(app: TestApp, credentials = defaultUser) {
  const response = await app.inject({
    method: "POST",
    url: "/auth/login",
    payload: { email: credentials.email, password: credentials.password },
  });
  if (response.statusCode !== 200) {
    throw new Error(`Login falhou no arrange do teste: ${response.statusCode} ${response.body}`);
  }
  return response.json<{ token: string }>().token;
}

// Valor do header Authorization para um token.
export function bearer(token: string) {
  return `Bearer ${token}`;
}

// Toda falha de autenticação tem a mesma resposta: 401, WWW-Authenticate e a mesma mensagem.
export function expectUnauthorized(response: LightMyRequestResponse) {
  expect(response.statusCode).toBe(401);
  expect(response.headers["www-authenticate"]).toBe("Bearer");
  expect(response.json()).toEqual({ error: "Não autenticado" });
}
