# Tarefas: Migração do backend para Rust

Plano: [plan.md](plan.md). Spec: [SPEC-migracao-rust.md](../SPEC-migracao-rust.md).
Cada tarefa: teste falhando → código → `cargo fmt --check` + `cargo clippy -- -D warnings` + `cargo test` + paridade do
trecho + `curl` na porta 3001 → explicação dos conceitos + perguntas de revisão → commit (com o pedido do dono).

## Task 0: Toolchain e esqueleto ✅
- Aceite:
  - O dono instalou o Rust (`rustup`, stable) e aprovou a lista de crates do plano.
  - `rust/` criado com `cargo new`; `Cargo.toml` com as crates aprovadas e versões fixadas; `cargo build` limpo.
  - `.gitignore` com `rust/target/`.
- Verificar: `cargo --version`; `cd rust && cargo build`.
- Conceitos: Cargo, crates, `main.rs`, `#[tokio::main]`.
- Arquivos: rust/Cargo.toml, rust/src/main.rs, .gitignore
- Tamanho: XS · Depende de: nada

## Task 1: Config, pool e `GET /health` ✅
- Aceite:
  - `config.rs`: `DATABASE_URL` e `JWT_SECRET` do ambiente (`../.env`), com fail fast e mensagem clara; `PORT` padrão 3001.
  - Pool `sqlx::PgPool`; `GET /health` → `200` com o banco de pé, `503` sem banco (corpos iguais aos do TS).
  - `sqlx-cli` e `.sqlx/` adiados para a T4: o `/health` usa `sqlx::query` (sem verificação em compilação), e a
    primeira `query!` só aparece no cadastro.
- Verificar: teste Rust do `/health`; `curl localhost:3001/health`.
- Conceitos: `struct`, `Result`/`?`, `std::env`, `State` do axum, `async`/`.await`.
- Arquivos: rust/src/{main,config,app}.rs, rust/src/routes/{mod,health}.rs
- Tamanho: S · Depende de: T0

## Task 2: `AppError` e respostas de erro ✅
- Aceite:
  - `enum AppError` + `IntoResponse` → `{ "error": "..." }`; 5xx genérico com log completo via `tracing`.
  - Fallback `404` `Rota não encontrada`; corpo inválido → `INVALID_BODY`; `413` acima de 1 MiB; `415` com a mensagem
    em português; diferenças de borda anotadas na spec.
- Verificar: testes Rust de cada resposta; `curl` de rota inexistente e corpo malformado.
- Conceitos: `enum` com dados, traits (`IntoResponse`, `From`), `thiserror`, `impl Trait`.
- Arquivos: rust/src/{error,app}.rs, SPEC-migracao-rust.md
- Tamanho: S · Depende de: T1

## Task 3: Modo `API_URL` nos helpers do vitest ✅
- Aceite:
  - Com `API_URL`, `createTestApp()` devolve um objeto com `inject()` via `fetch` (mesma forma de resposta);
    sem `API_URL`, nada muda (`npm test` verde).
  - Testes só-TS marcados (pulados com `API_URL`), cada um com a nota do teste Rust equivalente.
  - `npm run test:parity` no `package.json` (compila o Rust e o `global-setup` sobe o servidor na porta 3101, no banco
    `_test`). Dos testes do `errors.test.ts`, passam os que não dependem de rotas (404 e URL malformada); os de corpo
    passam a partir da T4, quando existir `/auth/register`.
- Verificar: `npm test`; `API_URL=http://127.0.0.1:3001 npm run test:parity -- test/errors.test.ts`.
- Arquivos: test/helpers.ts, package.json, testes marcados
- Tamanho: M · Depende de: T2

## Checkpoint A (T0–T3)
- [ ] Rust compila limpo, `/health` e erros com paridade
- [ ] Revisão do dono

## Task 4: `POST /auth/register` ✅
- Aceite: validação e normalização de `username`, `email` e `password` com as mensagens do TS e na mesma ordem;
  hash argon2id com os parâmetros do TS; `201`; `409` `Email ou username já cadastrado` (código `23505`).
  `register.test.ts` passa na paridade.
- Conceitos: `serde::Deserialize`, `Option`, `match`, funções que devolvem `Result`, `sqlx::query!`.
- `sqlx-cli` e `.sqlx/` (build sem banco) adiados para a T12: o `query!` funciona com o `DATABASE_URL` do `.env`.
- Arquivos: rust/src/{validation,user_fields,password}.rs, rust/src/routes/auth.rs
- Tamanho: M · Depende de: T3

