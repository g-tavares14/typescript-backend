---
name: build
description: Lifecycle shortcut for the implementation lifecycle. Use only when the user explicitly invokes /build.
disable-model-invocation: true
---

# Build

Read and follow both `../incremental-implementation/SKILL.md` and `../test-driven-development/SKILL.md` in full.

Treat `/build` as an explicit request to implement the next pending task with the canonical test-driven, incremental loop. If the user supplies `auto` or `all`, require an approved specification and plan, establish a clean baseline, then execute all remaining tasks in dependency order with verification. Stop on ambiguity, failed verification, or an irreversible action that lacks authorization.
