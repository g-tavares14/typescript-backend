# Spec: Registros financeiros (entradas e saídas)

Status: **concluída.** Plano em [tasks/plan.md](tasks/plan.md), tarefas em [tasks/todo.md](tasks/todo.md).

## Objetivo

Primeira etapa da parte financeira: cada usuário logado registra as próprias **entradas** e **saídas**
e consulta os registros com os totais. Os registros de um usuário nunca aparecem para outro.

Nesta etapa só existem os dois tipos (entrada e saída). Categorias (alimentação, salário...),
edição e exclusão ficam para depois.

A etapa termina com uma refatoração sem mudança de comportamento: a chamada manual de `authenticate()`
em cada rota protegida vira um hook `onRequest` (`requireAuth`), que roda antes do parse do corpo: quem não está autenticado recebe 401 sem que o corpo seja lido.

### Critérios de aceite (comportamento da API)

**Autenticação e isolamento**
- As duas rotas exigem `Authorization: Bearer <token>`. Sem token, token inválido ou revogado:
  `401` igual ao `/users/me` (`WWW-Authenticate: Bearer`, `{ "error": "Não autenticado" }`).
- O dono do registro vem **sempre do token**, nunca da requisição. Um `userId` no corpo é ignorado.
- `GET` devolve só os registros do usuário do token. O usuário A nunca vê nem soma registros do B.

**`POST /transactions`** — cria um registro

Corpo:
```json
{ "type": "expense", "amount": 1990, "description": "Almoço", "date": "2026-09-29" }
```

| Campo | Regra | Erro (`400`) |
|---|---|---|
| `type` | `"income"` (entrada) ou `"expense"` (saída) | `O tipo deve ser income ou expense` |
| `amount` | inteiro em **centavos**, `> 0` e `<= 100000000000` (R$ 1 bilhão). `1990` = R$ 19,90. Rejeita `19.9`, `0` e negativos | `O valor deve ser um número inteiro de centavos maior que zero` |
| `description` | texto; `trim()`; de 1 a 200 caracteres | `A descrição deve ter entre 1 e 200 caracteres` |
| `date` | `AAAA-MM-DD` válida (rejeita `2026-02-30`); passado ou futuro | `Data inválida (use AAAA-MM-DD)` |

- Campo ausente ou com tipo JSON errado: `400` `{ "error": "Campo obrigatório ausente ou inválido" }` (a mesma mensagem do cadastro).
  Ex.: `amount: "1990"` (string), `amount: null`, `type: 123`, `description: 5`, `date: 20260929`.
  A mensagem específica da tabela só vale quando o tipo JSON está certo e o valor fora da regra
  (`type: "foo"`, `amount: 19.9`, `date: "29/09/2026"`).
- Campos a mais (inclusive `userId`, `id`, `createdAt`) são ignorados.
- Sucesso: `201` com o registro criado:

```json
{
  "id": "0b6c...",
  "type": "expense",
  "amount": 1990,
  "description": "Almoço",
  "date": "2026-09-29",
  "createdAt": "2026-09-29T14:03:12.345Z"
}
```

**`GET /transactions`** — lista os registros e os totais

- Query opcional: `?from=AAAA-MM-DD&to=AAAA-MM-DD` (inclusivas). Pode mandar só uma das duas.
- Data inválida → `400` `Data inválida (use AAAA-MM-DD)`. `from` depois de `to` → `400`
  `A data inicial deve ser anterior ou igual à final`.
- Ordem: `date` mais recente primeiro; empate desempata por `createdAt` mais recente.
- Os totais usam **o mesmo filtro** da lista e são calculados no banco.
- Sem registros: `200` com lista vazia e totais `0`.

```json
{
  "summary": { "income": 500000, "expense": 120000, "balance": 380000 },
  "transactions": [
    { "id": "...", "type": "expense", "amount": 1990, "description": "Almoço", "date": "2026-09-29", "createdAt": "..." }
  ]
}
```

`balance = income - expense` e pode ser negativo.

### Contrato para o frontend

