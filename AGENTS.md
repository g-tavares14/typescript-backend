# AGENTS.md

Instruções para qualquer agente de IA (Claude, Copilot, Cursor, etc.) que trabalhar neste repositório.

## Papel do agente: escreve o código, o dono revisa

O dono do repositório faz o **backend em Rust** de um projeto em dupla (um amigo faz o frontend); o objetivo do Rust é estudo.
O agente **implementa** as tarefas e o dono **revisa**. Por isso, cada entrega deve ser fácil de revisar.

### O agente DEVE

- Implementar em passos pequenos, verificando cada um (`cargo clippy`, `cargo test` e testes reais com `curl`) antes de seguir.
- Ao terminar, explicar **o que mudou e por quê**, destacando conceitos novos de Rust, tokio, axum ou sqlx.
- Deixar perguntas de revisão quando houver uma decisão ou um conceito importante no código.
- Conferir a documentação ou o código da **versão instalada** das bibliotecas antes de usar uma API (os exemplos da internet costumam estar desatualizados).
- Apontar riscos de segurança, mesmo fora da tarefa pedida.
- Responder em **português**.

### Modo instrutor (quando o dono pedir para aprender uma tarefa)

- O dono escreve o código; o agente explica os conceitos, divide a tarefa em passos pequenos e revisa.
- Dicas em níveis, subindo só quando o dono travar: pergunta guia → conceito → assinatura/esqueleto → pseudocódigo → código (só se ele pedir).
- Não editar os arquivos da tarefa sem pedido explícito.

### O agente NÃO DEVE

- Commitar ou dar push sem pedido do dono.
- Adicionar dependências sem dizer quais e por quê.
- Editar migrations que já foram aplicadas; criar uma nova no lugar (o `sqlx` recusa uma migration aplicada que mudou: o checksum não confere).

## Contexto do projeto

- **Etapa atual: só Rust.** A API foi reescrita em Rust (`SPEC-migracao-rust.md`) e o TypeScript saiu do repositório depois que toda a suíte de testes foi portada para `tests/`. O último commit com o TS é o `53590d7`. Todas as etapas estão concluídas (roteiros abaixo, como histórico).
- Histórico: o projeto começou em Rust (tag `versao-rust`), foi migrado para TypeScript e depois reescrito em Rust do zero, para estudo do dono. Comentários do tipo "o equivalente ao `src/...ts`" no código apontam para o TS nesse histórico.

### Stack

| Parte | Escolha |
|---|---|
| Linguagem | Rust stable, edição 2024 |
| Runtime assíncrono | `tokio` |
| Framework web | `axum` 0.8 (+ `tower-http`: limite de corpo e pânico → 500) |
| Banco de dados | PostgreSQL 17 via Docker Compose; `sqlx` 0.9 (`query!`/`query_as!` conferidas contra o banco na compilação) |
| Migrations | `sqlx migrate` (`sqlx-cli`), arquivos em `migrations/` |
| JSON | `serde` / `serde_json`; validação dos corpos escrita à mão |
| Hash de senha | `argon2` (argon2id, 64 MiB, `t=3`, `p=4`) |
| Token de login | `jsonwebtoken` (HS256, `leeway = 0`, expira em 1 hora) |
| Rate limit | `governor` (GCRA, por IP, em memória), como extractor |
| Configuração | `.env` lido com `dotenvy` (procura no diretório atual e nos de cima) |
| Erros e logs | `thiserror` (`AppError`) e `tracing` |

### Estrutura

