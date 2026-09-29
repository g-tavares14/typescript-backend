---
name: verify
description: Lifecycle shortcut for checking completed work. Use only when the user explicitly invokes /verify.
disable-model-invocation: true
---

# Verify

Read and follow `../test-driven-development/SKILL.md` in full.

Treat `/verify` as an explicit request to check the current change against its acceptance criteria. Discover and run the repository's focused tests and required verification commands, inspect the results, and report failures with evidence. Do not claim checks passed unless they ran. If a required test is missing, identify the gap and propose or add a focused test as appropriate to the user's request.
