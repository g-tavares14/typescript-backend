# Spec: Migração do backend para Rust

Status: **aprovada pelo dono** (2026-10-05). Módulo `migracao-rust` do
[mapa de capacidades](CAPABILITY-MAP.md). Começa só depois de [SPEC-endpoints-restantes.md](SPEC-endpoints-restantes.md).

## Objetivo

Reescrever o backend em **Rust**, com **o mesmo contrato HTTP** da versão TypeScript: rotas, status, corpos,
mensagens em português, headers (`WWW-Authenticate`, `Retry-After`), rate limit e regras de segurança.

**Exceção (decisão do dono):** casos de borda que dependem do framework **seguem o funcionamento natural do axum**,
sem imitar o Fastify. Exemplo: o Fastify tem parser de `text/plain` e devolve `400` `INVALID_BODY`; o axum recusa
o tipo de conteúdo, então no Rust vira `415`. Cada diferença continua com resposta `{ "error": "..." }` em português,
fica comentada no código e listada na seção "Diferenças para o front" (abaixo, preenchida durante a migração) para o
front saber o que muda. As regras de negócio e de segurança (validação dos campos, `401`/`403`/`404`/`409`, rate
limit, mensagens dos campos) **não** entram nessa exceção.

**Por quê** (decisão nova do dono, registrada no AGENTS.md ao lado de "TypeScript em vez de Rust"): **estudar Rust**.
O agente escreve o código e o dono lê para aprender. Por isso a legibilidade e a explicação valem tanto quanto o
código funcionar:

- passos pequenos, um módulo por entrega;
- cada entrega explica os conceitos de Rust que apareceram (ownership e borrowing, `Result`/`?`, `Option`, traits,
  `async`/`.await`, `impl Trait`, extractors e `State` do axum, macros do `sqlx`/`serde`) e deixa perguntas de revisão;
- código idiomático e simples antes de esperto: nada de genéricos ou macros próprias sem necessidade.

O código parte **do zero** (a tag `versao-rust` é anterior ao endurecimento e não é base). O TypeScript continua no
repositório até o Rust passar na suíte de paridade; remover o TS é uma decisão separada, depois.

## Tech Stack

Versões exatas fixadas no `Cargo.toml` na primeira tarefa, conferindo a documentação da versão instalada.

| Parte | Escolha | Equivalente no TS |
|---|---|---|
| Linguagem | Rust stable (edição 2024) | TypeScript |
| Runtime assíncrono | `tokio` | Node.js |
| Framework web | `axum` (+ `tower`/`tower-http`) | Fastify |
| Banco | `sqlx` (Postgres, `query!`/`query_as!` verificadas em compilação) | Drizzle |
| Serialização | `serde` / `serde_json` | JSON nativo |
| Validação | manual, em funções por campo (sem crate de validação) | `zod` |
| Hash de senha | `argon2` (argon2id, mesmos parâmetros do hash atual) | `argon2` |
| JWT | `jsonwebtoken` (HS256, `sub`, `ver`, `iat`, `exp` obrigatório) | `jose` |
| Rate limit | `tower_governor` (por IP, em memória) | `@fastify/rate-limit` |
| Erros | `thiserror` + `IntoResponse` num tipo `AppError` | `sendError()` |
| Logs | `tracing` + `tracing-subscriber` | `pino` |
| Datas/ids | `time` (ou `chrono`) e `uuid` | `Date`, `crypto` |
| Config | variáveis de ambiente + `dotenvy` | `--env-file-if-exists` |

Todas são dependências novas no projeto Rust; a lista final vai para aprovação na primeira tarefa.

**Banco e migrations:** o Rust usa **o mesmo banco e o mesmo schema**. Durante a migração o `drizzle-kit` continua
dono das migrations (`drizzle/`); o Rust não cria nem altera tabelas. Hashes argon2 e tokens são compatíveis entre as
versões (mesmo `JWT_SECRET`): uma conta criada pelo TS faz login no Rust e vice-versa.

## Commands

```bash
cd rust && cargo run                      # servidor (lê ../.env)
cd rust && cargo watch -x run             # reload automático (cargo-watch, opcional)
cd rust && cargo build
cd rust && cargo clippy -- -D warnings    # lint
cd rust && cargo fmt --check
cd rust && cargo test                     # testes em Rust (banco _test)
API_URL=http://127.0.0.1:3001 npm run test:parity   # suíte de paridade contra o servidor Rust
```

`sqlx` com `query!` precisa do banco no build: usar `DATABASE_URL` do `.env` ou `cargo sqlx prepare` (modo offline,
`.sqlx/` versionado).

## Project Structure

```
rust/
├── Cargo.toml
├── .sqlx/                 # metadados das queries para build offline
├── src/
│   ├── main.rs            # lê config, cria o pool, sobe o servidor (server.ts)
│   ├── app.rs             # monta o Router: rotas, fallback 404, limites de corpo, rate limit (app.ts)
│   ├── config.rs          # env com fail fast (config.ts)
│   ├── error.rs           # AppError -> resposta { "error": ... } (errors.ts)
│   ├── auth.rs            # extractor CurrentUser: token -> usuário, ou 401 (authenticate.ts)
│   ├── password.rs, token.rs, validation.rs, user_fields.rs
│   └── routes/{health,auth,users,transactions}.rs
└── tests/                 # testes de integração em Rust
```