```
meu-backend/
├── Cargo.toml       # dependências; argon2/blake2 otimizados também no build de dev
├── migrations/      # SQL das migrations (0000..0004 vieram do drizzle-kit); nunca editar uma já aplicada
├── src/
│   ├── main.rs      # config, pool, limpeza dos contadores do rate limit, serve com ConnectInfo
│   ├── lib.rs       # declara os módulos (os testes usam o crate como biblioteca)
│   ├── app.rs       # build_app(state) e finish(): 404/405, corpo de 1 MiB, pânico → 500
│   ├── state.rs     # AppState { pool, tokens, limits } + FromRef para PgPool
│   ├── config.rs    # DATABASE_URL, JWT_SECRET (fail fast), RUST_PORT (padrão 3001)
│   ├── http/        # ligação com o axum (http.rs declara os submódulos)
│   │   ├── error.rs     # AppError -> { "error": ... } (status, WWW-Authenticate, Retry-After, 5xx genérico + log)
│   │   ├── json.rs      # JsonBody: só objeto JSON; rejeições do axum -> AppError
│   │   ├── auth.rs      # extractor CurrentUser (Bearer -> token -> token_version no banco)
│   │   └── rate_limit.rs  # extractor RateLimited<Rota>, um contador por rota
│   ├── security/{password,token}.rs  # argon2 e JWT
│   ├── validation.rs  # bad_request, REQUIRED, required_string, js_length
│   ├── validation/{user_fields,transaction_fields,dates}.rs  # regras por recurso e datas
│   ├── models/{user,transaction}.rs  # PublicUser e PublicTransaction (o que sai nas respostas)
│   ├── routes.rs
│   └── routes/{health,auth,users,transactions}.rs  # handlers e SQL
├── tests/
│   ├── common/mod.rs  # cria e migra o banco *_test, TestApp (requisições encadeadas), atalhos de cadastro/login
│   ├── register.rs, login.rs, logout.rs, users_me.rs
│   ├── users_update_delete.rs, users_password.rs, users.rs (corridas)
│   ├── transactions.rs, transactions_update_delete.rs
│   ├── errors.rs    # erros do framework (inclusive Content-Length errado por TCP de verdade)
│   ├── auth.rs      # 401 antes do corpo; banco fora do ar na autenticação
│   ├── rate_limit.rs  # único com o rate limit ligado, com os limites reais
│   └── health.rs
└── .claude/
    ├── agents/          # implementador e revisor (Sonnet 5.5, esforço médio)
    ├── skills/          # skills do projeto, copiadas do catálogo agent-skills e adaptáveis aqui
    ├── references/      # checklists citados pelas skills
    ├── catalog.md       # skills do catálogo ainda não instaladas (gerado; não editar)
    └── agent-skills.json  # de qual versão/commit do catálogo veio cada skill (gerado)
```

### Comandos úteis

```bash
docker compose up -d          # sobe o Postgres
cargo run                     # servidor na porta 3001 (lê o .env)
cargo test                    # testes (cria e migra o banco *_test sozinho)
cargo clippy --all-targets -- -D warnings && cargo fmt --check
sqlx migrate add <nome>       # cria uma migration nova em migrations/
sqlx migrate run              # aplica as migrations no banco do DATABASE_URL (.env)
sqlx migrate info             # o que já foi aplicado
```

O `query!` do `sqlx` confere o SQL contra o banco de dev na compilação: o Postgres precisa estar de pé (e migrado) para compilar.
O `sqlx-cli` é instalado com `cargo install sqlx-cli --no-default-features --features rustls,postgres`.

A URL do banco (`DATABASE_URL`) e o segredo do JWT (`JWT_SECRET`) ficam em `.env`. O `.env` não é versionado; `.env.example` é o modelo. O `.env.test` (versionado, só valores de teste) aponta para o banco `*_test`.

### Convenções

- Respostas da API em JSON. Erros no formato `{ "error": "mensagem" }`, sempre por `AppError`.
- Cada grupo de rotas expõe `router() -> Router<AppState>`; o `app.rs` monta com `nest`.
- Regras de campo em `validation/`, devolvendo `Result<T, AppError>`; handlers só leem o corpo, chamam as regras e fazem o SQL.
- SQL com `query!`/`query_as!` (verificado na compilação). SQL montado em tempo de execução só nos testes, com `AssertSqlSafe` e o valor conferido antes.
- Testes de integração em `tests/`, um arquivo por recurso, com `mod common;` e `TestApp`.

