# Implementation Plan: Migração do backend para Rust

Spec: [SPEC-migracao-rust.md](../SPEC-migracao-rust.md) (aprovada). Tarefas em [todo.md](todo.md).
Mapa: [CAPABILITY-MAP.md](../CAPABILITY-MAP.md), módulo `migracao-rust`. O plano anterior (troca de senha) foi
concluído (commit `3b6dc65`).

## Overview

Reescrever a API em `rust/` (axum + sqlx), no mesmo banco, com o mesmo contrato HTTP, exceto os casos de borda de
framework, que seguem o axum e vão para "Diferenças para o front" na spec. O objetivo é **estudo**: cada tarefa é
pequena, introduz poucos conceitos de Rust de cada vez e termina com explicação e perguntas de revisão para o dono.

Ciclo de cada tarefa: teste falhando (Rust e/ou paridade) → código → `cargo fmt --check` + `cargo clippy -- -D
warnings` + `cargo test` + paridade do trecho + `curl` na porta `3001` → explicação → commit (com o pedido do dono).

## Grafo de dependências

```
T0 toolchain + esqueleto (cargo new, crates aprovadas)
 └── T1 config + pool + GET /health
      └── T2 AppError + respostas de erro (404, 400 corpo, 413, 415, 500)
           └── T3 modo API_URL nos helpers do vitest (paridade) ── checkpoint A
                └── T4 POST /auth/register (validação, argon2, 409)
                     └── T5 JWT + POST /auth/login
                          └── T6 extractor CurrentUser + GET /users/me + POST /auth/logout ── checkpoint B
                               ├── T7 PATCH /users/me
                               ├── T8 DELETE /users/me e PUT /users/me/password
                               └── T9 POST e GET /transactions (totais, filtro)
                                    └── T10 PATCH e DELETE /transactions/:id ── checkpoint C
                                         └── T11 rate limit
                                              └── T12 paridade completa + documentação ── checkpoint final
```

T7, T8 e T9 só dependem de T6, mas ficam em sequência para haver uma tarefa aberta por vez (e para o estudo seguir
uma ordem).

## Architecture Decisions

- **Projeto em `rust/`**, um crate binário só (`meu-backend`), módulos na estrutura da spec. Sem workspace: um
  crate é mais fácil de ler.
- **Crates (T0, aprovação do dono antes do `cargo add`)**: `tokio`, `axum`, `tower`, `tower-http`, `sqlx`
  (`postgres`, `runtime-tokio`, `uuid`, `time`, `macros`), `serde`, `serde_json`, `argon2`, `jsonwebtoken`,
  `tower_governor`, `thiserror`, `tracing`, `tracing-subscriber`, `uuid`, `time`, `dotenvy`. Para testes: `reqwest`
  ou `tower::ServiceExt::oneshot` (preferir `oneshot`, que não precisa de crate a mais). Versões conferidas na doc
  da versão instalada.
- **Banco**: o mesmo Postgres e o mesmo schema. O `drizzle-kit` continua dono das migrations; o Rust não migra.
  `sqlx::query!` verifica as consultas contra o banco de dev no build; `cargo sqlx prepare` gera `.sqlx/` (versionado)
  para compilar sem banco. O `sqlx-cli` é instalado com `cargo install` (pedir ao dono na T1).
- **Testes em Rust**: integração com `tower::ServiceExt::oneshot` no `Router` (equivalente ao `app.inject()`), no
  banco `meu_backend_test`, que o `global-setup` do vitest já cria e migra. Os testes Rust limpam as tabelas como o
  `resetDatabase` e rodam com `--test-threads=1` (mesmo motivo do `fileParallelism: false`).
- **Paridade (T3)**: com `API_URL` definida, os helpers trocam `app.inject()` por `fetch` para o servidor Rust,
  devolvendo um objeto com a mesma forma (`statusCode`, `headers`, `json()`, `body`). Fica de fora da paridade HTTP,
  marcado como só-TS e coberto por teste em Rust:
  - testes com hooks injetados (`preHandler` para corrida, falha do banco no `requireAuth`) e asserção de log;
  - `rate-limit.test.ts`: os contadores ficam no processo e o IP não pode ser trocado por HTTP. O rate limit é testado
    em Rust, com um `Router` novo por teste.
  Os casos de borda de framework recebem a expectativa de cada servidor e uma linha em "Diferenças para o front".
- **Senhas compatíveis**: os hashes atuais são argon2id do `node-argon2` (64 MiB, `t=3`, `p=4`). A verificação no
  Rust lê os parâmetros do próprio hash; os hashes novos usam os **mesmos** parâmetros (o padrão do crate é menor),
  para as contas criadas pelo Rust terem o mesmo custo. Um teste prova que o Rust verifica um hash gerado pelo TS.
- **JWT compatível**: HS256, mesmo `JWT_SECRET`, claims `sub`, `ver`, `iat`, `exp` (obrigatórios `exp` e `sub`).
  Um teste prova que o token do TS vale no Rust e vice-versa.
- **Erros**: um `enum AppError` com `IntoResponse` (equivalente ao `sendError`). Os erros de extração do axum
  (`JsonRejection`, `PathRejection`) são convertidos para `AppError`, com mensagens em português. O 5xx loga o erro
  com `tracing` sem os parâmetros das consultas (o `sqlx::Error` não inclui os valores ligados, mas isso é conferido
  na T2).
- **Validação manual**: cada corpo é lido como `serde_json::Value` (ou struct com campos `Option<Value>`) e
  validado por funções por campo que devolvem `Result<T, AppError>`, na mesma ordem e com as mesmas mensagens do Zod.
  Isso deixa explícito o que o Zod fazia escondido, e é bom para estudar `match`, `Option` e `Result`.
- **Autenticação**: um extractor `CurrentUser` (`FromRequestParts`) roda antes do corpo (o `Json` é o último
  extractor do handler), reproduzindo o "401 antes do 400". Ele também leva o `token_version`.
- **Porta 3001** e o mesmo `.env` (lido com `dotenvy` a partir de `../.env`); `PORT` opcional.

## Riscos

| Risco | Mitigação |
|---|---|
| Ordem dos erros do axum diferente da do Fastify (ex.: `415` antes do `401`) | Teste de paridade de ordem na T6; se o axum não permitir a ordem do TS, vira linha em "Diferenças para o front" |
| `sqlx::query!` exige banco no build | `.sqlx/` versionado via `cargo sqlx prepare` (T1) |
| `tower_governor` com chave por IP depende de `ConnectInfo` | Configurar `into_make_service_with_connect_info` na T11; anotar o `trustProxy` equivalente (`SmartIpKeyExtractor`) como risco para o Cloudflare |
| Curva de aprendizado: erros de compilação de lifetimes/traits | Tarefas pequenas; explicação dos conceitos a cada entrega |
| Testes Rust e vitest disputando o mesmo banco `_test` | Rodar um de cada vez (documentado nos comandos) |

## Skills do catálogo

`deprecation-and-migration` e `api-and-interface-design` já foram instaladas. Nenhuma nova.

## Task List

Ver [todo.md](todo.md).
