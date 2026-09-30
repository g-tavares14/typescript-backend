# Tarefas: Editar e excluir a própria conta

Plano: [plan.md](plan.md). Spec: [SPEC-users-update-delete.md](../SPEC-users-update-delete.md).
Cada tarefa: teste falhando → código → `npm run typecheck` + `npm test` + `curl` → commit (com o pedido do dono).

## Task 1: Regras de username/email e `isUniqueViolation` num lugar compartilhado ✅
- Aceite:
  - `src/lib/user-fields.ts` exporta `usernameSchema` e `emailSchema`, com as mesmas regras, ordem e mensagens de hoje.
  - `src/lib/db-errors.ts` exporta `isUniqueViolation`; ele sai do `auth.ts`.
  - `registerSchema` e `loginSchema` usam os schemas compartilhados; nenhum comportamento muda.
  - `register.test.ts` e `login.test.ts` verdes **sem nenhuma alteração**.
- Verificar: `npm run typecheck`; `npm test`; `curl` cadastro com `"  Joao "` (vira `joao`) e cadastro duplicado (`409`).
- Arquivos: src/routes/auth.ts, src/lib/user-fields.ts (novo), src/lib/db-errors.ts (novo)
- Tamanho: S · Depende de: nada

## Task 2: `PATCH /users/me` (caminho feliz, 409 e 401) ✅
- Aceite:
  - Só `username`, só `email` e os dois → `200` no formato do `GET /users/me`, com os valores normalizados
    (`"  Joao_Silva "` → `joao_silva`, email em minúsculas); o `GET /users/me` mostra o novo; o mesmo token continua valendo.
  - Depois de trocar o email: login com o novo → `200`, com o antigo → `401` `Email ou senha inválidos`.
  - Enviar o próprio valor atual → `200`.
  - Username ou email de outra conta → `409` `Email ou username já cadastrado`, e nada muda (nem o outro campo enviado junto).
  - Sem token e token revogado por logout → `401` padrão (`expectUnauthorized`).
  - Conta apagada entre o `requireAuth` e o `UPDATE` → `401` padrão (teste com hook `preHandler`; ver Riscos no plano).
  - `unauthorized()` e `publicUserColumns` exportados de `authenticate.ts`; `requireAuth` sem mudança de comportamento.
  - Uma consulta só: `UPDATE ... WHERE id = <token> RETURNING publicUserColumns`.
- Verificar: `test/users-update-delete.test.ts` (novo); `npm run typecheck`; `npm test`;
  `curl` login → `PATCH` só `username` → `GET /users/me` → `PATCH` com o email de outro usuário (409).
- Arquivos: src/routes/users.ts, src/lib/authenticate.ts, test/users-update-delete.test.ts
- Tamanho: M · Depende de: T1

## Task 3: `PATCH /users/me` (validação) ✅
- Aceite:
  - `{}` e corpo só com campos desconhecidos (ex.: `{ "foo": 1 }`, `{ "password": "x" }`) → `400` `Envie ao menos um campo para alterar`.
  - `null` e tipo errado (`"email": null`, `"username": 123`) → `400` `Campo obrigatório ausente ou inválido`.
  - Valores fora da regra do cadastro: username curto, longo, com acento, com espaço no meio → mensagens do username;
    email inválido → `Email inválido`.
  - `null`, `[]` e texto na raiz → `INVALID_BODY`.
  - `role: "admin"` junto com `username` → `200`, e a `role` continua `user` no banco; `password` no corpo é ignorada
    (a senha antiga continua entrando).
  - `PATCH` recusado não altera nada (conferido pelo `GET /users/me`).
- Verificar: `test/users-update-delete.test.ts`; `npm run typecheck`; `npm test`;
  `curl` `PATCH` com `{}` e com `{"username":"ab"}`.
- Arquivos: src/routes/users.ts, test/users-update-delete.test.ts
- Tamanho: S · Depende de: T2

## Checkpoint A (T1–T3)
- [x] typecheck + testes verdes; `register.test.ts` e `login.test.ts` sem alteração
- [x] Revisão do dono antes do `DELETE`

## Task 4: `DELETE /users/me`
- Aceite:
  - `{ "password": <senha certa> }` → `204` com corpo vazio; depois disso o mesmo token dá `401` no `GET /users/me`, o login
    dá `401`, e o email e o username podem ser cadastrados de novo.
  - Os registros financeiros do usuário somem do banco (`CASCADE`); os de **outro usuário** continuam no `GET` dele.
  - Senha errada → `403` `Senha incorreta`, e a conta continua (o login ainda funciona).
  - Sem `password`, `password: ""` e `password: 123` → `400` `Campo obrigatório ausente ou inválido`.
  - Sem corpo, `null` e `[]` → `INVALID_BODY`; corpo com `Content-Type: text/plain` → `415` (mensagem do `sendError`).
  - Sem token e token revogado por logout → `401` padrão.
  - Conta apagada entre o `SELECT` do hash e o `DELETE` → `401` padrão.
  - O `passwordHash` não aparece em nenhuma resposta nem no `currentUser()`.
- Verificar: `test/users-update-delete.test.ts`; `npm run typecheck`; `npm test`;
  `curl` login → `POST /transactions` → `DELETE` com senha errada (403) → `DELETE` certo (204) → `GET /users/me` (401) → login (401).
- Arquivos: src/routes/users.ts, test/users-update-delete.test.ts
- Tamanho: S · Depende de: T3

## Task 5: Rate limit no `PATCH` e no `DELETE /users/me`
- Aceite:
  - `DELETE` 5/min e `PATCH` 10/min por IP (`config.rateLimit`), com constantes nomeadas como as do `auth.ts`.
  - Com token válido: o 6º `DELETE` (senha errada) e o 11º `PATCH` no mesmo minuto → `429`
    `Muitas tentativas. Tente novamente mais tarde.` com `Retry-After`.
  - Requisição sem token recebe `401` (o `requireAuth` roda antes do limite).
  - Os testes de `users-update-delete.test.ts` continuam com o rate limit desligado (`createTestApp()` padrão).
- Verificar: `test/rate-limit.test.ts`; `npm run typecheck`; `npm test`; `curl` 6 `DELETE` com senha errada (o 6º dá 429).
- Arquivos: src/routes/users.ts, test/rate-limit.test.ts
- Tamanho: S · Depende de: T4

## Checkpoint B (T4–T5)
- [ ] typecheck + testes verdes
- [ ] Fluxo do `curl` da spec (Success Criteria) completo no servidor real
- [ ] Revisão do dono

## Task 6: Documentação
- Aceite:
  - `AGENTS.md`: etapa atual (edição e exclusão da conta), estrutura (`user-fields.ts`, `db-errors.ts`, `users.ts`, testes novos),
    roteiro da etapa e decisões (`/users/me` sem admin, `PATCH` só `username`/`email`, email sem senha e o risco futuro
    da recuperação de senha, `DELETE` com senha e `403`, rate limits).
  - Spec com status **concluída** e os Success Criteria marcados.
- Verificar: leitura dos dois arquivos; `npm test` verde.
- Arquivos: AGENTS.md, SPEC-users-update-delete.md
- Tamanho: S · Depende de: T5

## Checkpoint final
- [ ] Todos os critérios de aceite da spec com teste passando
- [ ] `npm run typecheck` e `npm test` verdes
- [ ] Documentação atualizada; pronto para o commit (com o pedido do dono)