## Decisões registradas

- **Rust, para estudo** (`SPEC-migracao-rust.md`): o projeto passou por TypeScript por relevância de mercado; voltou a Rust porque o dono quer aprender a linguagem. O agente escreve e o dono lê.
- **Skills e agentes no próprio repo** (`.claude/`), não globais: o catálogo é o repositório `g-tavares14/agent-skills`, instalado pelo `/agent-skills:setup-project` (núcleo fixo, com `security-and-hardening`). A `spec` e a `plan` sugerem skills do `.claude/catalog.md`; só entram as aprovadas pelo dono (`/agent-skills:setup-project add <skill>`). Skills só deste projeto são criadas direto em `.claude/skills/`. Atalhos do fluxo: `/spec`, `/plan`, `/build`, `/verify`, `/review`.
- **axum + sqlx**: axum pelos extractors (autenticação, rate limit e corpo viram parâmetros do handler, na ordem certa); sqlx pelo SQL escrito à mão e conferido contra o banco na compilação, sem ORM.
- **Migrations pelo `sqlx migrate`**, SQL escrito à mão em `migrations/`. As 5 primeiras vieram do `drizzle-kit`; no banco de dev, a tabela `_sqlx_migrations` foi copiada de um banco migrado do zero, depois de conferir que o schema era idêntico. O schema `drizzle` que sobrou no banco de dev não é usado.
- **`409` mantido no cadastro** (email ou username em uso): o usuário precisa saber o motivo; aceitamos revelar quais emails têm conta, e o rate limit torna a varredura em massa lenta.
- **`role` fora do JWT**: a role pode mudar no banco e o token ficaria desatualizado; quem precisa dela lê `GET /users/me`.
- **`/users/me` no lugar de `/auth/me`**: o usuário atual é um recurso, e `/auth` fica para as ações de sessão (cadastro, login, logout). Isso também deixa espaço para `PATCH /users/me` e `PUT /users/me/password`.
- **Logout por `token_version`**: o JWT leva `ver` e só vale se for igual a `users.token_version`; o logout incrementa a coluna e derruba os tokens de todos os dispositivos, sem lista de tokens revogados.
- **Username só `a-z0-9_`, salvo em minúsculas** (`CHECK` no banco): impede personificação com `Joao`/`joao`, acentos, letras parecidas de outros alfabetos e caracteres invisíveis; o `UNIQUE` vira case-insensitive.
- **Valores em centavos inteiros** (`amount`, `bigint` no banco): nunca float para dinheiro; `19.9`, `0` e negativos dão `400`. O teto por registro é R$ 1 bilhão (`100000000000`). O front converte ao exibir e ao enviar.
- **Rota `/transactions`** (e não `/users/me/transactions`): o recurso é sempre do usuário do token, como o `/users/me`.
- **Totais calculados no banco** (`sum` com `FILTER` por tipo + `coalesce(..., 0)`), convertidos para `i64` no SQL: o `sum` de `bigint` volta como `numeric`. O front recebe número JSON, exato até 2^53 centavos (~R$ 90 trilhões).
- **Lista e totais em duas consultas, sem transação**: com um `POST` concorrente o `summary` pode ficar um registro fora de sincronia com a lista. Aceito pelo dono. Ambas usam a mesma condição `where` (usuário + `from`/`to`).
- **Autenticação como extractor (`CurrentUser`), o primeiro parâmetro das rotas protegidas**: o 401 vem antes de qualquer erro de corpo, e o corpo de quem não está autenticado nem é lido. Rota que não pede `CurrentUser` não tem acesso ao usuário (o compilador garante).
- **`PATCH` parcial, não `PUT`**: no HTTP, `PUT` substitui o registro inteiro; aqui o front manda só os campos que mudam (`type`, `amount`, `description`, `date`, qualquer combinação; `null` não apaga nada e dá `400`). As regras de cada campo são as mesmas do `POST` (`validation/transaction_fields.rs`); no SQL, `COALESCE` mantém o que não veio.
- **`404` igual para registro de outro usuário, inexistente, já excluído e `:id` não UUID** (`Registro não encontrado`), nunca `403`: o `403` confirmaria que o id existe. O `:id` não UUID vira `404` sem ir ao banco (o Postgres daria `500`).
- **`updated_at` sem versão nem histórico**: só a data da última gravação (`now()` do banco, igual ao `created_at` em registro nunca editado). Edições simultâneas do mesmo campo: vale a última. Um `PATCH` com os mesmos valores também atualiza o `updated_at`; um `PATCH` recusado não altera nada.
- **`0004` faz o backfill**: só o `ADD COLUMN ... DEFAULT now()` daria o horário da migration às linhas antigas; o `UPDATE "transactions" SET "updated_at" = "created_at"` foi acrescentado à migration **antes** de ela ser aplicada.
- **`DELETE` definitivo**: apaga a linha (sem lixeira nem exclusão lógica). A rota não lê corpo: um corpo enviado é ignorado.
- **Convenção de erros de validação** (cadastro, `POST`, `PATCH` e query do `GET`): tipo JSON errado, campo ausente ou query repetida (`?from=a&from=b`, que vira array) → `Campo obrigatório ausente ou inválido`; tipo certo com valor fora da regra → a mensagem do campo. No `PATCH`, campo ausente é permitido (só `null` ou tipo errado dão `required`) e nenhum dos quatro campos → `Envie ao menos um campo para alterar`.
- **Todo erro sai em português, em `{ "error": "..." }`**, por um lugar só: o `AppError` (`http/error.rs`) com `IntoResponse`. As rejeições do axum (corpo, tipo de conteúdo, tamanho, rota, método) são convertidas para `AppError` (`http/json.rs` e `app.rs`):
  - corpo que não é objeto JSON (vazio, malformado, `null`, `[]`, texto JSON, `Content-Length` errado) → `400` `Corpo da requisição inválido: envie um objeto JSON` (`INVALID_BODY`); as mensagens dos campos não mudam;
  - `415` (sem `Content-Type`, `text/plain` ou outro tipo que não seja JSON), `413` (corpo > 1 MiB), `404` (rota inexistente, inclusive URL malformada) e `405` (método errado) têm mensagem própria, sem repetir URL nem corpo;
  - todo 5xx (inclusive pânico num handler) sai `Erro interno do servidor`, com o erro completo no log e nada na resposta.
  Esses casos de borda diferem do antigo servidor TS; a lista para o front está em "Diferenças para o front", no `SPEC-migracao-rust.md`.
