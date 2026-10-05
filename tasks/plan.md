# Implementation Plan: Remover o TypeScript (só Rust)

Spec base: [SPEC-migracao-rust.md](../SPEC-migracao-rust.md) (checkpoint final: "decidir se o TS sai"). O plano
anterior (migração para Rust, T0–T12) foi concluído (commit `e040ada`) e está no histórico do git.
Tarefas em [todo.md](todo.md).

## Overview

O Rust já passa a suíte do vitest inteira (paridade). Agora o TypeScript sai do repositório, e com ele as duas
coisas que ainda eram dele:

- **Migrations**: hoje do `drizzle-kit`. Passam para o `sqlx migrate` (decisão do dono).
- **Testes**: hoje a suíte do vitest (154 testes por HTTP). É portada para `rust/tests/` com `Router::oneshot`
  (decisão do dono), sem perder nenhum caso.

Só depois disso o `src/`, `test/`, `package.json` e afins são apagados. A API não muda.

## Grafo de dependências

```
R1 migrations no sqlx (rust/migrations, banco de dev marcado, banco de teste pelo sqlx)
 └── R2 helpers de teste compartilhados (tests/common)
      ├── R3 porta: errors, health, register, login, logout, users-me, require-auth
      ├── R4 porta: users-update-delete, users-password
      ├── R5 porta: transactions, transactions-update-delete
      └── R6 porta: rate-limit (confere a cobertura que já existe em rust/tests/rate_limit.rs)
           └── R7 remove o TypeScript + documentação ── checkpoint final
```

R3–R6 só dependem de R2, mas ficam em sequência (uma tarefa aberta por vez). Até a R7 o vitest continua no repo:
cada tarefa de porte pode conferir que o caso existia lá.

## Architecture Decisions

- **Migrations em `rust/migrations/`**, com os mesmos 5 SQLs de `drizzle/` (`0000_create_users.sql` ...). O sqlx
  aceita esse nome (`<versão>_<descrição>.sql`, versão 0 a 4). As linhas `--> statement-breakpoint` do drizzle são
  comentários SQL e podem ficar ou sair (saem, para limpar).
- **Ferramenta**: `sqlx-cli` (`cargo install sqlx-cli --no-default-features --features rustls,postgres`, pedir ao
  dono). Comandos: `sqlx migrate add <nome>` cria, `sqlx migrate run` aplica. Feature `migrate` no `sqlx` do crate,
  para os testes aplicarem as migrations com `sqlx::migrate!()`.
- **Banco de dev já migrado pelo drizzle**: o `sqlx migrate run` tentaria rodar tudo de novo (e falharia: as tabelas
  existem). Um script de uma vez só (não versionado) cria a `_sqlx_migrations` e marca as 5 como aplicadas, com o
  checksum SHA-384 de cada arquivo, igual ao que o sqlx calcularia. A tabela `drizzle.__drizzle_migrations` fica
  (é inofensiva) ou é apagada, a critério do dono. O banco de testes não precisa disso: é recriado.
- **Banco de testes**: `tests/common` cria o `meu_backend_test` se não existir e aplica as migrations com
  `sqlx::migrate!()` (o que o `global-setup.ts` fazia). Mesma trava de segurança: só roda em banco terminado em
  `_test`. `.env.test` continua (lido com `dotenvy`).
- **Helpers de teste em `rust/tests/common/mod.rs`** (o equivalente ao `test/helpers.ts`): `TestApp` com
  `request(method, url).json(...).bearer(...)` → resposta com `status`, `headers`, `json()`; `reset_database()`;
  `register_user()`/`login()`. O rate limit fica desligado por padrão (`without_rate_limit()`), como no TS.
  Os testes rodam em série (`--test-threads=1`, já documentado), porque compartilham o banco.
- **Um arquivo de teste Rust por arquivo do vitest**, com os mesmos nomes de caso (em `snake_case` português),
  para a revisão poder comparar lado a lado. Os testes só-TS (internos do Fastify) já têm equivalente em Rust e não
  são portados de novo. As expectativas de borda ficam as do axum ("Diferenças para o front").
- **Remoção (R7)**: `src/`, `test/`, `package.json`, `package-lock.json`, `tsconfig.json`, `vitest.config.ts`,
  `drizzle.config.ts`, `drizzle/`, `node_modules/` (local). Fica: `docker-compose.yml`, `.env*`, specs, `.claude/`.
  `AGENTS.md` reescrito para o Rust (stack, comandos, estrutura, convenções e regras de segurança sem referências ao
  Fastify/Drizzle; as decisões continuam, com a menção do TS como histórico). Talvez promover `rust/` à raiz fica
  **fora** deste plano (mudança grande de caminhos; decidir depois).

## Riscos

| Risco | Mitigação |
|---|---|
| Marcar o banco de dev errado (checksum diferente) faz o `sqlx migrate run` reclamar | Conferir com `sqlx migrate info` depois de marcar; backup com `pg_dump` antes |
| Perder um caso de teste no porte | Contagem por arquivo no fim de cada tarefa (casos do vitest × casos Rust, menos os só-TS) |
| Testes Rust mais lentos (argon2 em cada cadastro) | `opt-level = 3` do argon2 já está no perfil de dev; cadastrar uma vez por teste, como no TS |
| `cargo test` rodando em paralelo no mesmo banco | `--test-threads=1` documentado; os helpers usam um `Mutex` global por garantia |
