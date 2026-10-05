# AGENTS.md

Instruções para qualquer agente de IA (Claude, Copilot, Cursor, etc.) que trabalhar neste repositório.

## Papel do agente: escreve o código, o dono revisa

O dono do repositório faz o **backend em TypeScript** de um projeto em dupla (um amigo faz o frontend).
O agente **implementa** as tarefas e o dono **revisa**. Por isso, cada entrega deve ser fácil de revisar.

### O agente DEVE

- Implementar em passos pequenos, verificando cada um (`npm run typecheck` e testes reais com `curl`) antes de seguir.
- Ao terminar, explicar **o que mudou e por quê**, destacando conceitos novos de TypeScript, Node.js, Fastify ou Drizzle.
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
- Editar migrations que já foram aplicadas; criar uma nova no lugar.

## Contexto do projeto

- **Etapa atual: migração para Rust** (módulo `migracao-rust` do `CAPABILITY-MAP.md`; spec em `SPEC-migracao-rust.md`, plano ainda não feito). A troca de senha (`PUT /users/me/password`, `SPEC-endpoints-restantes.md`) fechou o contrato que o Rust vai copiar. As etapas de autenticação, registros financeiros e edição/exclusão da conta estão concluídas (roteiros abaixo, como histórico).
- O projeto começou em Rust e foi migrado para TypeScript. A versão em Rust está na tag `versao-rust`.

### Stack

| Parte | Escolha |
|---|---|
| Linguagem | TypeScript 7 |
| Runtime | Node.js 22 (executa `.ts` direto, sem etapa de build) |
| Framework web | Fastify 5 |
| Banco de dados | PostgreSQL 17 via Docker Compose |
| ORM e migrations | Drizzle ORM + `drizzle-kit` (driver `pg`) |
| Validação | `zod` |
| Hash de senha | `argon2` (argon2id) |
| Token de login | JWT com `jose` (HS256, expira em 1 hora) |
| Rate limit | `@fastify/rate-limit` (contadores em memória, por IP) |
| Configuração | `.env` carregado pelo próprio Node (`--env-file-if-exists`) |
| Logs | `pino` (logger embutido do Fastify) |

### Estrutura

```
src/
├── server.ts        # ponto de entrada: lê a config, conecta no banco, sobe o servidor
├── app.ts           # monta o Fastify: error handler, campo `user` da requisição, rate limit e rotas
├── config.ts        # variáveis de ambiente (fail fast se faltar alguma)
├── db/
│   ├── client.ts    # pool de conexões + Drizzle
│   └── schema.ts    # definição das tabelas (fonte das migrations)
├── lib/
│   ├── authenticate.ts  # requireAuth(db) (hook onRequest: token -> request.user e request.tokenVersion, ou 401 padrão),
│   │                    # currentUser(request), currentTokenVersion(request),
│   │                    # unauthorized(reply) e publicUserColumns (colunas do usuário que podem sair numa resposta)
│   ├── user-fields.ts  # usernameSchema, emailSchema e passwordSchema (cadastro, login, PATCH /users/me e troca de senha)
│   │                   # e DUPLICATE_USER
│   ├── db-errors.ts  # isUniqueViolation(): código 23505 do Postgres dentro do erro do Drizzle
│   ├── password.ts  # hash e verificação de senha
│   ├── token.ts     # geração e verificação do JWT
│   ├── validation.ts  # badRequest() (400 com a primeira mensagem do Zod), `required` e `INVALID_BODY`
│   └── errors.ts    # sendError(): resposta de todo erro (4xx em português, por error.code; 5xx genérico + log)
└── routes/
    ├── health.ts    # GET /health
    ├── auth.ts      # POST /auth/register, /auth/login e /auth/logout
    ├── users.ts     # GET, PATCH (username/email) e DELETE (com senha) /users/me e PUT /users/me/password
    └── transactions.ts  # POST, GET (lista + totais, filtro from/to), PATCH /:id (parcial) e DELETE /:id
test/
├── global-setup.ts  # cria o banco de testes (_test) e aplica as migrations
├── helpers.ts       # createTestApp (rate limit desligado por padrão) e atalhos de cadastro/login
├── register.test.ts, login.test.ts, users-me.test.ts, logout.test.ts
├── users-update-delete.test.ts  # PATCH e DELETE /users/me (normalização, 409, validação, senha, CASCADE, corrida, 401)
├── users-password.test.ts  # PUT /users/me/password (token novo, tokens antigos caem, 403, validação, corrida, 401)
├── transactions.test.ts  # POST e GET /transactions (validação, totais, ordem, filtro, isolamento, 401)
├── transactions-update-delete.test.ts  # PATCH e DELETE /transactions/:id (parcial, updatedAt, validação, ordem dos erros, isolamento, 404, 401)
├── require-auth.test.ts  # requireAuth roda antes do parse do corpo; currentUser sem hook; falha do banco no hook
├── errors.test.ts  # sendError: 4xx em português (corpo inválido, 404, 413, 415, URL malformada, 429, 401 antes do 400),
│                   # warn só com o code em FST_* sem mapeamento, 5xx genérico + log (inclusive via frameworkErrors)
└── rate-limit.test.ts  # único que liga o rate limit, com os limites reais (login, cadastro, PATCH, DELETE e PUT password de /users/me)
drizzle/             # migrations SQL geradas pelo drizzle-kit
.claude/
├── agents/          # implementador e revisor (Sonnet 5.5, esforço alto)
├── skills/          # skills do projeto, copiadas do catálogo agent-skills e adaptáveis aqui
├── references/      # checklists citados pelas skills
├── catalog.md       # skills do catálogo ainda não instaladas (gerado; não editar)
└── agent-skills.json  # de qual versão/commit do catálogo veio cada skill (gerado)
```