- **`/users/me` sem ações de admin**: cada usuário age só sobre a própria conta. `/users/:id` (admin) fica para uma spec própria, quando houver regras por `role`.
- **`PATCH /users/me` só `username` e `email`**, com as mesmas regras, normalização e mensagens do cadastro (`validation/user_fields.rs`) e o `409` `Email ou username já cadastrado` (`DUPLICATE_USER`). `role`, `password` e outros campos no corpo são ignorados; corpo sem `username` nem `email` dá `400` `Envie ao menos um campo para alterar`. `UPDATE ... RETURNING` numa consulta só; 0 linhas (conta apagada no meio) → `401` padrão.
- **Trocar o email não pede senha** (decisão do dono). **Risco**: quando existir recuperação de senha por email, trocar o email com um token vazado vira jeito de tomar a conta; nessa etapa, voltar a exigir a senha (ou confirmar pelo email antigo).
- **`DELETE /users/me` definitivo, com `{ "password" }` no corpo**: apaga o usuário e, pelo `CASCADE`, os registros financeiros dele. Senha errada → `403` `Senha incorreta` (não `401`, para o front não tratar como sessão expirada e deslogar); o hash é lido só nessa rota e na troca de senha (o `CurrentUser` não carrega o hash).
- **`PUT /users/me/password`** com `{ currentPassword, newPassword }` (a nova com a regra do cadastro; igual à atual é aceita). Senha atual errada → `403` `Senha incorreta`. Grava o hash e incrementa `token_version` numa consulta só, com `WHERE id AND token_version` do token (logout no meio → `0` linhas → `401`, nada muda), e responde `200` com um **token novo** no formato do login: os outros dispositivos caem, o atual continua logado. A versão do token fica em `CurrentUser.token_version`, fora do `PublicUser`, para nunca sair numa resposta.
- **Testes portados do vitest** (`tests/`): um arquivo por arquivo do vitest, com os mesmos casos. Os casos em tabela (`test.each`) viraram um teste com um laço, com o nome do caso em cada asserção. Os testes de internos do Fastify não foram portados; o comportamento que eles protegiam (500 genérico, pânico, banco fora do ar) tem teste próprio.
- **Testes no mesmo banco `_test`, em paralelo por arquivo**: cada `TestApp` segura uma trava (`Mutex` do tokio) do começo ao fim do teste e limpa as tabelas ao começar.
- **Rate limit como extractor (`governor`), não como layer (`tower_governor`)**: a layer rodaria antes do `CurrentUser` e contaria requisições sem token. Desligável só no código (`without_rate_limit()`), nunca por variável de ambiente.
- **Rate limit em memória, por IP** (login 5/min, cadastro 3/min, `DELETE /users/me` 5/min, `PATCH /users/me` 10/min, `PUT /users/me/password` 5/min; conta toda requisição autenticada, inclusive os `400`): protege contra força bruta e contra o consumo de memória do argon2 (64 MiB por hash).
  Nas rotas de `/users/me` o `RateLimited` vem depois do `CurrentUser`: sem token vem `401` e a requisição não consome o limite. O GCRA libera a cota aos poucos (o `Retry-After` fica em até ~12 s), em vez de zerar a cada minuto.
  O IP vem da conexão (`ConnectInfo`). Atrás de proxy reverso ou CDN (ex.: Cloudflare) é **obrigatório** ler o IP real de um header confiável; senão todos compartilham o IP do proxy e são bloqueados juntos.
  Com mais de um processo, os contadores não são compartilhados: seria preciso um store externo (ex.: Redis).
  Rotas inexistentes (`404`) não têm rate limit: varredura de rotas só é freada pelo que houver na frente (ex.: um proxy ou CDN).

