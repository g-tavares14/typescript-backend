# Implementation Plan: Editar e excluir registros financeiros

Spec: [SPEC-transactions-update-delete.md](../SPEC-transactions-update-delete.md) (aprovada). Tarefas em [todo.md](todo.md).
O plano anterior (registros financeiros: `POST`/`GET`) foi concluído e está no histórico do git (commit `21e1289`).

## Overview

Acrescentar a coluna `updated_at` (migration `0004`) e expor `updatedAt` em todas as respostas com registro;
depois criar `DELETE /transactions/:id` e `PATCH /transactions/:id` (atualização parcial), sempre filtrando por
`user_id` do token na própria consulta. TDD em cada tarefa: teste falhando → código → `npm run typecheck` +
`npm test` + `curl` → commit (com o pedido do dono).

## Grafo de dependências

```
T1 coluna updated_at + migration 0004 + updatedAt nas respostas de POST e GET
 └── T2 DELETE /transactions/:id   (cria parseId e notFound, reaproveitados pelo PATCH)
      └── T3 PATCH /transactions/:id: caminho feliz, updatedAt, isolamento, 404, 401
           └── T4 PATCH /transactions/:id: validação (todos os 400)
                └── T5 documentação (contrato do front, AGENTS.md, spec)
```

Tudo sequencial: T1–T4 mexem em `src/routes/transactions.ts`, e o `PATCH` precisa do `updated_at` (T1)
e dos helpers de id (T2).

## Architecture Decisions

- **Migration sozinha no começo (T1)**, com revisão do SQL pelo dono antes do `db:migrate`: é a única parte
  irreversível no banco de dev. O `drizzle-kit` gera só o `ADD COLUMN ... DEFAULT now() NOT NULL`; a `0004` ganha,
  **antes de ser aplicada**, um `UPDATE "transactions" SET "updated_at" = "created_at";` logo em seguida (as linhas
  antigas ficariam com o horário da migration). Editar a migration nova antes de aplicá-la é permitido; o
  snapshot em `drizzle/meta` só descreve o schema e não muda com o `UPDATE`.
- **T1 é uma fatia vertical completa**: coluna + `publicColumns.updatedAt` + testes. Assim o `POST` e o `GET`
  já devolvem `updatedAt` antes de existir edição (`updatedAt === createdAt`, os dois do mesmo `now()` do insert).
- **`DELETE` antes do `PATCH`**: é a rota mais simples e introduz o que as duas usam:
  - `parseId(params)`: `z.object({ id: z.uuid() })`; id que não é UUID vira `undefined` → `404` sem ir ao banco
    (conferido: `z.uuid()` do Zod 4.6 aceita o `crypto.randomUUID()`/`gen_random_uuid()` e recusa `"abc"`);
  - `notFound(reply)`: `404` `{ "error": "Registro não encontrado" }`, local do `transactions.ts` (um só arquivo usa).
- **Uma consulta por operação, com `id` e `user_id` no mesmo `WHERE`**: `UPDATE ... RETURNING` e
  `DELETE ... RETURNING id`. Zero linhas = `404`, seja id inexistente, seja de outro usuário. Sem "ler e depois
  gravar", então não há janela entre conferir o dono e alterar.
- **Schema do `PATCH` = `createTransactionSchema.partial()` + `refine` "ao menos um campo"**. Conferido no Zod 4.6:
  `{}` e `{ foo: 1 }` caem no refine; `null` no campo → `required`; `trim` e mensagens do `POST` mantidos; `null`/`[]`
  na raiz → `INVALID_BODY`. Campos ausentes ficam fora do objeto de saída.
- **`.set()` com os quatro campos mapeados (`amount` → `amountCents`, `date` → `occurredOn`) + `updatedAt: sql\`now()\``**.
  Conferido no Drizzle 0.45 (`mapUpdateSet`): chaves `undefined` são descartadas, então só os campos enviados entram
  no `SET`; como o `updatedAt` sempre vai, o `SET` nunca fica vazio. `now()` do banco, e não `new Date()` do Node:
  o mesmo relógio do `createdAt`.
- **Testes novos em `test/transactions-update-delete.test.ts`** (T2–T4): o `test/transactions.test.ts` já tem 540 linhas
  e cobre `POST`/`GET`. Os testes de `updatedAt` no `POST`/`GET` (T1) ficam no `transactions.test.ts`.

## Risks and Mitigations

| Risco | Impacto | Mitigação |
|---|---|---|
| Linhas antigas com `updated_at` = horário da migration | Médio | `UPDATE ... = created_at` na `0004` antes de aplicar; conferir no `psql` depois do `db:migrate` |
| Teste de "`updatedAt` mudou" instável: `POST` e `PATCH` no mesmo milissegundo | Médio | No teste, recuar `created_at`/`updated_at` do registro direto no banco (ex.: `2026-01-01`) antes do `PATCH` e comparar com esse valor |
| Esquecer o `user_id` no `WHERE` de `UPDATE`/`DELETE` | Alto | Teste de isolamento em cada rota: o B recebe `404` e o registro do A continua intacto (conferido pelo `GET` do A) |
| `:id` não UUID chegar ao Postgres (`500`) | Médio | `parseId` antes da consulta; teste com `/transactions/abc` |
| Rota nova mudar o `404` de `DELETE /transactions` (sem id) em `test/errors.test.ts:134` | Baixo | `/transactions/:id` não casa com `/transactions`; o teste existente tem de continuar verde sem mudança |
| Testes que comparam o formato exato do registro (`transactions.test.ts:46` e `:315`) quebrarem | Baixo (esperado) | Ganham só `updatedAt: expect.any(String)`; nenhum outro teste existente muda |

## Open Questions

Nenhuma.
