# Spec: Editar e excluir registros financeiros

Status: **concluída** (aprovada pelo dono em 2026-09-29). Continuação de [SPEC-transactions.md](SPEC-transactions.md).

## Objetivo

O usuário logado passa a **corrigir** (`PATCH`, só os campos que mudam) e **excluir** (`DELETE`) os próprios registros
de entrada e saída. As duas rotas usam o token do usuário, como `POST` e `GET`.

O registro ganha um único campo novo: **`updatedAt`**, a data e a hora da última modificação (coluna `updated_at`,
migration `0004`). Sem número de versão nem histórico de versões (decisão do dono), sem exclusão lógica,
nenhuma dependência nova.

### Critérios de aceite (comportamento da API)

**Autenticação e isolamento**
- As duas rotas exigem `Authorization: Bearer <token>`, pelo hook `requireAuth` que o plugin `transactions.ts` já tem.
  Sem token, token inválido ou revogado: `401` padrão (`WWW-Authenticate: Bearer`, `{ "error": "Não autenticado" }`),
  antes de qualquer `400`, `404`, `413` ou `415`.
- Um registro só é editado ou excluído pelo seu dono. A condição é `id = :id AND user_id = <usuário do token>` **na própria
  consulta** de `UPDATE`/`DELETE` (uma consulta só, sem "ler e depois gravar").
- Registro de **outro usuário** responde **igual a um id que não existe**: `404` `Registro não encontrado`.
  Nunca `403`: o `403` confirmaria que o id existe.

**`updatedAt` (novo em todas as respostas com registro)**
- `POST`, `GET` e `PATCH` passam a devolver `updatedAt` (ISO 8601, como o `createdAt`).
- Registro nunca editado: `updatedAt` **igual** ao `createdAt`. Nunca é `null`.
- Cada `PATCH` com sucesso grava `updated_at = now()` (relógio do banco, o mesmo do `createdAt`), mesmo que os valores
  enviados sejam iguais aos atuais; o `createdAt` não muda.
- `PATCH` recusado (`400`, `404`) não altera nada, nem o `updatedAt`.
- Os registros que já existem recebem `updated_at = created_at` na migration (e não a data em que a migration rodou).

**`PATCH /transactions/:id`**: altera só os campos enviados

- O id vai **na URL**. O corpo tem **só os campos que mudam**, qualquer combinação de `type`, `amount`, `description`
  e `date`. Campo não enviado continua como está.

```json
// PATCH /transactions/0b6c...   (muda só a descrição)
{ "description": "Almoço (corrigido)" }
```

- Cada campo enviado segue a mesma regra e a mesma mensagem do `POST` (tabela em `SPEC-transactions.md`).
  Tipo JSON errado, inclusive `null` (ex.: `"amount": null`, `"amount": "1990"`) → `400` `Campo obrigatório ausente ou inválido`.
  `null` não apaga nada: todos os campos do registro são obrigatórios.
- Corpo sem nenhum dos quatro campos (ex.: `{}` ou só campos desconhecidos) → `400` `Envie ao menos um campo para alterar`.
- Campos a mais (`id`, `userId`, `createdAt`, `updatedAt`) são ignorados: o `id` vem só da URL, o dono só do token,
  e as duas datas só do banco.
- Sucesso: `200` com o registro **inteiro** já atualizado:

```json
{
  "id": "0b6c...",
  "type": "expense",
  "amount": 1990,
  "description": "Almoço (corrigido)",
  "date": "2026-09-29",
  "createdAt": "2026-09-29T14:03:12.345Z",
  "updatedAt": "2026-09-30T09:15:40.120Z"
}
```

- Não cria registro se o id não existir: `404`.
- Duas edições simultâneas: cada uma grava só os próprios campos; se as duas mudarem o **mesmo** campo, vale a última.

**`DELETE /transactions/:id`**: exclui um registro

- Sucesso: `204` sem corpo. A exclusão é definitiva.
- Excluir de novo o mesmo id: `404` (o registro não existe mais).
- Não precisa de corpo. Se o front mandar `Content-Type: application/json` **sem corpo**, o Fastify responde
  `400` `Corpo da requisição inválido: envie um objeto JSON` (comportamento atual do framework, conferido no 5.12.5); o
  contrato orienta o front a não enviar `Content-Type` no `DELETE`.

**`:id` inválido**
- `:id` que não é UUID (ex.: `/transactions/abc`) → `404` `Registro não encontrado`, sem ir ao banco
  (o Postgres recusaria o texto como `uuid` e daria `500`).
- No `PATCH`, a ordem é: `401` → erros de parse do corpo do Fastify (`400`/`413`/`415`) → `:id` inválido (`404`) →
  validação dos campos enviados (`400`) → nenhum campo enviado (`400`) → registro inexistente ou de outro usuário (`404`).

