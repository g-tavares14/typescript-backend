# Spec: Endpoints restantes (troca de senha)

Status: **concluída** (aprovada pelo dono em 2026-10-05). Módulo `endpoints-restantes` do
[mapa de capacidades](CAPABILITY-MAP.md). Continuação de [SPEC-users-update-delete.md](SPEC-users-update-delete.md).

## Objetivo

Fechar o contrato da API **antes** da migração para Rust ([SPEC-migracao-rust.md](SPEC-migracao-rust.md)), para que o
Rust copie um alvo estável. A análise das rotas existentes mostrou uma lacuna essencial: **não há como trocar a
senha**. Esta etapa adiciona `PUT /users/me/password`, já prevista no AGENTS.md e na spec anterior.

### Análise de lacunas (o que ficou de fora e por quê)

| Candidato | Decisão |
|---|---|
| `PUT /users/me/password` | **Entra nesta etapa** |
| `GET /transactions/:id` | Depois: o front lê os registros pela lista |
| Paginação do `GET /transactions` | Depois: só quando o volume pedir |
| Recuperação de senha por email | Depois: exige serviço de email (dependência nova) e reabre a decisão "trocar email sem senha" |
| Categorias de registros | Depois: funcionalidade de produto, spec própria |
| `/users/:id` (admin) | Depois: spec própria, quando houver regras por `role` |
| Limite de registros por usuário | Depois: endurecimento, não é endpoint |

Sem migration e sem dependência nova.

### Critérios de aceite (comportamento da API)

**Autenticação**
- Exige `Authorization: Bearer <token>` pelo `requireAuth` do plugin `users.ts`. Sem token, token inválido ou revogado:
  `401` padrão, antes de qualquer `400`, `403`, `413`, `415` ou `429` (o limite é da rota e roda depois do hook).
- A senha alterada é **sempre** a do usuário do token.

**`PUT /users/me/password`**

```json
// PUT /users/me/password   (Content-Type: application/json)
{ "currentPassword": "senha123", "newPassword": "outraSenha456" }
```

- `currentPassword`: string não vazia (sem tamanho mínimo, como no login). Ausente, vazia, `null` ou tipo errado →
  `400` `Campo obrigatório ausente ou inválido`.
- `newPassword`: a **mesma regra e mensagem do cadastro** (mínimo 8 caracteres: `A senha deve ter no mínimo 8
  caracteres`; ausente/tipo errado: `Campo obrigatório ausente ou inválido`). A regra passa a ser compartilhada
  (`passwordSchema` em `src/lib/user-fields.ts`), sem mudar o comportamento do cadastro.
- Ordem das validações: corpo do Fastify (`INVALID_BODY`, `413`, `415`) → `currentPassword` → `newPassword` → senha atual.
  Campos a mais são ignorados.
- Senha atual errada → `403` `Senha incorreta` (mesmo motivo do `DELETE /users/me`: `401` faria o front deslogar), e
  nada muda.
- Sucesso: grava o hash argon2id da nova senha **e incrementa `token_version` na mesma consulta**
  (`UPDATE ... SET password_hash = $1, token_version = token_version + 1 WHERE id = $2 AND token_version = $3
  RETURNING token_version`). Todos os tokens emitidos antes deixam de valer, em todos os dispositivos.
- Resposta: `200` com um **token novo**, no mesmo formato do login, para o dispositivo atual continuar logado:

```json
{ "token": "eyJ...", "tokenType": "Bearer", "expiresIn": 3600 }
```

- Depois da troca: o login com a senha antiga dá `401` `Email ou senha inválidos`, com a nova dá `200`; o token antigo
  dá `401` padrão; o token devolvido funciona.
- Corrida (logout, outra troca de senha ou exclusão da conta entre o `requireAuth` e o `UPDATE`): `0` linhas → `401`
  padrão, e nada muda. O `AND token_version = $3` garante que um token já revogado não troque a senha.
- **Rate limit**: 5/min por IP (cada tentativa custa um hash argon2, e com token roubado é a rota que permite
  adivinhar a senha atual), depois do `requireAuth`.
- Nunca logar `currentPassword`, `newPassword` nem o hash.

## Tech Stack, comandos, estrutura e estilo

Os mesmos do [AGENTS.md](AGENTS.md). Arquivos tocados: `src/lib/user-fields.ts` (`passwordSchema`),
`src/routes/auth.ts` (usar o `passwordSchema`), `src/routes/users.ts` (rota nova), testes novos em
`test/users-password.test.ts` e o caso de limite em `test/rate-limit.test.ts`.

## Testing Strategy

Vitest com `app.inject()` e o banco `_test`, como as etapas anteriores. Cobrir: caminho feliz (token novo funciona,
antigo não; login com senha nova e não com a antiga), `403` sem alterar nada, cada `400` e a ordem dos erros, `401`
antes do `400`, corrida (`token_version` mudou no meio) e o `429` com o limite real.

## Boundaries

- **Sempre**: `npm run typecheck` e `npm test` antes de entregar; `curl` real no servidor de dev.
- **Perguntar antes**: commits; qualquer dependência; mudar a mensagem ou o formato de rota já existente.
- **Nunca**: logar senha ou hash; ler o `token_version` e somar em JS (o incremento é do banco).

## Success Criteria

- [x] `PUT /users/me/password` responde conforme todos os critérios acima, com testes.
- [x] O cadastro continua com o mesmo comportamento (testes atuais passam sem mudança).
- [x] AGENTS.md atualizado (estrutura, decisões, roteiro) e contrato para o front nesta spec.

## Decisões das perguntas abertas

1. Sucesso devolve `200` com **token novo** (o dispositivo atual continua logado).
2. `newPassword` igual à atual é **aceita** (recusar custaria outro hash argon2 sem proteger nada relevante).