### Comandos úteis

```bash
docker compose up -d                          # sobe o Postgres
npm run dev                                   # servidor com reload automático
npm start                                     # servidor
npm run typecheck                             # verificação de tipos (tsc)
npm test                                      # testes automatizados (vitest, banco *_test)
npm run db:generate -- --name <nome>          # gera migration a partir do schema.ts
npm run db:migrate                            # aplica as migrations
```

A URL do banco (`DATABASE_URL`) e o segredo do JWT (`JWT_SECRET`) ficam em `.env`. O `.env` não é versionado; `.env.example` é o modelo.

### Convenções

- Imports relativos com extensão `.ts` (exigência do Node ao executar TypeScript direto).
- Só sintaxe de TypeScript que pode ser "apagada" (`erasableSyntaxOnly`): nada de `enum`, `namespace` ou parameter properties.
- Respostas da API em JSON. Erros no formato `{ "error": "mensagem" }`.
- Rotas são plugins do Fastify que recebem o `db` nas opções.

## Decisões registradas

- **TypeScript em vez de Rust**: decisão do dono, por relevância de mercado.
- **Skills e agentes no próprio repo** (`.claude/`), não globais: o catálogo é o repositório `g-tavares14/agent-skills`, instalado pelo `/agent-skills:setup-project` (núcleo fixo, com `security-and-hardening`). A `spec` e a `plan` sugerem skills do `.claude/catalog.md`; só entram as aprovadas pelo dono (`/agent-skills:setup-project add <skill>`). Skills só deste projeto são criadas direto em `.claude/skills/`. Atalhos do fluxo: `/spec`, `/plan`, `/build`, `/verify`, `/review`.
- **Fastify + Drizzle**: Fastify pela estrutura simples de rotas e bom suporte a TypeScript;
  Drizzle por ser leve, com sintaxe próxima de SQL e tipos inferidos direto do schema.
