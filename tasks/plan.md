# Implementation Plan: Registros financeiros (entradas e saídas)

Spec: [SPEC-transactions.md](../SPEC-transactions.md) (aprovada). Tarefas em [todo.md](todo.md).
O plano anterior (endurecimento da autenticação) foi concluído e está no histórico do git (commit `6b94780`).

## Overview

Criar a tabela `transactions` ligada a `users`, as rotas `POST /transactions` e `GET /transactions`
(lista + totais, com filtro opcional por período) e, por último, trocar a chamada manual de
`authenticate()` por um hook `preHandler` em todas as rotas protegidas. TDD em cada tarefa:
teste falhando → código → `npm run typecheck` + `npm test` + `curl` → commit (com o pedido do dono).

## Grafo de dependências

```
T1 tabela transactions + migration 0003 (+ resetDatabase)
 └── T2 POST /transactions: caminho feliz, 401, userId do token   (move badRequest para src/lib)
      └── T3 POST /transactions: validação (todos os 400)
           └── T4 GET /transactions: lista + totais + isolamento
                └── T5 GET /transactions: filtro from/to
                     └── T6 refatoração: authenticate() → hook preHandler (4 rotas protegidas)
                          └── T7 documentação (AGENTS.md, spec, contrato para o front)
```

Tudo é sequencial: T2–T5 mexem nos mesmos dois arquivos (`src/routes/transactions.ts` e
`test/transactions.test.ts`) e T6 depende de todas as rotas protegidas já existirem com testes de 401.

## Architecture Decisions

- **Migration sozinha na T1.** É a parte de maior risco e a única irreversível no banco de dev: sai
  primeiro e o SQL gerado é revisado antes de qualquer rota (fail fast).
- **Fatias verticais por comportamento**: primeiro o `POST` que funciona (T2), depois as regras de
  validação (T3), depois a leitura (T4) e o filtro (T5). Cada tarefa deixa a API funcionando e testável com `curl`.
- **Totais calculados no banco numa consulta separada da lista**, com o mesmo `WHERE`
  (`user_id` + período). Um `sum(...) FILTER (WHERE type = ...)` por tipo, `coalesce(..., 0)` e
  `.mapWith(Number)`, porque o `sum` de `bigint` volta como `numeric`, que o `pg` entrega como string.
  O `balance` é `income - expense` calculado em JS a partir dos dois números.
- **Um helper de condições** (`userId` + `from`/`to`) é reaproveitado pela lista e pelos totais, para
  que os dois nunca usem filtros diferentes. A condição `eq(transactions.userId, user.id)` fica sempre lá.
- **`badRequest()` vai para `src/lib/`** na T2: passa a ter dois usos (`auth.ts` e `transactions.ts`).
  A mensagem `required` ("Campo obrigatório ausente ou inválido") vai junto.
- **Colunas públicas num objeto só** (`id`, `type`, `amount`, `description`, `date`, `createdAt`),
  usado no `.returning()` e no `.select()`: a API nunca expõe `user_id` e os nomes da API
  (`amount`, `date`) ficam mapeados num lugar só.
- **Refatoração na T6 (opção A da spec)**, sem mudar comportamento:
  - `requireAuth(db)` em `src/lib/authenticate.ts` devolve um `preHandler` que chama `authenticate()`,
    responde `unauthorized()` se falhar e guarda o usuário em `request.user`.
  - Hook no plugin inteiro em `users.ts` e `transactions.ts` (todas as rotas são protegidas);
    na rota `/auth/logout`, `{ preHandler: requireAuth(db) }` (o plugin `/auth` tem rotas públicas).
  - `decorateRequest("user", null)` uma vez no `buildApp` + declaration merging
    (`interface FastifyRequest { user: AuthUser | null }`).
  - As rotas leem o usuário por um helper `currentUser(request)` que devolve `AuthUser` (sem `null`) e
    **lança erro** se o hook não rodou. Assim o TypeScript não precisa do `!`, e se alguém esquecer o hook
    a rota falha com 500 em vez de responder com dados de outra pessoa.
  - Prova de que o comportamento não mudou: **nenhum teste existente é alterado** e todos continuam verdes.

## Risks and Mitigations

| Risco | Impacto | Mitigação |
|---|---|---|
| `TRUNCATE TABLE users` passa a falhar por causa da chave estrangeira | Alto (todos os testes quebram) | T1 muda o `resetDatabase` para `TRUNCATE TABLE users, transactions` |
| `drizzle-kit` gerar FK, `CHECK` ou índice diferente do esperado | Médio | Revisar o SQL da 0003 antes do `db:migrate`; nunca editar 0000–0002 |
| Totais voltarem como string (`"1990"`) ou `null` sem registros | Médio (quebra o front) | Testes com `toEqual` e números exatos; caso "sem registros → 0" |
| Vazamento entre usuários (esquecer o `WHERE user_id`) | Alto (dados financeiros) | Teste de isolamento com dois usuários na T4 e na T5; helper de condições único |
| `from`/`to` inclusivos errados (`<` no lugar de `<=`) | Médio | Testes nos limites exatos do período |
| Hook da T6 não cobrir alguma rota (encapsulamento, ordem de registro) | Alto | Os testes de 401 de `/users/me`, `/auth/logout` e `/transactions` falhariam; mais `curl` sem token em cada rota |
| Declaration merging tipar `request.user` em rotas públicas | Baixo | `currentUser()` é o único jeito de ler; ele falha alto se não houver usuário |
| Contrato novo mal entendido pelo front (centavos, `income`/`expense`) | Médio | Tabela de contrato na spec + resumo final na T7 |

## Open Questions

Nenhuma: a opção A e a posição da refatoração foram decididas pelo dono.
