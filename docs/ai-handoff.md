# AI handoff

- Personal fork `RizzRioo06/fairway-xmp`; branch `feature/postgresql-integration`.
  Base main `dbc03ff`; Stage 2 `3af0075`. Use `git log -1` for latest commit.
- **Stage 3 explorer/SQL backend implemented; no graphical workspace yet.**
  Native pg_catalog inspection, schema-aware paginated/sorted/filtered rows, exact
  typed text/NULL results, single-statement SQL with read-only default and explicit
  write consent, bounded private libpq workers and connection-bound cancellation.
  Activity tests observe the server query active before cancellation/owner loss and
  absent afterward. Resolver contention preserves pre-dispatch `busy`.
- Backend: `src-tauri/src/postgresql/explorer/` and `explorer_contract.rs`;
  IPC: `desktop/postgresql_explorer_commands.rs`; TS: `src/postgresql/explorer*.ts`.
  Explicit memory-only managed/external sessions; opening a descriptor reports auth
  not checked. External sessions never own servers. No history/password persistence.
- [Architecture/contracts](postgresql-integration.md),
  [Stage 3 evidence/limits](postgresql-stage3-verification.md),
  [runtime/pin/release blockers](postgresql-runtime.md).
- Qualified backend fixtures: macOS arm64/client-server 18.6, loopback only. Remote
  verify-full fails closed: pinned client has no TLS. Other platforms/versions remain
  unverified. Disposable-cluster tests use memory vaults, never the login Keychain.
- **Stage 2 signed packaging and real signed-app Keychain acceptance remain blocked.**
  No real managed-cluster release acceptance is claimed. Preserve integrity/leases,
  journal/cluster ownership, Keychain-only managed credentials and smart shutdown.
  Never adopt/delete/upgrade clusters or control external services. Interrupted
  stages/orphan servers still require manual review; generic data removal refuses PG.

Stage 4 can bind these contracts to UI after a new request; preserve explicit SQL
consent, cancellation, truncation and uncertain-commit handling. No Stage 4,
role/backup UI or unrelated work has begun. `main`/upstream remain unchanged;
`feature/node-project-templates` stays at `3f81e1e`, awaiting manual acceptance/merge.
