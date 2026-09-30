# Spec: Editar e excluir a própria conta

Status: **concluída** (aprovada pelo dono em 2026-09-30). Continuação de [SPEC-auth-hardening.md](SPEC-auth-hardening.md).

## Objetivo

O usuário logado passa a **alterar o próprio perfil** (`PATCH /users/me`, `username` e/ou `email`) e a **excluir a
própria conta** (`DELETE /users/me`, com a senha atual). As duas rotas agem sempre sobre o usuário do token, como o
`GET /users/me`.

Ainda não há ações de administrador sobre outras contas (`/users/:id`): quem pode alterar ou excluir outra conta
fica para uma spec própria, quando o dono definir as regras por `role`. Sem migration e sem dependência nova.

### Critérios de aceite (comportamento da API)

**Autenticação**
- As duas rotas exigem `Authorization: Bearer <token>`, pelo hook `requireAuth` que o plugin `users.ts` já tem.
  Sem token, token inválido ou revogado: `401` padrão (`WWW-Authenticate: Bearer`, `{ "error": "Não autenticado" }`),
  antes de qualquer `400`, `403`, `409`, `413` ou `415`.
- O usuário alterado ou excluído é **sempre** o do token. Nenhum id vem da URL nem do corpo.

**`PATCH /users/me`**: altera só os campos enviados

- O corpo tem **só os campos que mudam**, um deles ou os dois: `username` e `email`. Campo não enviado continua como está.

```json
// PATCH /users/me   (muda só o username)
{ "username": "joao_silva" }
```

- Cada campo enviado segue **a mesma normalização, regra e mensagem do cadastro** (`POST /auth/register`):
  - `username`: `trim` + minúsculas; 3 a 50 caracteres (`O username deve ter entre 3 e 50 caracteres`); só `a-z0-9_`
    (`O username só pode ter letras sem acento, números e _`).
  - `email`: `trim` + minúsculas; formato de email (`Email inválido`).
  - Tipo JSON errado, inclusive `null` (ex.: `"email": null`, `"username": 123`) → `400` `Campo obrigatório ausente ou inválido`.
- Corpo sem nenhum dos dois campos (ex.: `{}` ou só campos desconhecidos) → `400` `Envie ao menos um campo para alterar`.
- Campos a mais (`id`, `role`, `password`, `passwordHash`, `tokenVersion`, `createdAt`) são **ignorados**: a `role` nunca vem
  da requisição e a senha só vai mudar no futuro `PUT /users/me/password`. Um corpo **só** com eles cai no
  `Envie ao menos um campo para alterar`.
- `username` ou `email` já usado por **outra** conta → `409` `Email ou username já cadastrado` (a mesma mensagem do cadastro).
  Enviar o próprio valor atual (ex.: o mesmo email) não é conflito: `200`.
- Trocar o email **não pede a senha** (decisão do dono; ver Riscos).
- Sucesso: `200` com o usuário **no mesmo formato do `GET /users/me`**, já atualizado:

```json
{
  "id": "5f1e...",
  "username": "joao_silva",
  "email": "joao@email.com",
  "role": "user",
  "createdAt": "2026-09-01T12:00:00.000Z"
}
```

- Os tokens continuam valendo depois da alteração (o JWT leva o id, não o username nem o email): não muda o `token_version`.
- Depois de trocar o email, o login passa a ser com o email novo; o antigo dá `401` `Email ou senha inválidos`.
- `PATCH` recusado (`400`, `409`) não altera nada.
- Conta excluída entre o `requireAuth` e o `UPDATE` (corrida com um `DELETE` em outro dispositivo): `0` linhas → `401` padrão.

**`DELETE /users/me`**: exclui a própria conta

- Corpo JSON com a senha atual:

```json
// DELETE /users/me   (Content-Type: application/json)
{ "password": "senha123" }
```

- Sucesso: `204` sem corpo. A exclusão é **definitiva**: apaga o usuário e, pelo `ON DELETE CASCADE` que já existe,
  **todos os registros financeiros** dele.
