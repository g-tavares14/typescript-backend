import { jwtVerify, SignJWT } from "jose";
import { z } from "zod";
import { config } from "../config.ts";

const secretKey = new TextEncoder().encode(config.jwtSecret);

export const ACCESS_TOKEN_TTL_SECONDS = 60 * 60; // 1 hora

export function createAccessToken(user: { id: string; role: string }): Promise<string> {
  return new SignJWT({ role: user.role })
    .setProtectedHeader({ alg: "HS256" })
    .setSubject(user.id) // "sub": quem é o dono do token
    .setIssuedAt() // "iat": quando foi emitido
    .setExpirationTime(`${ACCESS_TOKEN_TTL_SECONDS}s`) // "exp": quando deixa de valer
    .sign(secretKey);
}

// O "sub" precisa ser um uuid: senão a consulta no banco (coluna uuid) falharia com erro 500.
const claimsSchema = z.object({ sub: z.uuid(), role: z.string() });

export async function verifyAccessToken(token: string): Promise<{ userId: string; role: string }> {
  // requiredClaims: sem isso, o jose aceita um token sem "exp", que valeria para sempre.
  const { payload } = await jwtVerify(token, secretKey, {
    algorithms: ["HS256"],
    requiredClaims: ["exp", "sub"],
  });

  const { sub, role } = claimsSchema.parse(payload); // lança erro se o formato for inválido
  return { userId: sub, role };
}
