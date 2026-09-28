import { jwtVerify, SignJWT } from "jose";
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

export async function verifyAccessToken(token: string): Promise<{ userId: string; role: string }> {
  const { payload } = await jwtVerify(token, secretKey, { algorithms: ["HS256"] });

  if (typeof payload.sub !== "string" || typeof payload.role !== "string") {
    throw new Error("Token com formato inválido");
  }

  return { userId: payload.sub, role: payload.role };
}