import argon2 from "argon2";

// argon2id com salt aleatório por padrão. O salt e os parâmetros ficam dentro do próprio hash.
export function hashPassword(password: string): Promise<string> {
  return argon2.hash(password);
}
