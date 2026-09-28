# Implementation Plan: Endurecimento da autenticação

Spec: [SPEC-auth-hardening.md](../SPEC-auth-hardening.md) (aprovada). Tarefas em [todo.md](todo.md).

## Overview

Fechar os riscos abertos da revisão de auth: username sem ambiguidade, JWT sem `role` e com versão
revogável, logout, rate limit em login/cadastro e Postgres fechado para a rede. TDD em cada tarefa,
um commit por tarefa verde.

## Grafo de dependências

```
T0 commit da revisão (base limpa)
 ├── T1 docker-compose 127.0.0.1                    (independente)
 ├── T2 regra do username ── migration 0001
 │    └── T3 token { sub, ver } + token_version ── migration 0002
 │         └── T4 POST /auth/logout (usa token_version)
 │              └── T5 rate limit (mexe no helper de testes que todos usam)
 └── T6 AGENTS.md (depois de tudo, descreve o estado final)
```

T2 vem antes de T3 só para as migrations saírem em ordem (0001, 0002) e cada uma ser revisada sozinha.

## Architecture Decisions

- **Username normalizado para minúsculas + `CHECK` no banco** em vez de índice único em `lower(username)`:
  com o `CHECK '^[a-z0-9_]{3,50}$'`, o `UNIQUE` que já existe passa a ser case-insensitive de graça,
  e o valor salvo é o mesmo que o usuário vê.
- **Versão do token no próprio usuário (`token_version integer default 0`)**: o `/me` já consulta o
  banco, então checar a versão custa só um `AND token_version = $2` na mesma consulta.
  O logout é um `UPDATE ... SET token_version = token_version + 1`.
- **`authenticate(request, db)` como função** (não decorator/hook): dois usos (`/me`, `/logout`) não
  justificam a abstração. Vira `preHandler` quando surgir a terceira rota protegida.
- **Rate limit com `global: false` + `config.rateLimit` por rota**: só login e cadastro são limitados.
  O erro 429 passa pelo error handler central (o plugin lança um erro com `statusCode: 429`), então
  o `errorResponseBuilder` só troca a mensagem para português e o formato `{ error }` é mantido.
- **`buildApp(db, { rateLimit: false })` só nos testes**: o padrão é ligado; `server.ts` não passa a opção.

## Risks and Mitigations

| Risco | Impacto | Mitigação |
|---|---|---|
| `CHECK` falha no banco de dev por causa do `joão` | Médio | Renomear o registro antes de `db:migrate` (premissa 2 da spec) |
| `drizzle-kit` gerar SQL diferente do esperado para `check`/coluna nova | Médio | Revisar o SQL antes de aplicar; nunca editar a 0000 |
| Rate limit não pegar nas rotas por ordem de registro do plugin | Alto | O teste de rate limit usa o app real e falharia; conferir também com `curl` |
| Mudanças de contrato quebrarem o front | Médio | Tabela de contrato na spec + resumo final para repassar ao amigo |
| `docker compose up -d` recriar o container | Baixo | O volume `pgdata` mantém os dados; só a porta muda |

## Open Questions

Nenhuma: decisões fechadas na spec.