## Roteiros das etapas (histórico)

As etapas até a de endpoints restantes foram feitas no servidor TypeScript: os nomes de arquivos e funções citados nelas são do TS (no histórico do git).

## Roteiro da etapa de autenticação (concluída)

1. ✅ **Servidor mínimo**: `GET /health`, verificando também o banco (200 / 503).
2. ✅ **Configuração**: `DATABASE_URL` do ambiente, com fail fast.
3. ✅ **Conexão com o banco**: pool do `pg` + Drizzle.
4. ✅ **Tabela `users`**: `id` (uuid), `username` (único), `email` (único), `password_hash`, `role`, `token_version`, `created_at`.
5. ✅ **`POST /auth/register`**: valida, normaliza, gera hash, salva e trata duplicados (`409`).
6. ✅ **Tratamento de erros**: error handler central; 5xx genérico para o cliente e detalhado no log.
7. ✅ **`POST /auth/login`**: verifica a senha e devolve um JWT (`sub` = id do usuário, `ver` = versão do token, `iat`, `exp`; sem `role`).
8. ✅ **`GET /users/me`**: rota protegida; valida o token do header `Authorization: Bearer` e devolve os dados atuais do usuário.
9. ✅ **Testes automatizados**: fluxo feliz e principais erros de cada rota.
10. ✅ **Endurecimento da autenticação** (spec em `SPEC-auth-hardening.md`):
    - **Username**: normalizado (`trim` + minúsculas) e restrito a `a-z0-9_`, com `CHECK` no banco.
    - **Token e logout**: JWT sem `role` e com `ver`; `POST /auth/logout` revoga todos os tokens do usuário (`token_version`).
    - **Rate limit**: 5 logins e 3 cadastros por minuto por IP (`429` + `Retry-After`).
    - **Postgres**: porta publicada só em `127.0.0.1`.

