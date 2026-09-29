# Tarefas: Registros financeiros (entradas e saídas)

Plano: [plan.md](plan.md). Spec: [SPEC-transactions.md](../SPEC-transactions.md).
Cada tarefa: teste falhando → código → `npm run typecheck` + `npm test` + `curl` → commit (com o pedido do dono).

## Task 1: Tabela `transactions` e migration 0003 ✅
- Aceite:
  - `transactions` no `schema.ts` como na spec: FK `user_id` → `users.id` `ON DELETE CASCADE`,
    `CHECK` de `type` e de `amount_cents > 0`, índice `(user_id, occurred_on)`; `type Transaction` exportado.
  - `drizzle/0003_transactions.sql` gerada, **SQL revisado pelo dono** e aplicada no dev.
  - `resetDatabase` limpa `users` e `transactions`; os testes antigos continuam verdes.
- Verificar: `npm run db:generate -- --name transactions`; leitura do SQL; `npm run db:migrate`;
  `npm test`; `\d transactions` no `psql` mostra FK, `CHECK`s e índice.
- Arquivos: src/db/schema.ts, drizzle/0003_transactions.sql (+ meta), test/helpers.ts
- Tamanho: S · Depende de: nada

## Task 2: `POST /transactions` (caminho feliz e autenticação) ✅
- Aceite:
  - Corpo válido → `201` com `{ id, type, amount, description, date, createdAt }` (sem `userId`).
  - O registro é salvo com o `user_id` do token; um `userId` de outro usuário no corpo é ignorado.
  - Sem token, token inválido ou revogado por logout → `401` padrão (`expectUnauthorized`).
  - `badRequest()` e a mensagem `required` saem do `auth.ts` para `src/lib/`; os testes de auth continuam verdes.
- Verificar: `test/transactions.test.ts` (novo); `npm run typecheck`; `npm test`;
  `curl` com login → `POST` → conferir a linha no banco.
- Arquivos: src/routes/transactions.ts, src/app.ts, src/lib/validation.ts, src/routes/auth.ts, test/transactions.test.ts
- Tamanho: M · Depende de: T1

## Task 3: `POST /transactions` (validação) ✅
- Aceite: todos os `400` da tabela da spec com as mensagens exatas: `type` fora de `income`/`expense`;
  `amount` `0`, negativo, `19.9` e acima de `100000000000`; `description` vazia/só espaços e com
  201 caracteres (e `trim` aplicado ao salvar); `date` `2026-02-30` e `29/09/2026`; campo ausente ou com tipo JSON errado (ex.: `amount: "1990"`) → mensagem `required`.
- Verificar: testes novos em `test/transactions.test.ts`; `npm test`; `curl` com um valor em reais (`19.9`) → 400.
- Arquivos: src/routes/transactions.ts, test/transactions.test.ts
- Tamanho: S · Depende de: T2

## Checkpoint A (T1–T3)
- [x] typecheck + testes verdes; migration aplicada no dev
- [x] `curl`: login → `POST` válido (201) → `POST` inválido (400) → `POST` sem token (401)
- [x] Revisão do dono antes de seguir para a leitura

## Task 4: `GET /transactions` (lista + totais) ✅
- Aceite:
  - `200` com `{ summary: { income, expense, balance }, transactions: [...] }`; totais são `number`.
  - Sem registros → lista vazia e totais `0`.
  - Ordem: `date` desc, empate por `createdAt` desc. `balance` negativo quando as saídas são maiores.
  - **Isolamento**: o usuário B não vê nem soma registros do A.
  - Sem token → `401` padrão.
- Verificar: testes novos; `npm test`; `curl` com dois usuários.
- Arquivos: src/routes/transactions.ts, test/transactions.test.ts
- Tamanho: S · Depende de: T3

## Task 5: `GET /transactions` (filtro `from`/`to`) ✅
- Aceite: `from` e `to` inclusivos (registros exatamente nos limites entram); só `from` ou só `to` funciona;
  os totais usam o mesmo filtro da lista; data inválida → `400` `Data inválida (use AAAA-MM-DD)`;
  `from` depois de `to` → `400` `A data inicial deve ser anterior ou igual à final`; o isolamento continua valendo com filtro.
- Verificar: testes novos; `npm test`; `curl` com `?from=2026-09-01&to=2026-09-30`.
- Arquivos: src/routes/transactions.ts, test/transactions.test.ts
- Tamanho: S · Depende de: T4

## Checkpoint B (T4–T5)
- [ ] typecheck + testes verdes
- [ ] `curl`: fluxo completo (entrada + saída → `GET` com o saldo certo; filtro de mês; segundo usuário vê lista vazia)
- [ ] Revisão do dono antes da refatoração

## Task 6: Refatoração: `authenticate()` → hook `preHandler`
- Aceite:
  - `requireAuth(db)` e `currentUser(request)` em `src/lib/authenticate.ts`; `decorateRequest("user", null)`
    no `buildApp`; declaration merging de `FastifyRequest.user`.
  - Hook no plugin inteiro em `users.ts` e `transactions.ts`; `{ preHandler: requireAuth(db) }` só em `/auth/logout`.
  - Nenhum handler chama `authenticate()` ou `unauthorized()` diretamente.
  - `/health`, `/auth/register` e `/auth/login` continuam públicas.
  - **Nenhum teste existente alterado**, e todos verdes.
- Verificar: `npm run typecheck`; `npm test`; `git diff --stat test/` vazio; `curl` sem token nas 4 rotas
  protegidas (401) e nas 3 públicas (sem 401).
- Arquivos: src/lib/authenticate.ts, src/app.ts, src/routes/users.ts, src/routes/auth.ts, src/routes/transactions.ts
- Tamanho: M · Depende de: T5

## Task 7: Documentação
- Aceite:
  - AGENTS.md: etapa atual (financeiro), estrutura (`transactions.ts`, `validation.ts`, teste novo),
    roteiro da etapa, decisões (centavos inteiros, `/transactions`, totais no banco, `requireAuth` como hook) e a
    regra de segurança "toda rota protegida usa `requireAuth`; toda consulta a `transactions` filtra por `user_id`".
  - Critérios de sucesso da spec marcados; resumo do contrato para o frontend.
- Verificar: leitura; comandos e caminhos conferidos.
- Arquivos: AGENTS.md, SPEC-transactions.md, tasks/todo.md
- Tamanho: S · Depende de: T6

## Checkpoint final
- [ ] Todos os critérios de sucesso da spec marcados
- [ ] Resumo das mudanças de contrato para o frontend