## Code Style

```rust
// Erro da aplicação: cada variante vira um status e uma mensagem em português.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    BadRequest(String),
    #[error("Não autenticado")]
    Unauthorized,
    #[error("Erro interno do servidor")]
    Internal(#[from] anyhow::Error), // detalhe só no log, nunca na resposta
}

async fn me(user: CurrentUser) -> Json<PublicUser> {
    Json(user.0) // o extractor já devolveu 401 se o token não valesse
}
```

- `snake_case` em Rust, `camelCase` no JSON (`#[serde(rename_all = "camelCase")]`).
- Comentários em português explicando o **porquê**, e o conceito de Rust quando ele aparece pela primeira vez.
- Sem `unwrap()`/`expect()` em caminho de requisição; só na inicialização (fail fast).
- `cargo fmt` e `cargo clippy` sem avisos.

## Testing Strategy

1. **Paridade (critério principal):** a suíte do vitest roda contra o servidor Rust por HTTP. Os helpers ganham um
   modo `API_URL`: com ela, `app.inject()` vira `fetch` para a URL; sem ela, tudo continua como hoje. Os testes que
   dependem de internos do Fastify (falha simulada do banco no hook, asserção de log, `currentUser` sem hook) ficam
   marcados como só-TS e ganham equivalentes em Rust. Os casos de borda de framework que mudam no axum ganham a
   expectativa de cada servidor no próprio teste (escolhida pela `API_URL`), sempre ligada a uma linha de
   "Diferenças para o front".
2. **Testes em Rust** (`cargo test`): unitários para validação, token e senha; integração com o banco `_test` para as
   rotas, para o dono ver como se testa em Rust.
3. Rate limit: o servidor Rust sobe com os limites reais para o `rate-limit.test.ts` e com o limite desligado (variável
   só de teste) para o resto.

## Boundaries

- **Sempre**: `cargo fmt`, `cargo clippy -D warnings`, `cargo test` e a paridade do módulo antes de entregar; explicar
  os conceitos de Rust da entrega; consultar a doc da versão instalada de cada crate.
- **Perguntar antes**: qualquer crate fora da tabela; mudar regra de negócio ou de segurança do contrato (diferença de
  framework segue o axum e vai para "Diferenças para o front"; qualquer outra vira pergunta, não "melhoria"); alterar o
  schema; remover o TS.
- **Nunca**: logar senha, hash ou dados pessoais; montar SQL concatenando strings; `unsafe`; desligar o rate limit
  fora dos testes; editar migrations aplicadas.

## Ordem proposta (vira o plano)

1. Esqueleto: `Cargo.toml`, config com fail fast, pool, `GET /health` (200/503).
2. `AppError` e respostas de erro: 404, 400 de corpo inválido, 413 (1 MiB), 415, 5xx genérico + log.
3. `POST /auth/register` (validação, normalização, argon2, `409`).
4. `POST /auth/login` e JWT; extractor `CurrentUser` com `token_version`; `GET /users/me`; `POST /auth/logout`.
5. `PATCH`, `DELETE /users/me` e `PUT /users/me/password`.
6. `POST`, `GET /transactions` (totais e filtro).
7. `PATCH`, `DELETE /transactions/:id`.
8. Rate limit em todas as rotas que têm limite.
9. Paridade completa e documentação (AGENTS.md, decisão nova, como rodar).

## Success Criteria

- [ ] A suíte de paridade passa inteira contra o servidor Rust (exceto os testes marcados como só-TS, cada um com
      equivalente em Rust).
- [ ] Dados criados por uma versão funcionam na outra (login, token, registros).
- [ ] `cargo clippy -D warnings`, `cargo fmt --check` e `cargo test` limpos.
- [ ] Todas as regras de segurança do AGENTS.md valem no Rust.
- [ ] Cada entrega trouxe explicação dos conceitos e perguntas de revisão.

## Diferenças para o front

Preenchida durante a migração: cada caso de borda em que o Rust responde diferente do TS (status e mensagem).

| Caso | TypeScript (Fastify) | Rust (axum) |
|---|---|---|
| Corpo com `Content-Type: text/plain` ou sem `Content-Type` | `400` `Corpo da requisição inválido: envie um objeto JSON` | `415` `Tipo de conteúdo não suportado (use application/json)` |
| Método errado numa rota que existe (ex.: `POST /health`) | `404` `Rota não encontrada` | `405` `Método não permitido` |

O front já manda `application/json` em toda requisição com corpo e só usa os métodos documentados, então nenhum dos
dois casos aparece no uso normal.

## Decisões das perguntas abertas

1. Porta do Rust em dev: `3001`, ao lado do TS (`3000`).
2. Diferenças de framework: seguem o axum (ver Objetivo).
3. Remover o TS depois da paridade: decidir no fim.
