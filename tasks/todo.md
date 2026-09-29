# Tarefas: Editar e excluir registros financeiros

Plano: [plan.md](plan.md). Spec: [SPEC-transactions-update-delete.md](../SPEC-transactions-update-delete.md).
Cada tarefa: teste falhando → código → `npm run typecheck` + `npm test` + `curl` → commit (com o pedido do dono).

## Task 1: Coluna `updated_at`, migration 0004 e `updatedAt` nas respostas ✅
- Aceite:
  - `updatedAt: timestamp("updated_at", { withTimezone: true }).notNull().defaultNow()` no `schema.ts`.
  - `drizzle/0004_transactions_updated_at.sql` gerada, com `UPDATE "transactions" SET "updated_at" = "created_at";`
    acrescentado depois do `ADD COLUMN`; **SQL revisado pelo dono** e aplicado no dev; linhas antigas com
    `updated_at = created_at`.
  - `publicColumns` ganha `updatedAt`: `POST` (`201`) e cada item do `GET` trazem `updatedAt`, igual ao `createdAt`.
  - Nos testes existentes, só `transactions.test.ts:46` e `:315` mudam (ganham `updatedAt: expect.any(String)`).
- Verificar: `npm run db:generate -- --name transactions_updated_at`; leitura do SQL; `npm run db:migrate`;
  `\d transactions` e `select count(*) from transactions where updated_at <> created_at` (= 0) no `psql`;
  `npm run typecheck`; `npm test`; `curl` `POST` → `updatedAt` = `createdAt`.
- Arquivos: src/db/schema.ts, drizzle/0004_transactions_updated_at.sql (+ meta), src/routes/transactions.ts,
  test/transactions.test.ts
- Tamanho: M · Depende de: nada

## Checkpoint A (T1)
- [x] SQL da `0004` revisado e aplicado no dev; typecheck + testes verdes
- [x] Revisão do dono antes das rotas novas

## Task 2: `DELETE /transactions/:id` ✅
- Aceite:
  - Registro do próprio usuário → `204` com corpo vazio; ele some do `GET` e dos totais.
  - `404` `Registro não encontrado` para: id inexistente, id de **outro usuário** (o registro do outro continua no `GET`
    dele), `:id` não UUID (`/transactions/abc`, sem ir ao banco) e `DELETE` repetido.
  - Sem token ou token revogado por logout → `401` padrão (`expectUnauthorized`).
  - `parseId()` e `notFound()` criados no `transactions.ts`; condição `id` + `user_id` no próprio `DELETE`.
  - `test/errors.test.ts` (`DELETE /transactions` sem id → `404` `Rota não encontrada`) continua verde sem mudança.
- Verificar: `test/transactions-update-delete.test.ts` (novo); `npm run typecheck`; `npm test`;
  `curl` login → `POST` → `DELETE` (204) → `GET` sem o registro → `DELETE` de novo (404).
- Arquivos: src/routes/transactions.ts, test/transactions-update-delete.test.ts
- Tamanho: S · Depende de: T1

## Task 3: `PATCH /transactions/:id` (caminho feliz, isolamento e 404) ✅
- Aceite:
  - Só `description` → `200` com o registro inteiro: descrição nova, `type`, `amount` e `date` iguais. O mesmo para
    cada um dos outros campos sozinho e para os quatro juntos; o `GET` e o `summary` refletem a mudança
    (ex.: `amount` ou `type` alterado muda os totais).
  - `updatedAt` passa a ser posterior ao valor anterior (registro recuado no banco antes do `PATCH`, ver plano),
    e o `createdAt` não muda. Um `PATCH` com os mesmos valores também atualiza o `updatedAt`.
  - `404` para id inexistente, de outro usuário (registro do outro intacto) e `:id` não UUID.
  - Sem token ou token revogado → `401` padrão.
  - Uma consulta: `UPDATE ... SET <campos enviados>, updated_at = now() WHERE id AND user_id RETURNING`.
- Verificar: testes novos; `npm run typecheck`; `npm test`;
  `curl` login → `POST` → `PATCH` só `description` → `PATCH` só `amount` → `GET` com o saldo certo.
- Arquivos: src/routes/transactions.ts, test/transactions-update-delete.test.ts
- Tamanho: S · Depende de: T2

## Task 4: `PATCH /transactions/:id` (validação) ✅
- Aceite:
  - `{}` e corpo só com campos desconhecidos → `400` `Envie ao menos um campo para alterar`.
  - `null` e tipo JSON errado (`"amount": null`, `"amount": "1990"`, `"type": 123`) → `Campo obrigatório ausente ou inválido`.
  - Valor fora da regra → a mensagem do campo do `POST`: `type: "foo"`, `amount: 19.9`/`0`/acima do teto,
    `description: "   "`/201 caracteres, `date: "2026-02-30"`/`"29/09/2026"`; `trim` aplicado ao salvar.
  - `id`, `userId`, `createdAt` e `updatedAt` no corpo são ignorados (junto de um campo válido).
  - `PATCH` recusado (`400` ou `404`) não altera nada, nem o `updatedAt` (conferido pelo `GET`).
  - Ordem: `:id` não UUID com corpo inválido → `404`; id válido de outro usuário com corpo inválido → `400`.
- Verificar: testes novos; `npm test`; `curl` `PATCH` com `{}` e com `amount: 19.9` → `400`.
- Arquivos: src/routes/transactions.ts, test/transactions-update-delete.test.ts
- Tamanho: S · Depende de: T3

## Checkpoint B (T2–T4)
- [x] typecheck + testes verdes (os antigos também)
- [x] `curl`: fluxo completo da spec (POST → PATCH parcial → GET → DELETE → GET) e segundo usuário com `404`
- [x] Revisão do dono antes da documentação

## Task 5: Documentação
- Aceite:
  - `SPEC-transactions.md`: resumo do contrato para o front com `updatedAt` nas respostas, `PATCH` e `DELETE`,
    o `404` `Registro não encontrado`, o `400` `Envie ao menos um campo para alterar` e o aviso do `DELETE` sem `Content-Type`.
  - `AGENTS.md`: estrutura (teste novo, `transactions.ts` com as 4 rotas), roteiro da etapa e decisões
    (`PATCH` parcial, `404` para registro de outro usuário, `updated_at` sem versão, `UPDATE` da `0004`).
  - Critérios de sucesso da `SPEC-transactions-update-delete.md` marcados.
- Verificar: leitura; comandos e caminhos conferidos.
- Arquivos: SPEC-transactions.md, SPEC-transactions-update-delete.md, AGENTS.md, tasks/todo.md
- Tamanho: S · Depende de: T4

## Checkpoint final
- [ ] Todos os critérios de sucesso da spec marcados
- [ ] Resumo das mudanças de contrato para o frontend
