# Capability Map: Fechar a API e migrar para Rust

Aprovado pelo dono em 2026-10-05.

| Módulo | Responsabilidade | Depende de | Spec |
|---|---|---|---|
| `endpoints-restantes` | Completar a API em TypeScript (troca de senha) | — | [SPEC-endpoints-restantes.md](SPEC-endpoints-restantes.md) |
| `migracao-rust` | Reescrever a API em Rust com o mesmo contrato HTTP, para estudo do dono | `endpoints-restantes` | [SPEC-migracao-rust.md](SPEC-migracao-rust.md) |

Ordem: `endpoints-restantes` → `migracao-rust`. Os dois estão concluídos; o TypeScript saiu do repositório depois
da migração (`tasks/plan.md`).