**O que não muda**
- As regras de `POST` e `GET` continuam as mesmas; só a resposta ganha o `updatedAt`.
- A ordem do `GET` continua por `date` e `createdAt`: um registro editado não "sobe" na lista.

## Modelo de dados

Migration nova `0004` (gerada pelo `drizzle-kit`; as antigas não são editadas):

| Coluna | Tipo | Regra |
|---|---|---|
| `updated_at` | `timestamptz` | `NOT NULL`, `DEFAULT now()` |

- No insert, `created_at` e `updated_at` usam o `DEFAULT now()` da mesma instrução: o `now()` do Postgres é o horário
  do início da transação, então os dois saem **iguais**.
- O `drizzle-kit` gera só o `ADD COLUMN ... DEFAULT now() NOT NULL`, que preencheria as linhas antigas com o horário da
  migration. Por isso a `0004` ganha, **antes de ser aplicada**, um `UPDATE "transactions" SET "updated_at" = "created_at";`
  logo depois do `ADD COLUMN`. O dono revisa o SQL antes do `db:migrate`.

## Tech Stack

A atual (ver AGENTS.md). **Nenhuma dependência nova.**
APIs a conferir na versão instalada antes de usar: `z.uuid()` e `.partial()` (Zod 4.6: se campo ausente fica fora do
objeto de saída); `db.update(...).set(...)` ignorando chaves `undefined`, `.returning(...)` e
`db.delete(...).where(...).returning(...)` (Drizzle 0.45); gravar o `now()` do banco no `set` (`sql\`now()\``).

## Commands

```bash
docker compose up -d
npm run db:generate -- --name transactions_updated_at   # gera drizzle/0004_transactions_updated_at.sql
npm run db:migrate                                        # depois de acrescentar o UPDATE e revisar o SQL
npm run typecheck
npm test
npm run dev
```

## Project Structure

```
src/db/schema.ts               # coluna updatedAt na tabela transactions
drizzle/0004_*.sql (+ meta)    # nova migration (ADD COLUMN + UPDATE das linhas antigas)
src/routes/transactions.ts     # publicColumns ganha updatedAt; schema parcial; rotas PATCH /:id e DELETE /:id no mesmo plugin
test/transactions-update-delete.test.ts  # novo: testes do PATCH e do DELETE
test/transactions.test.ts      # updatedAt no POST e no GET; os 2 testes que conferem o formato exato do registro passam a esperar updatedAt
SPEC-transactions.md           # resumo do contrato para o front: updatedAt e as duas rotas
AGENTS.md                      # estrutura, roteiro e decisões
```

Nenhuma mudança em `app.ts`, `errors.ts` ou `authenticate.ts`.

## Code Style

O mesmo do `POST`: reaproveitar as regras de campo do `createTransactionSchema` (em versão parcial), `publicColumns`,
`badRequest()` e `currentUser()`. Esboço (a forma final fica para o plano e a implementação):

```ts
// Mesmas regras do POST, mas cada campo é opcional; exige ao menos um.
const updateTransactionSchema = createTransactionSchema
  .partial()
  .refine((body) => Object.keys(body).length > 0, { error: "Envie ao menos um campo para alterar" });

app.patch("/:id", async (request, reply) => {
  const user = currentUser(request);

  const id = parseId(request.params);            // z.uuid(); inválido -> undefined
  if (!id) {
    return notFound(reply);                      // 404 { error: "Registro não encontrado" }
  }

  const parsed = updateTransactionSchema.safeParse(request.body);
  if (!parsed.success) {
    return badRequest(reply, parsed.error);
  }
  const { type, amount, description, date } = parsed.data;

  // Campo undefined fica fora do SET. Dono e id na mesma condição: registro de outro usuário = 0 linhas = 404.
  const [transaction] = await db
    .update(transactions)
    .set({ type, amountCents: amount, description, occurredOn: date, updatedAt: sql`now()` })
    .where(and(eq(transactions.id, id), eq(transactions.userId, user.id)))
    .returning(publicColumns);

  if (!transaction) {
    return notFound(reply);
  }
  return reply.send(transaction);
});
```

## Testing Strategy

- Vitest + `app.inject()` no banco `_test`, TDD (teste falhando antes do código), como nas tarefas anteriores.
- `updatedAt`: no `POST` é igual ao `createdAt`; no `GET` aparece em cada registro; depois de um `PATCH` é maior que o
  `createdAt`, e o `createdAt` não muda; um `PATCH` recusado não altera o `updatedAt`.
- `PATCH` parcial: só `description` muda a descrição e mantém `type`, `amount` e `date`; o mesmo para cada um dos outros
  campos sozinhos; os quatro juntos; o `GET` e os totais refletem a mudança (ex.: `amount` ou `type` alterado muda o `summary`).
