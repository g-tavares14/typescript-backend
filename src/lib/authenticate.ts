import { and, eq } from "drizzle-orm";
import type { FastifyReply, FastifyRequest, onRequestAsyncHookHandler } from "fastify";
import type { Db } from "../db/client.ts";
import { users } from "../db/schema.ts";
import { verifyAccessToken } from "./token.ts";

// Resposta padrão para qualquer falha de autenticação: sem token, token inválido ou revogado, ou usuário inexistente.
export function unauthorized(reply: FastifyReply) {
  return reply.code(401).header("WWW-Authenticate", "Bearer").send({ error: "Não autenticado" });
}

// Colunas do usuário que as rotas podem expor (sem password_hash nem token_version).
// O `authenticate` e as rotas que devolvem o usuário (`.returning(...)`) usam a mesma lista, então o formato é um só.
export const publicUserColumns = {
  id: users.id,
  username: users.username,
  email: users.email,
  role: users.role,
  createdAt: users.createdAt,
};

// Devolve o usuário dono do token e a versão do token, ou null se o token for inválido, estiver revogado ou a conta
// não existir. A versão fica fora do usuário: ela não pode sair numa resposta, só serve para gravações que precisam
// conferir que o token continua valendo (ex.: a troca de senha).
async function authenticate(request: FastifyRequest, db: Db) {
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
    .select(publicUserColumns)
    .from(users)
    .where(and(eq(users.id, claims.userId), eq(users.tokenVersion, claims.tokenVersion)))
    .limit(1);

  return user ? { user, tokenVersion: claims.tokenVersion } : null;
}

type AuthenticatedUser = NonNullable<Awaited<ReturnType<typeof authenticate>>>["user"];

// Diz ao TypeScript que a requisição tem `user` e `tokenVersion`. O app.ts registra os campos com
// decorateRequest(..., null), e o requireAuth preenche. `null` = requisição sem autenticação (rotas públicas ou antes
// do hook).
declare module "fastify" {
  interface FastifyRequest {
    user: AuthenticatedUser | null;
    tokenVersion: number | null;
  }
}

// Hook onRequest: autentica antes de qualquer outra etapa. Se falhar, responde 401 e a rota nem chega a rodar.
// Por rodar antes do parse do corpo, quem não está autenticado recebe 401 sem que o corpo seja lido.
// Uso: app.addHook("onRequest", requireAuth(db)) protege o plugin inteiro (só as rotas dele, o hook não vaza
// para plugins irmãos); { onRequest: requireAuth(db) } na opção de uma rota protege só ela.
export function requireAuth(db: Db): onRequestAsyncHookHandler {
  return async (request, reply) => {
    const auth = await authenticate(request, db);
    if (!auth) {
      unauthorized(reply);
      // O send já respondeu; retornar o reply avisa o Fastify para não rodar o handler.
      return reply;
    }
    request.user = auth.user;
    request.tokenVersion = auth.tokenVersion;
  };
}

// Usuário autenticado da requisição, para usar dentro dos handlers das rotas protegidas.
// Falha alto de propósito: se alguém esquecer o requireAuth numa rota nova, devolver null em silêncio faria a
// rota tratar "ninguém" como um usuário qualquer (ou quebrar longe da causa). Com o throw, o erro vai para o
// error handler (500 genérico + log com a causa) e o esquecimento aparece já no primeiro teste da rota.
export function currentUser(request: FastifyRequest): AuthenticatedUser {
  if (!request.user) {
    throw new Error("currentUser() chamado numa rota sem o hook requireAuth");
  }
  return request.user;
}

// Versão do token da requisição (já conferida com o banco pelo requireAuth). Falha alto pelo mesmo motivo do
// currentUser(). Usada em gravações que não podem acontecer com um token revogado no meio da requisição.
export function currentTokenVersion(request: FastifyRequest): number {
  if (request.tokenVersion === null) {
    throw new Error("currentTokenVersion() chamado numa rota sem o hook requireAuth");
  }
  return request.tokenVersion;
}
