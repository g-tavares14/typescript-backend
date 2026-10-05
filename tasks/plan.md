# Implementation Plan: Endpoints restantes (troca de senha)

Spec: [SPEC-endpoints-restantes.md](../SPEC-endpoints-restantes.md) (aprovada). Tarefas em [todo.md](todo.md).
Mapa: [CAPABILITY-MAP.md](../CAPABILITY-MAP.md), módulo `endpoints-restantes`. O plano anterior (edição e exclusão da
conta) foi concluído e está no histórico do git (commit `136ba6d`). O plano da migração para Rust vem depois deste.

## Overview

Criar `PUT /users/me/password`: confere a senha atual, grava o hash da nova, incrementa `token_version` na mesma
consulta e devolve um token novo. Sem migration e sem dependência nova. TDD em cada tarefa: teste falhando → código →
`npm run typecheck` + `npm test` + `curl` → commit (com o pedido do dono).

## Grafo de dependências

```
T1 passwordSchema compartilhado (refatoração do cadastro, sem mudar comportamento)
 └── T2 PUT /users/me/password: caminho feliz, token novo, 403, 401 e corrida
      └── T3 validação (todos os 400) + rate limit 5/min
           └── T4 documentação (AGENTS.md, spec)
```

Tudo sequencial: T2 e T3 mexem em `src/routes/users.ts`.

## Architecture Decisions

- **Refatoração primeiro (T1)**: `passwordSchema` (`z.string(required).min(8, "A senha deve ter no mínimo 8
  caracteres")`) vai para `src/lib/user-fields.ts`; o `registerSchema` passa a usá-lo. `register.test.ts` tem de ficar
  verde **sem alteração**.
- **Schema da rota**: `z.object({ currentPassword: z.string(required).min(1, required.error), newPassword:
  passwordSchema }, INVALID_BODY)`. A ordem das chaves no `z.object` define a ordem das mensagens (o Zod valida na
  ordem declarada e o `badRequest` responde a primeira): `currentPassword` antes de `newPassword`, como a spec pede.
- **Fluxo**: `SELECT password_hash WHERE id = <token>` → `verifyPassword()` (`403` se errada) → `hashPassword(nova)` →
  `UPDATE users SET password_hash = $hash, token_version = token_version + 1 WHERE id = $id AND token_version = $ver
  RETURNING id, token_version` → `createAccessToken(...)`. O incremento é do banco (`sql\`${users.tokenVersion} + 1\``,
  como no logout).
- **`token_version` do token na condição**: o `currentUser()` hoje não expõe a versão (o `publicUserColumns` não tem
  `tokenVersion`, de propósito). Opções:
  1. o `requireAuth` guarda `request.tokenVersion` (ou um objeto `auth` separado do `user`), sem colocar a versão em
     `publicUserColumns`;
  2. reler e conferir no `SELECT` do hash (`WHERE id AND token_version`), e o `UPDATE` repete a condição.
  Proposta: **opção 1**, um campo novo no request preenchido pelo `requireAuth` (a versão já foi conferida ali), sem
  mudar o formato do usuário exposto. O `UPDATE` usa esse valor.
- **`403` `Senha incorreta`**: a mesma constante do `DELETE /users/me` (extrair para uma constante no `users.ts`).
- **Resposta** no formato do login: `{ token, tokenType: "Bearer", expiresIn }`. O `ACCESS_TOKEN_TTL_SECONDS` já é
  exportado de `token.ts`.
- **Rate limit por rota** (`config.rateLimit`, 5/min), como o `DELETE /users/me`: roda depois do `requireAuth`.
- **Log**: o hash entra como parâmetro do `UPDATE`; o `sendError` loga só a `query` dos erros do Drizzle, sem os
  parâmetros (`src/lib/errors.ts`), então o hash não vai para o log num `500`.

## Riscos

- **Teste da corrida**: simular logout entre o `requireAuth` e o `UPDATE`, com um hook `preHandler` no app de teste
  que incrementa `token_version` (mesma técnica do teste de conta apagada no `PATCH`). Esperado: `401` e a senha antiga
  continua valendo.
- **Testes lentos**: cada caso chama argon2 até três vezes (login, verificação, hash novo). Aceitável no volume atual.

## Skills do catálogo

Nenhuma nova: `security-and-hardening` (núcleo) cobre a rota, e `api-and-interface-design` já foi instalada.

## Task List

Ver [todo.md](todo.md).
