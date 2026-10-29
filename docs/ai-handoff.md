# AI handoff

- Repository: `/Users/rioo/projects/fairway-xmp-personal`, personal fork
  `RizzRioo06/fairway-xmp`; active branch `feature/postgresql-integration`.
- Base: verified `origin/main` at `dbc03ff`. Use `git log -1` for this stage's commit.
- **Phase 2 PostgreSQL Stage 1 is complete; Stage 2 has not started.** read-only tool discovery, explicit native
  `--version` inspection, typed Rust/Tauri/TypeScript IPC and architecture plan.
  No PostgreSQL server initialization/start, connection, profile/password storage,
  service controls, explorer or database mutation is implemented.
- Backend: `src-tauri/src/postgresql.rs`, `postgresql/`,
  `desktop/postgresql_commands.rs`; frontend contracts: `src/postgresql/`.
- [Architecture and later-stage contracts](postgresql-integration.md),
  [verification/limitations](postgresql-stage1-verification.md).

PostgreSQL is independent of MySQL/PHP/Node. Discovered tools and matching versions
never grant cluster/process ownership or prove readiness. External connections
must stay separate from managed lifecycle. Future passwords require an OS vault
or memory-only use; no plaintext persistence or credentials in diagnostics.

`feature/node-project-templates` remains unchanged at `3f81e1e`, awaiting the user's
final manual acceptance/merge. It is not included in this branch. The existing
Node Projects integration on main remains. Project instructions in `AGENTS.md`
were carried forward unchanged because main did not yet contain that file.

Next: review/authorize Stage 2 runtime packaging, vault boundary and explicit safe
managed-cluster lifecycle using disposable fixtures. Do not begin Stage 2, merge
main, alter other feature branches or push upstream without a new request.
