# Tarefas: Endurecimento da autenticação

Plano: [plan.md](plan.md). Cada tarefa: teste falhando → código → `npm run typecheck` + `npm test` + `curl` → commit.

## Task 0: Commitar a revisão e corrigir a mensagem duplicada
- Aceite: commit `d6f59f6` com mensagem própria ("Formata os testes do cadastro"); testes da revisão
  (register/login/me) e as 4 correções (exp obrigatório, sub uuid, tokenType, bearer minúsculo) commitados.
- Verificar: `npm test` verde (34); `git log --oneline` sem mensagens repetidas.
- Arquivos: src/lib/token.ts, src/routes/auth.ts, test/*.ts
- Tamanho: S · Depende de: nada

## Task 1: Postgres só em 127.0.0.1
- Aceite: `docker compose port db 5432` → `127.0.0.1:5432`.
- Verificar: `docker compose up -d`, `npm test` verde.
- Arquivos: docker-compose.yml
- Tamanho: XS · Depende de: T0

## Task 2: Regra do username
- Aceite: `trim` + `toLowerCase`; formato `^[a-z0-9_]{3,50}$` (400 com mensagem da spec);
  `Joao` depois de `joao` → 409; `CHECK users_username_format_check` no banco (migration 0001).
- Verificar: testes novos em register.test.ts; SQL da 0001 revisado; `joão` renomeado no dev; `db:migrate` ok; curl.
- Arquivos: src/routes/auth.ts, src/db/schema.ts, drizzle/0001_*.sql (+ meta), test/register.test.ts
- Tamanho: M · Depende de: T0

## Task 3: JWT sem role, com versão
- Aceite: payload `{ sub, ver, iat, exp }` sem `role`; coluna `token_version` (migration 0002);
  token com `ver` diferente do banco → 401 no `/me`; `authenticate()` extraída.
- Verificar: testes em login.test.ts e me.test.ts; SQL da 0002 revisado; `db:migrate` ok; curl.
- Arquivos: src/db/schema.ts, drizzle/0002_*.sql (+ meta), src/lib/token.ts, src/routes/auth.ts, test/login.test.ts, test/me.test.ts
- Tamanho: M · Depende de: T2

## Checkpoint A (T0–T3)
- [ ] typecheck + testes verdes, migrations aplicadas no dev, fluxo register → login → me com curl

## Task 4: POST /auth/logout
- Aceite: 204 sem corpo; token antigo → 401 em `/me` e `/logout`; novo login funciona; outro usuário não é afetado; sem token → 401.
- Verificar: test/logout.test.ts; curl.
- Arquivos: src/routes/auth.ts, test/logout.test.ts
- Tamanho: S · Depende de: T3

## Task 5: Rate limit em login e cadastro
- Aceite: 6º login/min por IP → 429 `{ error }` + `Retry-After`; 4º cadastro/min → 429;
  429 acontece antes de validar o corpo; `/me` e `/logout` sem limite; testes antigos usam `rateLimit: false`.
- Verificar: test/rate-limit.test.ts com os limites reais; curl (6 logins seguidos).
- Arquivos: package.json, package-lock.json, src/app.ts, src/routes/auth.ts, test/helpers.ts, test/rate-limit.test.ts
- Tamanho: M · Depende de: T4

## Checkpoint B (T4–T5)
- [ ] typecheck + testes verdes; curl: logout revoga, 6º login → 429

## Task 6: Documentação
- Aceite: AGENTS.md com estrutura, roteiro (passo 9 ✅ + endurecimento), decisões (409 mantido, role fora do token,
  token_version, rate limit/trustProxy, regra do username) e a regra de segurança sobre `token_version`.
- Verificar: leitura; links e comandos corretos.
- Arquivos: AGENTS.md, SPEC-auth-hardening.md, tasks/todo.md
- Tamanho: S · Depende de: T5

## Checkpoint final
- [ ] Todos os critérios de sucesso da spec marcados
- [ ] Resumo das mudanças de contrato para o frontend
