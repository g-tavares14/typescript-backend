import { randomUUID } from "node:crypto";
import argon2 from "argon2";

// argon2id com salt aleatório por padrão. O salt e os parâmetros ficam dentro do próprio hash.
export function hashPassword(password: string): Promise<string> {
  return argon2.hash(password);
}

export function verifyPassword(hash: string, password: string): Promise<boolean> {
  return argon2.verify(hash, password);
}

// Hash de uma senha aleatória que ninguém conhece, gerado uma vez na inicialização.
const dummyHash = hashPassword(randomUUID());

// Usado no login quando o email não existe: gasta o mesmo tempo de uma verificação real,
// para que o tempo de resposta não revele quais emails estão cadastrados.
export async function simulatePasswordVerification(password: string): Promise<void> {
  await verifyPassword(await dummyHash, password);
}