- **Sem etapa de build**: o Node 22 executa `.ts` removendo os tipos; o `tsc` é usado só para verificar os tipos.
- **Migrations geradas pelo `drizzle-kit`** a partir do `src/db/schema.ts`. Sempre revisar o SQL gerado antes de aplicar.
- **`409` mantido no cadastro** (email ou username em uso): o usuário precisa saber o motivo; aceitamos revelar quais emails têm conta, e o rate limit torna a varredura em massa lenta.
- **`role` fora do JWT**: a role pode mudar no banco e o token ficaria desatualizado; quem precisa dela lê `GET /users/me`.
- **`/users/me` no lugar de `/auth/me`**: o usuário atual é um recurso, e `/auth` fica para as ações de sessão (cadastro, login, logout). Isso também deixa espaço para `PATCH /users/me` e `PUT /users/me/password`.
- **Logout por `token_version`**: o JWT leva `ver` e só vale se for igual a `users.token_version`; o logout incrementa a coluna e derruba os tokens de todos os dispositivos, sem lista de tokens revogados.
- **Username só `a-z0-9_`, salvo em minúsculas** (`CHECK` no banco): impede personificação com `Joao`/`joao`, acentos, letras parecidas de outros alfabetos e caracteres invisíveis; o `UNIQUE` vira case-insensitive.
- **Valores em centavos inteiros** (`amount`, `bigint` no banco): nunca float para dinheiro; `19.9`, `0` e negativos dão `400`. O teto por registro é R$ 1 bilhão (`100000000000`). O front converte ao exibir e ao enviar.
- **Rota `/transactions`** (e não `/users/me/transactions`): o recurso é sempre do usuário do token, como o `/users/me`.
- **Totais calculados no banco** (`sum` com `FILTER` por tipo + `coalesce(..., 0)` + `.mapWith(Number)`): o `sum` de `bigint` volta como `numeric`, que o `pg` entrega como string. `Number()` é exato até 2^53 centavos (~R$ 90 trilhões).
- **Lista e totais em duas consultas, sem transação**: com um `POST` concorrente o `summary` pode ficar um registro fora de sincronia com a lista. Aceito pelo dono. Ambas usam a mesma condição `where` (usuário + `from`/`to`).
- **`requireAuth` como hook `onRequest`, não `preHandler`**: o 401 vem antes do parse do corpo, e o corpo de quem não está autenticado nem é lido. Hook no plugin inteiro (`users.ts`, `transactions.ts`) ou na opção da rota (`/auth/logout`, porque o plugin `/auth` tem rotas públicas).
- **`currentUser()` falha alto** (`throw` → 500 genérico + log) se chamado numa rota sem o hook, em vez de devolver `null`: esquecer o `requireAuth` aparece no primeiro teste.
- **`PATCH` parcial, não `PUT`**: no HTTP, `PUT` substitui o registro inteiro; aqui o front manda só os campos que mudam (`type`, `amount`, `description`, `date`, qualquer combinação; `null` não apaga nada e dá `400`). Schema = `createTransactionSchema.partial()` + `refine` "ao menos um campo".
- **`404` igual para registro de outro usuário, inexistente, já excluído e `:id` não UUID** (`Registro não encontrado`), nunca `403`: o `403` confirmaria que o id existe. O `:id` não UUID vira `404` sem ir ao banco (o Postgres daria `500`).
- **`updated_at` sem versão nem histórico**: só a data da última gravação (`now()` do banco, igual ao `created_at` em registro nunca editado). Edições simultâneas do mesmo campo: vale a última. Um `PATCH` com os mesmos valores também atualiza o `updated_at`; um `PATCH` recusado não altera nada.
- **`0004` faz o backfill**: o `drizzle-kit` gera só o `ADD COLUMN ... DEFAULT now()`, que daria o horário da migration às linhas antigas; o `UPDATE "transactions" SET "updated_at" = "created_at"` foi acrescentado à migration **antes** de ela ser aplicada.
- **`DELETE` definitivo**: apaga a linha (sem lixeira nem exclusão lógica). Sem corpo: o front não envia `Content-Type` (com `application/json` e sem corpo o Fastify responde `400`).
- **Convenção de erros de validação** (cadastro, `POST`, `PATCH` e query do `GET`): tipo JSON errado, campo ausente ou query repetida (`?from=a&from=b`, que vira array) → `Campo obrigatório ausente ou inválido`; tipo certo com valor fora da regra → a mensagem do campo. No `PATCH`, campo ausente é permitido (só `null` ou tipo errado dão `required`) e nenhum dos quatro campos → `Envie ao menos um campo para alterar`.
- **Todo erro 4xx sai em português, em `{ "error": "..." }`**, mapeado por `error.code` (nunca pelo texto da mensagem) num lugar só, `sendError()` em `src/lib/errors.ts`, usada nas duas portas de erro do Fastify (`setErrorHandler` e `frameworkErrors`, que cobre erros gerados antes de escolher a rota):
  - corpo que não é objeto JSON (ausente, vazio, malformado, `null`, `[]`, texto, `Content-Length` errado) → `400` `Corpo da requisição inválido: envie um objeto JSON` (constante `INVALID_BODY`, em `src/lib/validation.ts`, também usada como `error` na raiz dos `z.object` de corpo; as mensagens dos campos não mudam);
  - `415` (tipo de conteúdo), `413` (corpo > 1 MiB), `404` (`setNotFoundHandler`) e `400` de URL malformada (opção `frameworkErrors`) têm mensagem própria, sem repetir URL nem corpo;
  - outro código `FST_*` → `Requisição inválida`, com o mesmo status, e loga só o código (`warn`, nunca URL, corpo ou a mensagem original); 4xx mapeados e 4xx sem código `FST_*` (ex.: o `429` do rate limit, que mantém a própria mensagem) não são logados;
  - todo 5xx, inclusive os do próprio Fastify (ex.: `FST_ERR_ASYNC_CONSTRAINT`), sai `Erro interno do servidor` com o erro completo no log.
