# Spec: Endurecimento da autenticação

Origem: revisão do fluxo de auth de 28/09/2026. Os 4 bugs achados pelos testes já foram corrigidos;
esta spec cobre os pontos que dependiam de decisão do dono.

## Objetivo

Fechar os riscos abertos da autenticação antes de seguir para a próxima etapa:

1. **Força bruta e DoS**: limitar tentativas em `POST /auth/login` e `POST /auth/register`
   (cada hash argon2 usa 64 MiB de RAM).
2. **Personificação por username**: `Joao` e `joao` não podem coexistir; nada de acento,
   espaço, alfabetos parecidos ou caracteres invisíveis.
3. **Banco exposto na rede**: o Postgres do Docker só aceita conexões da própria máquina.
4. **Role desatualizada no token**: o JWT deixa de carregar a `role`.
5. **Logout**: o usuário consegue invalidar os próprios tokens antes de expirarem.
6. **Registrar decisões** (409 no cadastro, rate limit atrás de proxy) no AGENTS.md.

### Critérios de aceite (comportamento da API)

**Rate limit**
- `POST /auth/login`: no máximo **5 requisições por minuto por IP**. A 6ª responde `429`.
- `POST /auth/register`: no máximo **3 requisições por minuto por IP**. A 4ª responde `429`.
- O `429` segue o padrão da API: `{ "error": "Muitas tentativas. Tente novamente mais tarde." }`
  e o header `Retry-After` (segundos).
- O limite é checado **antes** de validar o corpo e de calcular qualquer hash (hook `onRequest`).
- As outras rotas (`/health`, `/auth/me`, `/auth/logout`) não têm limite.

**Username**
- É normalizado com `trim()` + `toLowerCase()` antes de validar e salvar (como o email).
- Formato: `^[a-z0-9_]{3,50}$`. Fora disso: `400` com
  `{ "error": "O username só pode ter letras sem acento, números e _" }`
  (o erro de tamanho continua com a mensagem atual).
- `Joao` depois de `joao` → `409` (é o mesmo username).
- O banco garante a regra com um `CHECK` (defesa em profundidade: vale mesmo se alguém inserir sem passar pela API).

**Token**
- Payload do JWT: `sub` (id), `ver` (versão do token), `iat`, `exp`. **Sem `role`.**
- Um token é válido só se `ver` for igual ao `token_version` atual do usuário no banco.
- A resposta do login não muda: `{ token, tokenType: "Bearer", expiresIn: 3600 }`.

**Logout**
- `POST /auth/logout` com `Authorization: Bearer <token>` válido → `204` sem corpo,
  e **todos** os tokens daquele usuário deixam de valer (logout em todos os dispositivos).
- Depois do logout, o token antigo recebe `401` em `/auth/me` e em `/auth/logout`.
- Um novo login gera um token que funciona normalmente.
- Tokens de outros usuários não são afetados.
- Sem token / token inválido → `401` igual ao `/me` (`WWW-Authenticate: Bearer`).

**Cadastro (decisão, sem mudança de código)**
- Continua respondendo `409` para email/username em uso. Aceitamos que isso revela quais emails
  têm conta: o usuário precisa saber, e o rate limit torna a varredura em massa lenta.

### Mudanças de contrato (avisar o frontend)

| Mudança | Impacto no front |
|---|---|
| `tokenType` voltou a ser `"Bearer"` (sem espaço) | Montar o header como `` `${tokenType} ${token}` `` |
| Sem `role` no JWT | Ler a role pelo `GET /auth/me` |
| Username só `a-z`, `0-9`, `_`, salvo em minúsculas | Validar no formulário; exibir o username que a API devolve |
| Nova rota `POST /auth/logout` (204) | Chamar no "Sair" e apagar o token local |
| `429` em login/cadastro | Mostrar a mensagem e respeitar o `Retry-After` |

## Tech Stack

A atual (ver AGENTS.md), mais **uma dependência nova**:

- `@fastify/rate-limit@^11.2.0` — plugin oficial do Fastify, compatível com Fastify 5.
  Guarda os contadores em memória (suficiente para um único processo).