- Depois disso, qualquer token da conta (de qualquer dispositivo) dá `401` padrão (o usuário não existe mais) e o login
  dá `401` `Email ou senha inválidos`. O email e o username ficam livres para um novo cadastro.
- Senha errada → `403` `Senha incorreta`, e nada é apagado. É `403`, não `401`: o token é válido, e o front trata
  `401` como "sessão expirou, faça login de novo".
- `password` ausente, vazia ou com tipo errado → `400` `Campo obrigatório ausente ou inválido` (a senha não tem tamanho
  mínimo aqui, como no login).
- Corpo que não é objeto JSON (ausente, vazio, malformado, `null`, `[]`) → `400` `Corpo da requisição inválido: envie um objeto JSON`.
  `Content-Type: text/plain` → também `INVALID_BODY` (o Fastify tem parser de texto e entrega uma string); tipo sem
  parser (ex.: `application/xml`) → `415` (o `sendError` já mapeia).
- Campos a mais no corpo são ignorados.
- Conta excluída entre o `requireAuth` e o `DELETE` (dois `DELETE` simultâneos): `0` linhas → `401` padrão.

**Rate limit** (por IP, em memória, como login e cadastro)
- `DELETE /users/me`: **5 por minuto**. Cada tentativa calcula um argon2 (64 MiB) e a rota é um oráculo de senha para
  quem tiver um token.
- `PATCH /users/me`: **10 por minuto**. O `409` revela se um email ou username tem conta; o cadastro limita isso a 3 por
  minuto, e o `PATCH` sem limite seria uma varredura mais rápida.
- Estourou: `429` `Muitas tentativas. Tente novamente mais tarde.` com `Retry-After` (comportamento atual do plugin).
- A ordem entre `401` e `429` não faz parte do contrato.

**O que não muda**
- `GET /users/me`, cadastro, login e logout continuam iguais.
- Nenhuma coluna nova: o usuário não ganha `updatedAt`.

## Modelo de dados

Sem migration. As regras já estão no banco:
- `UNIQUE` em `username` e `email` → `409` (código `23505`, o mesmo `isUniqueViolation` do cadastro).
- `CHECK` do formato do username (`users_username_format_check`): defesa em profundidade, a API já valida antes.
- `transactions.user_id` com `ON DELETE CASCADE`: o `DELETE` do usuário apaga os registros na mesma instrução.

## Tech Stack

A atual (ver AGENTS.md). **Nenhuma dependência nova.**
Já conferido no Fastify 5.12.5 instalado: `DELETE` está em `bodywith` (`node_modules/fastify/fastify.js`), então o corpo
JSON do `DELETE` é lido e validado como no `POST`.
APIs a conferir na versão instalada antes de usar: `.partial()` + `.refine` (Zod 4.6, igual ao `PATCH` de transactions);
`db.update(users).set(...)` ignorando chaves `undefined` e `.returning(...)`; `db.delete(users).where(...).returning(...)`
(Drizzle 0.45); `config.rateLimit` por rota (`@fastify/rate-limit` 11).

## Commands

```bash
docker compose up -d
npm run typecheck
npm test
npm run dev
```

## Project Structure

```
src/routes/users.ts        # PATCH /me e DELETE /me no mesmo plugin (o requireAuth do plugin já cobre as duas)
src/routes/auth.ts         # passa a importar as regras de username/email e o isUniqueViolation do lugar compartilhado
src/lib/user-fields.ts     # novo: usernameSchema e emailSchema (as regras do cadastro, usadas no register e no PATCH)
src/lib/db-errors.ts       # novo: isUniqueViolation (sai do auth.ts; usado no register e no PATCH)
test/users-update-delete.test.ts  # novo: testes do PATCH e do DELETE
test/rate-limit.test.ts    # limites do PATCH e do DELETE /users/me
AGENTS.md                  # etapa atual, estrutura, roteiro e decisões
```

