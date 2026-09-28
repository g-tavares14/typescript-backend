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

- **Etapa atual: autenticação.**
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
├── app.ts           # monta o Fastify: error handler, rate limit e rotas
├── config.ts        # variáveis de ambiente (fail fast se faltar alguma)
├── db/
│   ├── client.ts    # pool de conexões + Drizzle
│   └── schema.ts    # definição das tabelas (fonte das migrations)
├── lib/
│   ├── authenticate.ts  # authenticate() (token -> usuário) e unauthorized() (401 padrão)
│   ├── password.ts  # hash e verificação de senha
│   └── token.ts     # geração e verificação do JWT
└── routes/
    ├── health.ts    # GET /health
    ├── auth.ts      # POST /auth/register, /auth/login e /auth/logout
    └── users.ts     # GET /users/me
test/
├── global-setup.ts  # cria o banco de testes (_test) e aplica as migrations
├── helpers.ts       # createTestApp (rate limit desligado por padrão) e atalhos de cadastro/login
├── register.test.ts, login.test.ts, users-me.test.ts, logout.test.ts
└── rate-limit.test.ts  # único que liga o rate limit, com os limites reais
drizzle/             # migrations SQL geradas pelo drizzle-kit
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
- **Fastify + Drizzle**: Fastify pela estrutura simples de rotas e bom suporte a TypeScript;
  Drizzle por ser leve, com sintaxe próxima de SQL e tipos inferidos direto do schema.
- **Sem etapa de build**: o Node 22 executa `.ts` removendo os tipos; o `tsc` é usado só para verificar os tipos.
- **Migrations geradas pelo `drizzle-kit`** a partir do `src/db/schema.ts`. Sempre revisar o SQL gerado antes de aplicar.
- **`409` mantido no cadastro** (email ou username em uso): o usuário precisa saber o motivo; aceitamos revelar quais emails têm conta, e o rate limit torna a varredura em massa lenta.
- **`role` fora do JWT**: a role pode mudar no banco e o token ficaria desatualizado; quem precisa dela lê `GET /users/me`.
- **`/users/me` no lugar de `/auth/me`**: o usuário atual é um recurso, e `/auth` fica para as ações de sessão (cadastro, login, logout). Isso também deixa espaço para `PATCH /users/me` e `PUT /users/me/password`.
- **Logout por `token_version`**: o JWT leva `ver` e só vale se for igual a `users.token_version`; o logout incrementa a coluna e derruba os tokens de todos os dispositivos, sem lista de tokens revogados.
- **Username só `a-z0-9_`, salvo em minúsculas** (`CHECK` no banco): impede personificação com `Joao`/`joao`, acentos, letras parecidas de outros alfabetos e caracteres invisíveis; o `UNIQUE` vira case-insensitive.
- **Rate limit em memória, por IP** (login 5/min, cadastro 3/min): protege contra força bruta e contra o consumo de memória do argon2 (64 MiB por hash).
  Atrás de proxy reverso é **obrigatório** configurar `trustProxy` no Fastify; senão todos compartilham o IP do proxy e são bloqueados juntos.
  Com mais de um processo, os contadores não são compartilhados: trocar o store por Redis.

## Roteiro da etapa de autenticação

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

### Regras de segurança (verificar em toda mudança)

- Nunca salvar nem logar senha em texto puro. Não logar hash de senha nem dados pessoais (cuidado com os parâmetros de consultas nos erros do Drizzle).
- Login com email ou senha errados retorna a **mesma** mensagem de erro.
- A `role` nunca vem da requisição: novos usuários usam o `DEFAULT` do banco.
- `JWT_SECRET` vem do ambiente, nunca fica fixo no código. Tokens com expiração (`exp`).
- Consultas sempre parametrizadas (o Drizzle faz isso); nunca montar SQL concatenando strings. No `sql\`...\``, só interpolar valores, nunca texto de SQL vindo de fora.
- Não expor detalhes internos (erro do banco, stack trace) na resposta HTTP.
- Toda rota protegida usa `authenticate()` (`src/lib/authenticate.ts`): ela confere a assinatura, a expiração e a `token_version` do token.
- A troca de senha (futura) deve incrementar `token_version`, para derrubar os tokens emitidos com a senha antiga.
- Nunca desligar o rate limit fora dos testes (`rateLimit: false` só em `test/helpers.ts`).