| Ponto | O que o front faz |
|---|---|
| Valores em centavos (inteiro) | Converter ao exibir (`1990` → `R$ 19,90`) e ao enviar (`19,90` → `1990`) |
| `type` em inglês (`income`/`expense`) | Traduzir para "Entrada"/"Saída" na tela |
| `date` é só o dia, sem hora nem fuso | Enviar a data **local** do usuário; exibir como está |
| Filtro de mês | `?from=2026-09-01&to=2026-09-30` |

## Modelo de dados

Nova tabela `transactions` (migration `0003`, gerada pelo `drizzle-kit`):

| Coluna | Tipo | Regra |
|---|---|---|
| `id` | `uuid` | PK, `defaultRandom()` |
| `user_id` | `uuid` | `NOT NULL`, FK → `users.id` `ON DELETE CASCADE` |
| `type` | `text` | `NOT NULL`, `CHECK (type IN ('income', 'expense'))` |
| `amount_cents` | `bigint` | `NOT NULL`, `CHECK (amount_cents > 0)` |
| `description` | `varchar(200)` | `NOT NULL` |
| `occurred_on` | `date` | `NOT NULL` (na API se chama `date`) |
| `created_at` | `timestamptz` | `NOT NULL`, `DEFAULT now()` |

- Índice `transactions_user_id_occurred_on_idx` em `(user_id, occurred_on)`: toda consulta filtra por
  usuário e ordena/filtra por data.
- Os `CHECK`s repetem regras da API como defesa em profundidade (mesma lógica do username).
- `amount_cents` como `bigint` com `mode: "number"` no Drizzle (vira `number` no TypeScript;
  seguro até 2^53, muito acima do limite de R$ 1 bilhão por registro).
- `occurred_on` como `date` com o modo padrão do Drizzle (`string` `"AAAA-MM-DD"`): sem conversão
  para `Date` do JS, então sem erro de fuso horário.
- A soma no Postgres (`sum` de `bigint`) volta como `numeric`, que o driver `pg` entrega como **string**:
  o resultado precisa ser convertido para `number` (ex.: `.mapWith(Number)`) e `coalesce(..., 0)` para
  quando não houver registros.

## Tech Stack

A atual (ver AGENTS.md). **Nenhuma dependência nova.**

APIs conferidas na versão instalada: `bigint(name, { mode: "number" })`, `date(name)` (modo string),
`sum()` retornando `SQL<string | null>` (Drizzle 0.45) e `z.iso.date()` rejeitando `2026-02-30` (Zod 4).

## Commands

```bash
docker compose up -d
npm run typecheck
npm test
npm run db:generate -- --name transactions     # gera drizzle/0003_transactions.sql a partir do schema.ts
npm run db:migrate                             # aplica no banco de dev (depois de revisar o SQL)
npm run dev
```

## Project Structure

```
src/db/schema.ts               # nova tabela transactions + type Transaction
src/routes/transactions.ts     # novo: POST / e GET / (plugin com o db nas opções)
src/app.ts                     # registra transactionsRoutes com prefix "/transactions"; decorateRequest("user")
src/lib/validation.ts          # badRequest() e a mensagem required, saídos do auth.ts
src/lib/authenticate.ts        # (refatoração) requireAuth(db) e currentUser(request)
src/routes/users.ts, auth.ts   # (refatoração) usam requireAuth no lugar da chamada manual
drizzle/0003_transactions.sql  # nova migration (+ meta); nenhuma migration antiga é editada
test/transactions.test.ts      # novo
test/helpers.ts                # resetDatabase também limpa transactions
AGENTS.md                      # etapa atual, estrutura, roteiro e decisões
```

## Code Style

O mesmo das rotas atuais: schema Zod no topo, `safeParse` + `400` com a primeira mensagem,
`returning` com só os campos que a API expõe. A autenticação mudou na T6: o exemplo abaixo é o
**estilo da T2 a T5, antes da refatoração da T6** (`authenticate()` no começo de cada rota).

```ts
app.post("/", async (request, reply) => {
  const user = await authenticate(request, db);
  if (!user) {
    return unauthorized(reply);
  }

  const parsed = createTransactionSchema.safeParse(request.body);
  if (!parsed.success) {
    return badRequest(reply, parsed.error);
  }

  // userId vem do token, nunca do corpo.
  const [transaction] = await db
    .insert(transactions)
    .values({ ...toColumns(parsed.data), userId: user.id })
    .returning(publicColumns);

  return reply.code(201).send(transaction);
});
```