Os nomes finais dos dois arquivos novos ficam para o plano. Nenhuma mudança em `app.ts`, `errors.ts` ou no schema do
banco (o `authenticate.ts` só passou a exportar `unauthorized` e `publicUserColumns`). Os testes de `register.test.ts`
não mudam: as regras são as mesmas, só mudam de arquivo.

## Code Style

O mesmo do `PATCH` de transactions: regras do cadastro reaproveitadas em versão parcial, `badRequest()`, `currentUser()`,
colunas públicas iguais às do `requireAuth`. Esboço (a forma final fica para o plano e a implementação):

```ts
// Mesmas regras do cadastro, mas cada campo é opcional; exige ao menos um.
const updateMeSchema = z
  .object({ username: usernameSchema, email: emailSchema }, INVALID_BODY)
  .partial()
  .refine((body) => Object.keys(body).length > 0, { error: "Envie ao menos um campo para alterar" });

app.patch("/me", { config: { rateLimit: UPDATE_ME_RATE_LIMIT } }, async (request, reply) => {
  const user = currentUser(request);

  const parsed = updateMeSchema.safeParse(request.body);
  if (!parsed.success) {
    return badRequest(reply, parsed.error);
  }

  try {
    // Campo undefined fica fora do SET. O id vem só do token.
    const [updated] = await db
      .update(users)
      .set(parsed.data)
      .where(eq(users.id, user.id))
      .returning(publicColumns);

    if (!updated) {
      return unauthorized(reply);   // a conta foi excluída no meio do caminho
    }
    return reply.send(updated);
  } catch (error) {
    if (isUniqueViolation(error)) {
      return reply.code(409).send({ error: "Email ou username já cadastrado" });
    }
    throw error;
  }
});
```

No `DELETE`, o hash vem do banco (o `currentUser()` não carrega o `passwordHash`, de propósito), é verificado com
`verifyPassword()` e só então o `DELETE ... WHERE id = <usuário do token>` roda.

## Testing Strategy

- Vitest + `app.inject()` no banco `_test`, TDD (teste falhando antes do código), como nas etapas anteriores.
- `PATCH` feliz: só `username`; só `email`; os dois; normalização (`"  Joao_Silva "` → `joao_silva`, email em minúsculas);
  o `GET /users/me` reflete a mudança; o mesmo token continua valendo; login com o email novo funciona e com o antigo dá `401`;
  enviar o próprio valor atual dá `200`.
- `PATCH` inválido: `{}` e corpo só com campos desconhecidos → `Envie ao menos um campo para alterar`; `null` e tipo errado →
  `required`; os casos de valor fora da regra do cadastro (username curto, com acento, email inválido);
  `role: "admin"` e `password` no corpo ignorados (a `role` continua `user` e a senha antiga continua entrando);
  `409` para username e para email de outra conta, sem alterar nada.
- `DELETE` feliz: `204` com corpo vazio; o token passa a dar `401` no `GET /users/me`; o login dá `401`; os registros
  financeiros do usuário somem do banco; os de **outro usuário** continuam; o email pode ser cadastrado de novo.
- `DELETE` inválido: senha errada → `403` `Senha incorreta` e a conta continua; sem `password`, `password: ""` e
  `password: 123` → `required`; sem corpo → `INVALID_BODY`.
- Nas duas rotas: `401` sem token e com token revogado por logout.
- `rate-limit.test.ts`: o 6º `DELETE` e o 11º `PATCH` no mesmo minuto dão `429`.
- Toda tarefa termina com `npm run typecheck`, `npm test` e `curl` no servidor real.

## Boundaries

- **Sempre**: o id do usuário só do token; as regras e mensagens de username/email iguais às do cadastro (um lugar só);
  `409` pelo `error.code` (`23505`), nunca pelo texto do erro; mensagens em português no formato `{ "error": ... }`;
  rate limit nas duas rotas.
- **Perguntar antes**: rotas `/users/:id` ou qualquer ação de admin; troca de senha; coluna nova (ex.: `updated_at`,
  `deleted_at`); alterar os testes existentes além de mover imports; commits; dependências.