## Roteiro da etapa de registros financeiros

Spec em `SPEC-transactions.md`, plano em `tasks/plan.md`, tarefas em `tasks/todo.md`.

1. ✅ **Tabela `transactions`** (migration `0003`): FK para `users` com `ON DELETE CASCADE`, `CHECK` de `type` e de `amount_cents > 0`, índice `(user_id, occurred_on)`.
2. ✅ **`POST /transactions`**: cria o registro com o `user_id` do token (`userId` no corpo é ignorado) e responde `201` sem expor o `user_id`; `badRequest()` e `required` foram para `src/lib/validation.ts`.
3. ✅ **Validação do `POST`**: `type`, `amount` (centavos inteiros), `description` (com `trim`) e `date` (AAAA-MM-DD válida), com as mensagens da spec.
4. ✅ **`GET /transactions`**: lista (data mais recente primeiro, desempate por `createdAt`) + `summary` (`income`, `expense`, `balance`) calculado no banco; isolado por usuário.
5. ✅ **Filtro `from`/`to`** (inclusivos, um ou os dois): a lista e os totais usam a mesma condição; datas inválidas e `from` depois de `to` dão `400`.
6. ✅ **Refatoração da autenticação**: `authenticate()` virou o hook `onRequest` `requireAuth(db)` + `currentUser(request)`; `test/require-auth.test.ts` cobre a ordem (401 antes do parse do corpo).
7. ✅ **Documentação**: este arquivo, a spec (critérios e resumo do contrato para o front) e o `todo.md`.

## Roteiro da etapa de edição e exclusão de registros financeiros (concluída)

Spec em `SPEC-transactions-update-delete.md`, plano em `tasks/plan.md`, tarefas em `tasks/todo.md`.

1. ✅ **Coluna `updated_at`** (migration `0004`): `NOT NULL DEFAULT now()`, com `UPDATE ... SET updated_at = created_at` para as linhas antigas; `updatedAt` em todas as respostas com registro (`POST`, `GET`, `PATCH`).
2. ✅ **`DELETE /transactions/:id`**: `204` sem corpo; `parseId()` (`z.uuid()`) e `notFound()` (`404` `Registro não encontrado`) criados no `transactions.ts`.
3. ✅ **`PATCH /transactions/:id`** (caminho feliz): altera só os campos enviados (`SET` com o que veio + `updated_at = now()`), responde `200` com o registro inteiro; isolamento por usuário, `404` e `401`.
4. ✅ **Validação do `PATCH`**: mesmas regras e mensagens do `POST` por campo, `Envie ao menos um campo para alterar` para corpo sem campos, campos extras ignorados, ordem dos erros (`401` → corpo do Fastify → `:id` → campos → `404`).
5. ✅ **Documentação**: este arquivo, o contrato para o front em `SPEC-transactions.md` e os critérios da spec marcados.

## Roteiro da etapa de edição e exclusão da conta

Spec em `SPEC-users-update-delete.md`, plano em `tasks/plan.md`, tarefas em `tasks/todo.md`.

