import { jwtVerify, SignJWT } from "jose";
import { z } from "zod";
import { config } from "../config.ts";

const secretKey = new TextEncoder().encode(config.jwtSecret);

export const ACCESS_TOKEN_TTL_SECONDS = 60 * 60; // 1 hora

// O token não carrega a role (ela mudaria no banco e o token ficaria desatualizado): quem precisa dela consulta o banco.
// "ver" é a versão do token do usuário; a rota só aceita o token se for igual à versão atual no banco.
export function createAccessToken(user: { id: string; tokenVersion: number }): Promise<string> {
  return new SignJWT({ ver: user.tokenVersion })
    .setProtectedHeader({ alg: "HS256" })
    .setSubject(user.id) // "sub": quem é o dono do token
    .setIssuedAt() // "iat": quando foi emitido
    .setExpirationTime(`${ACCESS_TOKEN_TTL_SECONDS}s`) // "exp": quando deixa de valer
    .sign(secretKey);
}

// O "sub" precisa ser um uuid: senão a consulta no banco (coluna uuid) falharia com erro 500.
// O "ver" precisa ser um inteiro >= 0: um token sem ele nunca é aceito.
const claimsSchema = z.object({ sub: z.uuid(), ver: z.number().int().nonnegative() });

// Só confere assinatura, expiração e formato. Se a versão ainda é a atual quem decide é o banco.
export async function verifyAccessToken(
  token: string,
): Promise<{ userId: string; tokenVersion: number }> {
  // requiredClaims: sem isso, o jose aceita um token sem "exp", que valeria para sempre.
  const { payload } = await jwtVerify(token, secretKey, {
    algorithms: ["HS256"],
    requiredClaims: ["exp", "sub"],
  });

  const { sub, ver } = claimsSchema.parse(payload); // lança erro se o formato for inválido
  return { userId: sub, tokenVersion: ver };
}