O `badRequest()` hoje é local do `auth.ts`; como passa a ter dois usos, vai para `src/lib/`.

O trecho acima é o estilo das tarefas de rota. Na refatoração final, as 3 primeiras linhas saem do handler:
o hook `requireAuth(db)` do plugin faz a autenticação, e a rota lê o usuário com `currentUser(request)`.

## Testing Strategy

- Vitest + `app.inject()` contra o banco `_test`, como hoje. TDD: cada tarefa começa com o teste falhando.
- `test/transactions.test.ts` cobre: 401 nas duas rotas; `POST` feliz (201 e corpo); cada erro de validação
  da tabela acima; `userId` no corpo ignorado; `GET` vazio (totais 0); totais e saldo (inclusive negativo);
  ordenação; filtro `from`/`to` (limites inclusivos, só um dos dois, `from > to`, data inválida);
  **isolamento** entre dois usuários; token revogado por logout → 401.
- `resetDatabase` passa a limpar `transactions` junto com `users`.
- Toda tarefa termina com `npm run typecheck`, `npm test` e um teste com `curl` no servidor real.

## Boundaries

- **Sempre**: teste falhando antes do código; migration nova (nunca editar 0000–0002); revisar o SQL
  gerado antes de aplicar; `{ "error": ... }` em todos os erros; `user_id` só do token.
- **Perguntar antes**: commits; qualquer dependência nova; rotas de editar/excluir ou categorias (fora do escopo).
- **Nunca**: aceitar `userId` da requisição; consultar `transactions` sem `WHERE user_id = <usuário do token>`;
  float para dinheiro; logar corpo de requisição com dados financeiros.

## Success Criteria

- [x] Todos os critérios de aceite acima têm teste automatizado passando (`test/transactions.test.ts` e `test/require-auth.test.ts`).
- [x] `npm run typecheck` e `npm test` verdes (os testes antigos continuam passando).
- [x] SQL da `0003` revisado (FK com cascade, dois `CHECK`s, índice) e aplicado no banco de dev sem erro.
- [x] `curl` no servidor real: login → 2 `POST` (entrada e saída) → `GET` mostra os dois e o saldo certo;
      um segundo usuário recebe lista vazia.
- [x] Refatoração: nenhum handler chama `authenticate()` diretamente; nenhum teste existente foi alterado
      e todos continuam verdes.
- [x] AGENTS.md atualizado (etapa atual, estrutura, roteiro, decisões).

## Resumo do contrato para o frontend

Tudo abaixo exige `Authorization: Bearer <token>`. O token vem de `POST /auth/login` (`{ "token": "...", "tokenType": "Bearer", "expiresIn": 3600 }`)
e vale 1 hora; depois do `POST /auth/logout` (em qualquer dispositivo) ele deixa de valer.

| Rota | Sucesso | Erros |
|---|---|---|
| `POST /transactions` | `201` com o registro criado | `400`, `401` |
| `GET /transactions` (`?from=&to=` opcionais) | `200` com `summary` e `transactions` | `400`, `401` |

**Regras dos dados**
- **Valores em centavos, inteiros:** `1990` = R$ 19,90. Nada de `19.90` nem string (`"1990"`). Máximo `100000000000` (R$ 1 bilhão).
- **Datas `AAAA-MM-DD`,** sem hora nem fuso; mande a data local do usuário. Pode ser no futuro. `from` e `to` do filtro têm o mesmo formato e são inclusivos.
- **`type`:** `"income"` (entrada) ou `"expense"` (saída), exatamente assim (minúsculas, em inglês).
- **`description`:** de 1 a 200 caracteres (espaços nas pontas são removidos).
- A resposta nunca traz `userId`. Campos a mais no corpo são ignorados.

**`POST /transactions`**
```json
// request
{ "type": "expense", "amount": 1990, "description": "Almoço", "date": "2026-09-29" }
// 201
{ "id": "0b6c...", "type": "expense", "amount": 1990, "description": "Almoço", "date": "2026-09-29", "createdAt": "2026-09-29T14:03:12.345Z" }
```