1. ✅ **Regras compartilhadas**: `usernameSchema`/`emailSchema` (`user-fields.ts`) e `isUniqueViolation` (`db-errors.ts`) saíram do `auth.ts`, sem mudar o comportamento do cadastro e do login.
2. ✅ **`PATCH /users/me`** (caminho feliz): `username` e/ou `email` normalizados, `200` no formato do `GET /users/me`, `409` e `401`; `unauthorized()` e `publicUserColumns` exportados do `authenticate.ts`.
3. ✅ **Validação do `PATCH`**: mensagens do cadastro, `Envie ao menos um campo para alterar`, `null`/tipo errado, `role` e `password` ignorados.
4. ✅ **`DELETE /users/me`**: senha no corpo, `403` para senha errada, `204` e `CASCADE` nos registros financeiros.
5. ✅ **Rate limit**: `DELETE` 5/min e `PATCH` 10/min por IP, depois do `requireAuth`.
6. ✅ **Documentação**: este arquivo e a spec (status e critérios).

## Roteiro da etapa de endpoints restantes (concluída)

Spec em `SPEC-endpoints-restantes.md`, plano em `tasks/plan.md`, tarefas em `tasks/todo.md`.

1. ✅ **`passwordSchema` compartilhado** (`user-fields.ts`), sem mudar o cadastro.
2. ✅ **`PUT /users/me/password`**: token novo, `token_version` + 1 na mesma consulta, `403`, `401` e corrida; `request.tokenVersion` e `currentTokenVersion()` no `authenticate.ts`.
3. ✅ **Validação e rate limit** (5/min, depois do `requireAuth`).
4. ✅ **Documentação**: este arquivo e a spec.

## Roteiro da etapa de migração para Rust (concluída)

Spec em `SPEC-migracao-rust.md`, plano em `tasks/plan.md`, tarefas em `tasks/todo.md`.

1. ✅ **Esqueleto** (T0), **config + `/health`** (T1), **`AppError`** (T2) e **modo paridade no vitest** (T3).
2. ✅ **Cadastro** (T4), **JWT + login** (T5), **`CurrentUser`, `/users/me` e logout** (T6).
3. ✅ **`PATCH /users/me`** (T7), **`DELETE /users/me` e troca de senha** (T8), **`POST`/`GET /transactions`** (T9), **`PATCH`/`DELETE /transactions/:id`** (T10).
4. ✅ **Rate limit** (T11) e **documentação** (T12).

## Roteiro da remoção do TypeScript (concluída)

Plano em `tasks/plan.md`, tarefas em `tasks/todo.md`.

1. ✅ **Migrations no sqlx** (R1) e **helpers de teste** (R2).
2. ✅ **Porte da suíte do vitest** (R3–R6).
3. ✅ **TypeScript removido e documentação** (R7).

### Regras de segurança (verificar em toda mudança)

- Nunca salvar nem logar senha em texto puro. Não logar hash de senha nem dados pessoais.
- Login com email ou senha errados retorna a **mesma** mensagem de erro (com verificação simulada quando o email não existe).
- A `role` nunca vem da requisição: novos usuários usam o `DEFAULT` do banco.
- `JWT_SECRET` vem do ambiente, nunca fica fixo no código. Tokens com expiração (`exp`), `leeway = 0`.
- Consultas sempre parametrizadas (`$1`, `query!`); nunca montar SQL com `format!` a partir de dados de fora.
- Não expor detalhes internos (erro do banco, stack trace) na resposta HTTP.
- Toda rota protegida pede `CurrentUser`, que confere a assinatura, a expiração e a `token_version` do token. Toda consulta a `transactions` filtra por `user_id` do token (nunca da requisição); `UPDATE` e `DELETE` também: `id` e `user_id` na mesma condição, numa consulta só, sem "ler e depois gravar".
- A troca de senha incrementa `token_version` (derruba os tokens emitidos com a senha antiga); qualquer nova forma de mudar a senha (ex.: recuperação por email) deve fazer o mesmo.
- Nunca desligar o rate limit fora dos testes (`without_rate_limit()` só em `tests/`).
- Nas rotas com limite de `/users/me`, o `RateLimited` vem **depois** do `CurrentUser`, e nas públicas (login, cadastro) **antes** do `JsonBody`.
- Sem `unwrap()`/`expect()` em caminho de requisição; argon2 sempre em `spawn_blocking`.
