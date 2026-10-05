# Tarefas: Remover o TypeScript (só Rust)

Plano: [plan.md](plan.md). Cada tarefa: código → `cargo fmt --check` + `cargo clippy --all-targets -- -D warnings`
+ `cargo test` → explicação dos conceitos + perguntas de revisão → commit (com o pedido do dono).

## R1: Migrations no sqlx ✅
- Aceite:
  - O dono aprovou instalar o `sqlx-cli` e a feature `migrate` do `sqlx`.
  - `rust/migrations/0000..0004_*.sql` com o SQL dos arquivos de `drizzle/`.
  - Banco de dev marcado (backup antes); `sqlx migrate info` mostra as 5 como aplicadas.
- Verificar: `sqlx migrate info`; `cargo run` + `curl localhost:3001/health`.
- Conceitos: `sqlx migrate`, checksum de migration, `sqlx::migrate!()` (SQL embutido no binário).
- Tamanho: S

## R2: Helpers de teste compartilhados ✅
- Aceite: `rust/tests/common/mod.rs` cria/migra o banco `_test` (trava `_test`), `TestApp` com requisições
  encadeadas, `reset_database`, `register_user`, `login`. Os testes Rust atuais passam a usar os helpers.
- Conceitos: módulo `common` em `tests/`, builder pattern, `OnceCell`/`Mutex` do tokio.
- Tamanho: M · Depende de: R1

## R3: Porte — erros, health, cadastro, login, logout, `/users/me`, require-auth ✅
- Aceite: todos os casos não só-TS de `errors`, `register`, `login`, `logout`, `users-me`, `require-auth`.
- Tamanho: M · Depende de: R2

## R4: Porte — `PATCH`/`DELETE /users/me` e troca de senha ✅
- Aceite: todos os casos de `users-update-delete` e `users-password` (as corridas já cobertas em `tests/users.rs`).
- Tamanho: M · Depende de: R3

## R5: Porte — transações ✅
- Aceite: todos os casos de `transactions` e `transactions-update-delete`.
- Tamanho: M · Depende de: R4

## R6: Porte — rate limit ✅
- Aceite: cada caso de `rate-limit.test.ts` tem equivalente em `tests/rate_limit.rs` (completar o que faltar).
- Tamanho: S · Depende de: R5

## R7: Remover o TypeScript e documentar
- Aceite: arquivos do TS apagados (lista no plano); `AGENTS.md`, `CAPABILITY-MAP.md`, `.gitignore` e
  `.env.example` só com o Rust; `cargo test` verde.
- Tamanho: S · Depende de: R6

## Checkpoint final
- [ ] Nenhum caso do vitest sem equivalente; `cargo clippy`, `cargo fmt --check`, `cargo test` limpos
- [ ] Revisão do dono; decidir se `rust/` vira a raiz do repositório