- `PATCH` inválido: `{}` e só campos desconhecidos → `400` `Envie ao menos um campo para alterar`; `null` e tipo JSON
  errado → `required`; valor fora da regra → mensagem do campo (reaproveitando os casos do `POST`: `amount: 19.9`,
  `type: "foo"`, `description: "   "`, `date: "2026-02-30"`); `id`, `userId`, `createdAt` e `updatedAt` no corpo ignorados.
- Nas duas rotas: `401` (sem token e token revogado por logout); `404` para id inexistente, id de **outro usuário**
  (e o registro do outro continua intacto) e `:id` não UUID. `DELETE` → `204` com corpo vazio, o registro some do `GET`
  e dos totais; `DELETE` repetido → `404`.
- Testes existentes: só os 2 que comparam o formato exato do registro ganham `updatedAt: expect.any(String)`; nenhum outro muda.
- Toda tarefa termina com `npm run typecheck`, `npm test` e `curl` no servidor real.

## Boundaries

- **Sempre**: `user_id` do token na condição de toda consulta; `404` igual para "não existe" e "é de outro usuário";
  migration nova (nunca editar `0000`–`0003`); revisar o SQL antes de aplicar; mensagens em português no formato `{ "error": ... }`.
- **Perguntar antes**: outro campo ou coluna (versão, histórico, exclusão lógica); `PUT` de substituição completa;
  rate limit nas rotas; alterar testes existentes além dos 2 citados; commits; dependências.
- **Nunca**: `UPDATE`/`DELETE` sem `WHERE user_id = <usuário do token>`; aceitar `userId`, `id` ou datas do corpo; `403` para
  registro de outro usuário; logar o corpo da requisição.

## Success Criteria

- [x] Todos os critérios de aceite acima com teste automatizado passando.
- [x] `npm run typecheck` e `npm test` verdes.
- [x] SQL da `0004` revisado (`ADD COLUMN` + `UPDATE` das linhas antigas) e aplicado no banco de dev; registros antigos com
      `updated_at = created_at`.
- [x] `curl` no servidor real: login → `POST` (`updatedAt` = `createdAt`) → `PATCH` só com `description` (200, demais campos
      iguais, `updatedAt` novo) → `PATCH` só com `amount` → `GET` mostra os valores novos e o saldo certo → `DELETE` (204)
      → `GET` sem o registro; um segundo usuário recebe `404` no `PATCH` e no `DELETE` do registro do primeiro.
- [x] Contrato do front (`SPEC-transactions.md`) e `AGENTS.md` atualizados.

## Resumo do contrato para o frontend (entra no `SPEC-transactions.md`)

Todo registro devolvido pela API passa a ter `updatedAt` (igual ao `createdAt` se nunca foi editado).

| Rota | Sucesso | Erros |
|---|---|---|
| `PATCH /transactions/:id` (corpo só com os campos que mudam) | `200` com o registro inteiro atualizado | `400`, `401`, `404` |
| `DELETE /transactions/:id` (sem corpo e sem `Content-Type`) | `204` sem corpo | `401`, `404` |

| Status | Quando | `error` |
|---|---|---|
| `400` | `PATCH` sem nenhum dos campos `type`, `amount`, `description`, `date` | `Envie ao menos um campo para alterar` |
| `404` | id inexistente, de outro usuário, já excluído ou que não é UUID | `Registro não encontrado` |

## Premissas (corrija se alguma estiver errada)

1. Atualização **parcial** (decidido pelo dono): o front envia o id na URL e só os campos que mudam.
   Por isso o método é `PATCH` (no HTTP, `PUT` significa substituir o registro inteiro).
2. `:id` que não é UUID → `404` (decidido pelo dono).
3. Só `updated_at`; sem `version` nem histórico (decidido pelo dono).
4. `updatedAt` igual ao `createdAt` em registro nunca editado, em vez de `null` (dono sem preferência; o front não precisa tratar `null`).
5. Edições simultâneas do mesmo campo: vale a última (dono sem preferência; controlar isso exigiria um campo de versão).
6. Exclusão definitiva (`DELETE` no banco), sem lixeira: exclusão lógica exigiria outra coluna.
7. Sem rate limit nas duas rotas, como `POST` e `GET` (exigem login, não calculam hash).

## Open Questions

Nenhuma.

## Riscos (fora do escopo)

- A exclusão é irreversível: um clique errado no front apaga o registro. Mitigação no front (confirmação).
- `updatedAt` diz quando foi a última edição, mas não o que mudou nem quantas edições houve.
- Duas edições simultâneas do mesmo campo: a segunda sobrescreve a primeira sem aviso.