## Task 5: JWT e `POST /auth/login` ✅
- Aceite: login com a mesma mensagem para email inexistente e senha errada (com verificação simulada); token HS256
  compatível com o TS nos dois sentidos (teste); hash gerado pelo TS verifica no Rust. `login.test.ts` na paridade.
- Conceitos: `serde::Serialize`, `jsonwebtoken`, `OnceLock` (hash fictício), `spawn_blocking` (argon2 fora do runtime).
- Arquivos: rust/src/token.rs, rust/src/password.rs, rust/src/routes/auth.rs
- Tamanho: M · Depende de: T4

## Task 6: `CurrentUser`, `GET /users/me` e `POST /auth/logout` ✅
- Aceite: extractor `FromRequestParts` com `401` padrão (`WWW-Authenticate: Bearer`) antes de qualquer erro de corpo;
  `token_version` conferido no banco; logout incrementa a versão no banco. `users-me`, `logout` e as partes HTTP de
  `require-auth` na paridade.
- Conceitos: extractors, `FromRequestParts`, ordem dos extractors.
- Arquivos: rust/src/auth.rs, rust/src/routes/{users,auth}.rs
- Tamanho: M · Depende de: T5

## Checkpoint B (T4–T6)
- [ ] Cadastro, login, `/users/me` e logout com paridade; conta do TS funciona no Rust e vice-versa
- [ ] Revisão do dono

## Task 7: `PATCH /users/me` ✅
- Aceite: `username`/`email` parciais, `null`/tipo errado, `Envie ao menos um campo para alterar`, `409`, `UPDATE ...
  RETURNING`, `0` linhas → `401`. Parte do `PATCH` em `users-update-delete.test.ts` na paridade; corrida em teste Rust.
- Conceitos: `Option<Option<T>>` (ausente × `null`), SQL dinâmico seguro (`COALESCE` ou `QueryBuilder` com binds).
- Arquivos: rust/src/routes/users.rs
- Tamanho: S · Depende de: T6

## Task 8: `DELETE /users/me` e `PUT /users/me/password` ✅
- Aceite: `403` `Senha incorreta`; `204` + `CASCADE`; troca de senha com `token_version` + 1 e `WHERE token_version`
  numa consulta, token novo. Partes do `DELETE` e `users-password.test.ts` na paridade.
- Conceitos: reaproveitar funções entre handlers, `RETURNING` com `query_as!`.
- Arquivos: rust/src/routes/users.rs
- Tamanho: M · Depende de: T7

## Task 9: `POST` e `GET /transactions` ✅
- Aceite: validação de `type`, `amount` (inteiro, `> 0`, teto), `description`, `date`; lista ordenada + `summary`
  calculado no banco; filtro `from`/`to` com os erros do TS (inclusive query repetida). `transactions.test.ts` na paridade.
- Conceitos: `i64` e o limite de 2^53, `time::Date`, `Query` do axum, `serde(rename_all)`.
- Arquivos: rust/src/routes/transactions.rs
- Tamanho: M · Depende de: T8

## Task 10: `PATCH` e `DELETE /transactions/:id`
- Aceite: `:id` não UUID → `404` sem ir ao banco; `404` igual para outro usuário/inexistente; `PATCH` parcial com
  `updated_at = now()`; ordem dos erros. `transactions-update-delete.test.ts` na paridade.
- Conceitos: `Path` e rejeição customizada, `uuid::Uuid::parse_str`.
- Arquivos: rust/src/routes/transactions.rs
- Tamanho: M · Depende de: T9

## Checkpoint C (T7–T10)
- [ ] Todas as rotas com paridade
- [ ] Revisão do dono

## Task 11: Rate limit
- Aceite: limites iguais aos do TS (login 5, cadastro 3, `DELETE`/`PUT password` 5, `PATCH /users/me` 10 por minuto
  por IP), `429` com `Retry-After` e mensagem em português; nas rotas de `/users/me` o limite roda depois da
  autenticação; desligável só por configuração de teste. Testes Rust equivalentes ao `rate-limit.test.ts`.
- Conceitos: `tower::Layer`, middleware, `ConnectInfo`.
- Arquivos: rust/src/app.rs, rust/src/routes/*.rs, rust/tests/
- Tamanho: M · Depende de: T10

## Task 12: Paridade completa e documentação
- Aceite: `npm run test:parity` verde inteiro (exceto só-TS); "Diferenças para o front" completa; AGENTS.md com a
  stack Rust, comandos, estrutura e decisões; spec com critérios marcados.
- Arquivos: AGENTS.md, SPEC-migracao-rust.md
- Tamanho: S · Depende de: T11

## Checkpoint final
- [ ] Paridade, `cargo clippy`, `cargo fmt --check`, `cargo test` limpos
- [ ] Revisão do dono; decidir se o TS sai do repositório
