import { and, eq } from "drizzle-orm";
import type { FastifyReply, FastifyRequest } from "fastify";
import type { Db } from "../db/client.ts";
import { users } from "../db/schema.ts";
import { verifyAccessToken } from "./token.ts";

// Resposta padrão para qualquer falha de autenticação: sem token, token inválido ou revogado, ou usuário inexistente.
export function unauthorized(reply: FastifyReply) {
  return reply.code(401).header("WWW-Authenticate", "Bearer").send({ error: "Não autenticado" });
}

// Devolve o usuário dono do token, ou null se o token for inválido, estiver revogado ou a conta não existir.
export async function authenticate(request: FastifyRequest, db: Db) {
  // Formato "Bearer <token>". O nome do esquema não diferencia maiúsculas (RFC 7235).
  const [scheme, token] = request.headers.authorization?.split(" ") ?? [];
  if (scheme?.toLowerCase() !== "bearer" || !token) {
    return null;
  }

  // Token inválido (assinatura, expiração ou formato) é culpa de quem chamou: vira null → 401.
  const claims = await verifyAccessToken(token).catch(() => null);
  if (!claims) {
    return null;
  }

  // Fora do catch acima: se o banco falhar, o erro vai para o error handler (500 + log).
  // Exigir a versão igual na mesma consulta é o que revoga tokens antigos; só sai daqui o que a rota pode expor.
  const [user] = await db
    .select({
      id: users.id,
      username: users.username,
      email: users.email,
      role: users.role,
      createdAt: users.createdAt,
    })
    .from(users)
    .where(and(eq(users.id, claims.userId), eq(users.tokenVersion, claims.tokenVersion)))
    .limit(1);

  return user ?? null;
}