- **`/users/me` sem ações de admin**: cada usuário age só sobre a própria conta. `/users/:id` (admin) fica para uma spec própria, quando houver regras por `role`.
- **`PATCH /users/me` só `username` e `email`**, com as mesmas regras, normalização e mensagens do cadastro (`user-fields.ts`) e o `409` `Email ou username já cadastrado` (`DUPLICATE_USER`). `role`, `password` e outros campos no corpo são ignorados; corpo sem `username` nem `email` dá `400` `Envie ao menos um campo para alterar`. `UPDATE ... RETURNING` numa consulta só; 0 linhas (conta apagada no meio) → `401` padrão.
- **Trocar o email não pede senha** (decisão do dono). **Risco**: quando existir recuperação de senha por email, trocar o email com um token vazado vira jeito de tomar a conta; nessa etapa, voltar a exigir a senha (ou confirmar pelo email antigo).
- **`DELETE /users/me` definitivo, com `{ "password" }` no corpo**: apaga o usuário e, pelo `CASCADE`, os registros financeiros dele. Senha errada → `403` `Senha incorreta` (não `401`, para o front não tratar como sessão expirada e deslogar); o hash é lido só nessa rota (o `currentUser()` não carrega `passwordHash`). Corpo com `text/plain` dá `INVALID_BODY` (o Fastify tem parser de texto); só tipo sem parser (ex.: `application/xml`) dá `415`.
- **`PUT /users/me/password`** com `{ currentPassword, newPassword }` (a nova com a regra do cadastro, `passwordSchema`; igual à atual é aceita). Senha atual errada → `403` `Senha incorreta`. Grava o hash e incrementa `token_version` numa consulta só, com `WHERE id AND token_version` do token (logout no meio → `0` linhas → `401`, nada muda), e responde `200` com um **token novo** no formato do login: os outros dispositivos caem, o atual continua logado. A versão do token fica em `request.tokenVersion`, fora do `user`, para nunca sair numa resposta.
- **Rust de novo, agora para estudo** (`SPEC-migracao-rust.md`): o dono quer aprender Rust; o agente escreve e o dono lê. A decisão "TypeScript em vez de Rust" continua valendo para o mercado; o TS fica no repo até o Rust passar na paridade. Casos de borda de framework seguem o axum (listados na spec, em "Diferenças para o front"); regras de negócio e de segurança não mudam.
- **Rate limit em memória, por IP** (login 5/min, cadastro 3/min, `DELETE /users/me` 5/min, `PATCH /users/me` 10/min, `PUT /users/me/password` 5/min; conta toda requisição autenticada, inclusive os `400`): protege contra força bruta e contra o consumo de memória do argon2 (64 MiB por hash).
  Nas rotas de `/users/me` o hook do limite é da rota e roda depois do `requireAuth` do plugin: sem token vem `401` e a requisição não consome o limite.
  Atrás de proxy reverso é **obrigatório** configurar `trustProxy` no Fastify; senão todos compartilham o IP do proxy e são bloqueados juntos.
  Com mais de um processo, os contadores não são compartilhados: trocar o store por Redis.
  Rotas inexistentes (`404`) não têm rate limit: varredura de rotas só é freada pelo que houver na frente (ex.: um proxy ou CDN).

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

### Regras de segurança (verificar em toda mudança)

- Nunca salvar nem logar senha em texto puro. Não logar hash de senha nem dados pessoais (cuidado com os parâmetros de consultas nos erros do Drizzle).
- Login com email ou senha errados retorna a **mesma** mensagem de erro.
- A `role` nunca vem da requisição: novos usuários usam o `DEFAULT` do banco.
- `JWT_SECRET` vem do ambiente, nunca fica fixo no código. Tokens com expiração (`exp`).
- Consultas sempre parametrizadas (o Drizzle faz isso); nunca montar SQL concatenando strings. No `sql\`...\``, só interpolar valores, nunca texto de SQL vindo de fora.
- Não expor detalhes internos (erro do banco, stack trace) na resposta HTTP.
- Toda rota protegida usa `requireAuth` (hook no plugin ou na rota, `src/lib/authenticate.ts`) e lê o usuário com `currentUser()`: o hook confere a assinatura, a expiração e a `token_version` do token. Toda consulta a `transactions` filtra por `user_id` do token (nunca da requisição); `UPDATE` e `DELETE` também: `id` e `user_id` na mesma condição, numa consulta só, sem "ler e depois gravar".
- A troca de senha incrementa `token_version` (derruba os tokens emitidos com a senha antiga); qualquer nova forma de mudar a senha (ex.: recuperação por email) deve fazer o mesmo.
- Nunca desligar o rate limit fora dos testes (`rateLimit: false` só em `test/helpers.ts`).
