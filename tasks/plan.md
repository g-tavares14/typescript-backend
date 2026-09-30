# Implementation Plan: Editar e excluir a própria conta

Spec: [SPEC-users-update-delete.md](../SPEC-users-update-delete.md) (aprovada). Tarefas em [todo.md](todo.md).
O plano anterior (edição e exclusão de registros financeiros) foi concluído e está no histórico do git (commit `45607bd`).

## Overview

Criar `PATCH /users/me` (altera `username` e/ou `email`, com as regras do cadastro e `409` para duplicado) e
`DELETE /users/me` (senha atual no corpo, `204`, apaga a conta e os registros pelo `CASCADE`), as duas com rate limit
por IP. Sem migration e sem dependência nova. TDD em cada tarefa: teste falhando → código → `npm run typecheck` +
`npm test` + `curl` → commit (com o pedido do dono).

## Grafo de dependências

```
T1 regras de username/email e isUniqueViolation num lugar compartilhado (refatoração, sem mudar comportamento)
 └── T2 PATCH /users/me: caminho feliz, 409, 401
      └── T3 PATCH /users/me: validação (todos os 400)
           └── T4 DELETE /users/me: senha, 204 + CASCADE, 403, 400, 401
                └── T5 rate limit nas duas rotas
                     └── T6 documentação (AGENTS.md, spec)
```

Tudo sequencial: T2–T5 mexem em `src/routes/users.ts`. T4 não depende da lógica do `PATCH`, mas vem depois para
não haver duas tarefas abertas no mesmo arquivo.

## Architecture Decisions

- **Refatoração primeiro e separada (T1)**: mover `usernameSchema`, `emailSchema` e `isUniqueViolation` para fora do
  `auth.ts` sem mudar nada no comportamento. Os testes de `register.test.ts` e `login.test.ts` são a rede de segurança:
  têm de continuar verdes **sem nenhuma alteração**. Assim, o diff das rotas novas (T2–T4) fica só com código novo.
  - `src/lib/user-fields.ts`: `usernameSchema` (`trim` + minúsculas + tamanho + regex) e `emailSchema`, com as
    mensagens atuais. O `registerSchema` e o `loginSchema` passam a usá-los.
  - `src/lib/db-errors.ts`: `isUniqueViolation(error)` (`DrizzleQueryError` + `pg.DatabaseError` + código `23505`).
- **Duas exportações novas em `src/lib/authenticate.ts`** (pequeno desvio da spec, que dizia "nenhuma mudança" nesse arquivo):
  - `unauthorized(reply)` passa a ser exportada: o `PATCH` e o `DELETE` respondem o `401` padrão quando a conta some
    no meio do caminho (0 linhas), sem copiar a resposta.
  - `publicUserColumns`: as colunas que o `requireAuth` já seleciona (`id`, `username`, `email`, `role`, `createdAt`)
    viram uma constante. O `.returning(publicUserColumns)` do `PATCH` sai então **exatamente** no formato do
    `GET /users/me`, e um campo novo no futuro entra nos dois de uma vez.
  Nenhum comportamento do `requireAuth` muda.
- **`PATCH`: uma consulta só**: `UPDATE users SET <campos enviados> WHERE id = <token> RETURNING ...`. O `UNIQUE` do banco
  decide o `409` (sem `SELECT` antes para "ver se o email existe", que teria corrida). Schema =
  `z.object({ username: usernameSchema, email: emailSchema }, INVALID_BODY).partial().refine(ao menos um campo)`,
  o mesmo padrão do `PATCH` de transactions (Zod 4.6: chave ausente fica fora da saída; o Drizzle 0.45 descarta
  `undefined` no `.set()`). O refine garante que o `SET` nunca fica vazio (o Drizzle lança erro com `.set({})`).
- **`DELETE`: ler o hash, verificar, apagar**: `SELECT password_hash WHERE id = <token>` → `verifyPassword()` →
  `DELETE WHERE id = <token> RETURNING id`. Aqui "ler e depois gravar" é aceitável: o id vem do token (não há dono a
  conferir), e a única corrida possível (dois `DELETE` juntos) termina em `0` linhas → `401`. O `currentUser()` continua
  sem o `passwordHash`, de propósito.
- **`403` `Senha incorreta`** no `DELETE` com senha errada (spec, premissa 5). Validação da senha igual à do login:
  `z.string(required).min(1, required.error)`, sem tamanho mínimo.
- **Rate limit por rota** (`config.rateLimit`), como login e cadastro: `DELETE` 5/min, `PATCH` 10/min.
  O `@fastify/rate-limit` 11 põe o hook dele no fim do `onRequest` da rota (`routeOptions.onRequest.push`), então ele
  roda **depois** do `requireAuth` do plugin: requisição sem token recebe `401` e não conta no limite. O teste do
  `429` usa um token válido.
- **Log**: o `sendError` loga só a `query` dos erros do Drizzle, sem os parâmetros (`src/lib/errors.ts:44`), então o
  email do `PATCH` não vai para o log num `500`. A senha do `DELETE` nunca entra numa consulta.
- **Testes novos em `test/users-update-delete.test.ts`** (T2–T4) e os limites em `test/rate-limit.test.ts` (T5).
  O `users-me.test.ts` continua só com o `GET`.

## Risks and Mitigations

| Risco | Impacto | Mitigação |
|---|---|---|
| A refatoração (T1) mudar uma mensagem ou a ordem da normalização do cadastro | Médio | T1 isolada; `register.test.ts` e `login.test.ts` verdes sem alteração; diff só de movimentação |
| `PATCH` aceitar `role` do corpo | Alto | O schema só tem `username` e `email`, e o `.set()` recebe só a saída do Zod; teste com `role: "admin"` confere no banco que continua `user` |
| `DELETE` apagar sem conferir a senha | Alto | Teste de senha errada: `403` e a conta continua (o login ainda funciona) |
| `CASCADE` apagar registros de outro usuário | Alto | Teste com dois usuários: depois do `DELETE` do A, os registros do B continuam no `GET` do B |
| Teste da corrida (conta apagada entre o `requireAuth` e a consulta) difícil de montar | Baixo | App próprio no teste com um hook `preHandler` que apaga o usuário antes do handler; se o Fastify não permitir, fica coberto pela leitura do código e isso é relatado |
| `DELETE` com corpo JSON diferente do `DELETE /transactions/:id` (sem corpo) confundir o front | Baixo | Contrato na spec explica os dois; testes de `INVALID_BODY` e `415` |
| Rate limit por IP bloquear usuários atrás do mesmo NAT | Baixo | Limites de conta pessoal (5 e 10 por minuto); `trustProxy` já registrado para quando houver proxy |

## Open Questions

Nenhuma. Catálogo de skills conferido: nenhuma tarefa pede skill nova (sem migration, sem frontend, sem métricas).