- **Nunca**: aceitar `role`, `id`, `tokenVersion` ou `passwordHash` do corpo; logar o corpo da requisição (tem senha no `DELETE`);
  devolver o `passwordHash`; apagar a conta sem verificar a senha; desligar o rate limit fora dos testes.

## Success Criteria

- [x] Todos os critérios de aceite acima com teste automatizado passando.
- [x] `npm run typecheck` e `npm test` verdes, sem mudar o comportamento dos testes existentes.
- [x] `curl` no servidor real: cadastro → login → `PATCH` só com `username` (200) → `GET /users/me` com o novo →
      `PATCH` com o email de um segundo usuário (409) → `POST /transactions` → `DELETE` com senha errada (403) →
      `DELETE` com a senha certa (204) → `GET /users/me` com o mesmo token (401) → login (401).
- [x] `AGENTS.md` atualizado (etapa atual, estrutura, roteiro e decisões).

Nota: o ramo "`DELETE` com 0 linhas" (conta apagada entre o `SELECT` do hash e o `DELETE`) ficou coberto só por leitura de código, aceito pelo dono; o teste da corrida cobre o ramo do `SELECT` vazio.

## Resumo do contrato para o frontend

| Rota | Corpo | Sucesso | Erros |
|---|---|---|---|
| `PATCH /users/me` | só os campos que mudam: `username`, `email` | `200` com o usuário (formato do `GET /users/me`) | `400`, `401`, `409`, `429` |
| `DELETE /users/me` | `{ "password": "..." }` com `Content-Type: application/json` | `204` sem corpo | `400`, `401`, `403`, `429` |

| Status | Quando | `error` |
|---|---|---|
| `400` | `PATCH` sem `username` nem `email` | `Envie ao menos um campo para alterar` |
| `403` | `DELETE` com a senha errada | `Senha incorreta` |
| `409` | username ou email de outra conta | `Email ou username já cadastrado` |

Depois do `DELETE` com sucesso, o front descarta o token e volta para a tela de login. A exclusão apaga também todos os
registros financeiros: pedir confirmação na tela.

## Premissas (corrija se alguma estiver errada)

1. Rotas em `/users/me` (decidido pelo dono): cada usuário age só sobre a própria conta. Admin fica para depois.
2. `PATCH` altera só `username` e `email` (decidido pelo dono). Senha no futuro `PUT /users/me/password`; `role` nunca.
3. Trocar o email não pede senha (decidido pelo dono).
4. `DELETE` definitivo, com a senha atual no corpo (decidido pelo dono).
5. Senha errada no `DELETE` → `403` `Senha incorreta` (não `401`, para o front não confundir com sessão expirada).
6. Rate limit por IP: `DELETE` 5/min (argon2) e `PATCH` 10/min (o `409` revela contas existentes).
7. `PATCH` responde no formato do `GET /users/me`, sem `updatedAt` (o usuário não ganha coluna nova).
8. Campos extras no corpo são ignorados, inclusive `password` e `role` no `PATCH`, como nas outras rotas.
9. Conta excluída no meio de um `PATCH`/`DELETE` (corrida) → `401` padrão.

## Open Questions

Nenhuma, se as premissas 5 a 9 estiverem certas.

## Riscos (fora do escopo)

- **Token vazado permite trocar o email sem a senha** (premissa 3). Hoje isso não dá acesso à conta, porque não há
  "esqueci a senha". Quando houver recuperação de senha por email, trocar o email vira um jeito de **tomar a conta**:
  nessa etapa, voltar a exigir a senha na troca de email (ou confirmar pelo email antigo).
- A exclusão é irreversível e leva todos os registros. Mitigação: senha no `DELETE` e confirmação no front.
- Com uma `role` `admin` excluindo a própria conta, o sistema pode ficar sem admin. Hoje nenhuma rota depende de admin;
  entra na spec das ações de admin.
- Rate limit em memória e por IP: as mesmas limitações já registradas (proxy exige `trustProxy`; vários processos
  exigem Redis).
