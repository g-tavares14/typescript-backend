import { SignJWT } from "jose";
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
