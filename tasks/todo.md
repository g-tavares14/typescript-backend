# Tarefas: Endpoints restantes (troca de senha)

Plano: [plan.md](plan.md). Spec: [SPEC-endpoints-restantes.md](../SPEC-endpoints-restantes.md).
Cada tarefa: teste falhando → código → `npm run typecheck` + `npm test` + `curl` → commit (com o pedido do dono).

## Task 1: `passwordSchema` compartilhado ✅
- Aceite:
  - `src/lib/user-fields.ts` exporta `passwordSchema` (mínimo 8, mesmas mensagens de hoje).
  - O `registerSchema` usa o `passwordSchema`; nenhum comportamento muda.
  - `register.test.ts` verde **sem nenhuma alteração**.
- Verificar: `npm run typecheck`; `npm test`; `curl` cadastro com senha de 7 caracteres (`400` com a mensagem de hoje).
- Arquivos: src/lib/user-fields.ts, src/routes/auth.ts
- Tamanho: XS · Depende de: nada

## Task 2: `PUT /users/me/password` (caminho feliz, 403, 401 e corrida) ✅
- Aceite:
  - Senha atual certa → `200` `{ token, tokenType: "Bearer", expiresIn: 3600 }`; o token novo funciona no
    `GET /users/me`; o token antigo dá `401` padrão; login com a senha nova `200`, com a antiga `401`
    `Email ou senha inválidos`.
  - Tokens de outro "dispositivo" (outro login antes da troca) também dão `401`.
  - Senha atual errada → `403` `Senha incorreta`; a senha e os tokens continuam valendo.
  - Sem token e token revogado → `401` padrão (`expectUnauthorized`).
  - Corrida (`token_version` incrementado entre o `requireAuth` e o `UPDATE`) → `401` padrão; nada muda.
  - Uma consulta de gravação: `UPDATE ... SET password_hash, token_version + 1 WHERE id AND token_version RETURNING`.
  - O `requireAuth` passa a guardar a versão do token no request, sem mudar o formato do `GET /users/me`.
- Verificar: `test/users-password.test.ts` (novo); `npm run typecheck`; `npm test`;
  `curl` login → `PUT` → `GET /users/me` com o token antigo (`401`) e com o novo (`200`).
- Arquivos: src/routes/users.ts, src/lib/authenticate.ts, test/users-password.test.ts
- Tamanho: M · Depende de: T1

## Checkpoint A (T1–T2)
- [ ] `npm run typecheck` e `npm test` verdes
- [ ] Revisão do dono antes de seguir

## Task 3: Validação e rate limit ✅
- Aceite:
  - `currentPassword` ausente, vazia, `null` ou tipo errado → `400` `Campo obrigatório ausente ou inválido`.
  - `newPassword` ausente/`null`/tipo errado → `Campo obrigatório ausente ou inválido`; com menos de 8 → `A senha
    deve ter no mínimo 8 caracteres`.
  - Os dois inválidos → mensagem do `currentPassword` (ordem da spec); erro de validação não verifica a senha.
  - `null`, `[]` e texto na raiz → `INVALID_BODY`; campos a mais ignorados.
  - Sem token com corpo inválido → `401` (antes do `400`).
  - 5 tentativas por minuto por IP; a 6ª → `429` com `Retry-After`; sem token não consome o limite.
- Verificar: `test/users-password.test.ts`; `test/rate-limit.test.ts`; `npm run typecheck`; `npm test`;
  `curl` com `newPassword` curta (`400`) e 6 tentativas seguidas (`429`).
- Arquivos: src/routes/users.ts, test/users-password.test.ts, test/rate-limit.test.ts
- Tamanho: S · Depende de: T2

## Task 4: Documentação
- Aceite:
  - AGENTS.md: etapa atual, estrutura (`users.ts`, teste novo), decisões (troca de senha com token novo e
    `token_version`; rate limit 5/min), roteiro da etapa; a regra "A troca de senha (futura) deve incrementar
    `token_version`" deixa de ser futura.
  - Spec: status concluída e critérios marcados.
- Verificar: leitura do dono.
- Arquivos: AGENTS.md, SPEC-endpoints-restantes.md
- Tamanho: XS · Depende de: T3

## Checkpoint final
- [ ] `npm run typecheck` e `npm test` verdes
- [ ] Revisão do dono; depois, `/plan` da migração para Rust