**`GET /transactions?from=2026-09-01&to=2026-09-30`**
```json
// 200: mais recente primeiro (empate: criado por último primeiro); os totais seguem o mesmo filtro
{
  "summary": { "income": 500000, "expense": 120000, "balance": 380000 },
  "transactions": [
    { "id": "...", "type": "expense", "amount": 1990, "description": "Almoço", "date": "2026-09-29", "createdAt": "..." }
  ]
}
```
`summary` sempre tem os três campos como número inteiro em centavos (`0` sem registros). `balance = income - expense` e pode ser negativo.

**Erros** (sempre `{ "error": "mensagem" }`)

| Status | Quando | `error` |
|---|---|---|
| `401` | sem token, token inválido, expirado ou revogado (header `WWW-Authenticate: Bearer`) | `Não autenticado` |
| `400` | campo ausente, tipo JSON errado (ex.: `amount` string) ou `from`/`to` repetido na query | `Campo obrigatório ausente ou inválido` |
| `400` | `type` fora de `income`/`expense` | `O tipo deve ser income ou expense` |
| `400` | `amount` decimal, `0`, negativo ou acima do teto | `O valor deve ser um número inteiro de centavos maior que zero` |
| `400` | `description` vazia, só espaços ou com mais de 200 caracteres | `A descrição deve ter entre 1 e 200 caracteres` |
| `400` | `date`, `from` ou `to` que não é uma data AAAA-MM-DD válida (ex.: `2026-02-30`, `29/09/2026`) | `Data inválida (use AAAA-MM-DD)` |
| `400` | `from` depois de `to` | `A data inicial deve ser anterior ou igual à final` |
| `400` | corpo ausente, vazio, não-JSON ou que não é um objeto JSON (`null`, `[]`, `"x"`, `text/plain`) | mensagem em inglês (Fastify/Zod), por exemplo `Invalid input: expected object, received null`, `Body cannot be empty when content-type is set to 'application/json'` ou `Body is not valid JSON but content-type is set to 'application/json'`; não dependa do texto |
| `500` | erro interno | `Erro interno do servidor` |

Só as mensagens em português desta tabela são estáveis: para o `400` de corpo inválido, decida só pelo status, não pelo texto.
Sem token, o `401` vem antes de qualquer `400`: a validação só roda para quem está autenticado.

**Sem paginação:** `GET /transactions` devolve todos os registros do filtro de uma vez. Telas de listagem devem sempre enviar `from` e `to` (por exemplo, o mês exibido); sem filtro, a resposta cresce com o histórico inteiro do usuário.

## Premissas (corrija se alguma estiver errada)

1. Rota em `/transactions` (não `/users/me/transactions`): o recurso é sempre do usuário do token, como o `/users/me`.
2. Nomes em inglês no código e na API (`income`/`expense`, `amount`, `date`); mensagens de erro em português.
3. Só real (BRL). Nada de moeda na tabela.
4. `date` é obrigatório e pode estar no futuro (conta agendada). O servidor não assume "hoje",
   porque o dia depende do fuso do usuário.
5. Sem rate limit nas duas rotas: exigem login e não calculam hash. Fica registrado como risco.
6. Sem paginação: o filtro por período mantém a resposta pequena no uso normal (um mês).

## Open Questions

Nenhuma.

Decidido: **`authenticate()` explícito** nas rotas financeiras e troca por hook `onRequest` (não `preHandler`: o 401 vem antes do parse do corpo) como
**última tarefa de código desta etapa** (opção A). Motivos: uma mudança por vez para revisar, os testes de 401
provam que a refatoração não abriu nenhuma rota, e com só 2 rotas financeiras o risco de esquecer
a autenticação até lá é baixo.

## Riscos (fora do escopo)

- Um usuário logado pode criar registros sem limite e encher o banco. Mitigação futura: rate limit por
  usuário no `POST` ou um teto de registros por conta.
- Sem paginação, um filtro de período muito grande (ou nenhum filtro) devolve tudo de uma vez.
- `ON DELETE CASCADE`: quando existir exclusão de conta, os registros financeiros somem junto. É o
  esperado (dados pessoais), mas é irreversível.