## Commands

```bash
docker compose up -d                                  # recria o Postgres com a porta nova
npm run typecheck
npm test
npm run db:generate -- --name <nome>                  # gera a migration a partir do schema.ts
npm run db:migrate                                    # aplica no banco de dev
npm install @fastify/rate-limit@^11.2.0
```

## Project Structure

```
src/app.ts                 # registra o @fastify/rate-limit (global: false)
src/db/schema.ts           # CHECK do username + coluna token_version
src/lib/token.ts           # payload { sub, ver }, sem role
src/routes/auth.ts         # regra do username, limites por rota, helper authenticate, POST /logout
drizzle/0001_*.sql         # CHECK do formato do username
drizzle/0002_*.sql         # coluna token_version
test/rate-limit.test.ts    # novo
test/logout.test.ts        # novo
test/{register,login,me}.test.ts, test/helpers.ts   # ajustados
docker-compose.yml         # porta só em 127.0.0.1
AGENTS.md                  # decisões, roteiro e estrutura atualizados
```

## Code Style

O mesmo do código atual. A autenticação vira uma função simples reaproveitada por `/me` e `/logout`
(dois usos não justificam um decorator/hook do Fastify ainda):

```ts
// Devolve o usuário dono do token, ou null se o token for inválido, estiver revogado ou a conta não existir.
async function authenticate(request: FastifyRequest, db: Db) { ... }
```

## Testing Strategy

- Vitest + `app.inject()` contra o banco `_test`, como hoje. TDD: cada tarefa começa com o teste falhando.
- Os testes existentes usam um app **sem** rate limit (`buildApp(db, { rateLimit: false })`),
  senão fariam mais de 5 logins por minuto. O `test/rate-limit.test.ts` usa o app com os limites
  **reais** de produção, provando a configuração que vai para o ar.
- Tokens montados à mão nos testes passam a incluir `ver`.
- Toda tarefa termina com `npm run typecheck`, `npm test` e um teste com `curl` no servidor real.

## Boundaries

- **Sempre**: teste falhando antes do código; migrations novas (nunca editar a `0000`); revisar o SQL
  gerado antes de aplicar; `{ "error": ... }` em todos os erros.
- **Perguntar antes**: commits (e reescrever a mensagem do commit `d6f59f6`); qualquer dependência além
  do `@fastify/rate-limit`; apagar dados do banco de dev.
- **Nunca**: logar token, senha ou hash; `role` vinda da requisição; desligar o rate limit fora dos testes.

## Success Criteria

- [x] Todos os critérios de aceite acima têm teste automatizado passando.
- [x] `npm run typecheck` e `npm test` verdes.
- [x] `curl` no servidor real: 6º login em 1 min → 429; logout → token antigo 401; `Joao` duplica `joao`.
- [x] `docker compose port db 5432` mostra `127.0.0.1:5432`.
- [x] Migrations aplicadas no banco de dev sem erro.
- [x] AGENTS.md atualizado (decisões, estrutura, roteiro).

## Premissas

1. O servidor roda **um único processo** e **sem proxy reverso** na frente. Com proxy, é preciso
   configurar `trustProxy` no Fastify, senão todo mundo compartilha o IP do proxy e o limite
   bloqueia todos juntos. Fica registrado no AGENTS.md.
2. O username `joão` que já existe no banco de **dev** é renomeado para `joao` (ou `joao_2`, se `joao`
   já existir) antes de aplicar a migration, senão o `CHECK` falha. O banco de testes é apagado a cada execução.
3. O logout invalida **todos** os dispositivos (escolha do dono: `token_version`).
4. Quando existir troca de senha, ela também deve incrementar o `token_version` (fora deste escopo).

## Riscos que continuam (fora do escopo)

- Rate limit por IP não segura um ataque distribuído (muitos IPs) contra o consumo de memória do argon2.
  Mitigação futura: limitar quantos hashes rodam ao mesmo tempo, ou um WAF/limite na infraestrutura.
- Contadores em memória zeram quando o servidor reinicia e não funcionam com vários processos
  (nesse caso, usar o store Redis do plugin).
