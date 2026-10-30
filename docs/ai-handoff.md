# AI handoff

- Personal fork `RizzRioo06/fairway-xmp`; branch `feature/postgresql-integration`.
  Base `origin/main`: `dbc03ff`; Stage 1: `4af4d1e`. Use `git log -1` for latest.
- **Stage 2 implemented; signed runtime/Keychain acceptance remains outstanding.**
  Pinned official PostgreSQL 18.6 macOS arm64 source recipe, verified private runtime
  provisioning, Keychain-only production secrets, explicit SCRAM/loopback initdb,
  owned foreground lifecycle, smart-only shutdown, separate responsiveness/auth,
  bounded retained logs and typed managed-only IPC. No auto-start/service UI.
- Modules: `src-tauri/src/postgresql/managed/`, `managed_contract.rs`,
  `desktop/postgresql_managed_commands.rs`, `src/postgresql/`.
- [Architecture](postgresql-integration.md), [runtime/pin/release blockers](postgresql-runtime.md),
  [verification and manual acceptance](postgresql-stage2-verification.md).
- Real disposable-cluster tests use a memory fixture vault, **not** the login Keychain.
  Headless native Keychain smoke is compile-checked only; run manually in a disposable
  account. Final signed payload must be pinned after signing; current pin is the
  qualified ad-hoc development payload. Existing release workflows omit it.
- Never adopt/delete/overwrite/upgrade a cluster, signal external installations or
  bypass unavailable vault/integrity checks. Failed stages/journals/PID markers are
  retained. Active clients can delay smart stop; orphan recovery is manual review.
  Generic `--remove-data` refuses any PostgreSQL tree/link. MySQL/PHP/Node remain separate.

`main` and upstream remain unchanged. `feature/node-project-templates` stays at
`3f81e1e`, awaiting manual acceptance/merge. Do not merge, distribute this unqualified
signed-runtime path, or start Stage 3/explorer/SQL/roles/backups without a new request.
