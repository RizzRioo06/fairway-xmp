# AI handoff

## Mac sharing package repair (2026-11-25)

- Owner's sharing run `45643736390` on `eeda0b3` produced the Windows installer,
  but Mac packaging failed strict codesign verification with an invalid symlink
  destination. No usable Mac artifact or native GUI acceptance is claimed.
- The sharing packager now validates self-contained app/runtime trees before
  copying, materializes internal aliases into regular files, and rejects external,
  broken, cyclic and special-file inputs before copying. Strict verification and
  PostgreSQL's existing pin/signature checks remain enabled; pinned PostgreSQL is
  never re-signed. Publication still refuses aliases and existing destinations.
- A disposable native codesign fixture reproduces the old failure and verifies
  the repaired bundle after moving it. Four copy/native fixtures and the existing
  signing/PostgreSQL integrity tests passed locally. Full sharing CI remains the
  required evidence for the actual app package.
- Work stays on `feature/ci-check-fixes`. Preserve the unrelated uncommitted
  `src/data/ColumnFields.tsx` edit; do not include it in packaging commits.

## CI repair in progress (2026-11-25)

- Current work is on `feature/ci-check-fixes`; personal fork `main` and teacher
  upstream are unchanged. The owner requested fixing the failing CI checks.
- Official archived Windows PHP ZIPs match all five existing SHA256 pins,
  including the upgrade fixture. Only their URLs changed; versions/hashes remain.
- PostgreSQL source extraction validates POSIX TAR names independently of the host
  and rejects Windows rooted/drive/backslash paths before any extraction.
- Updated the browser sidebar assertion for PostgreSQL, fixed Windows-only strict
  Clippy findings without allowing warnings, and refreshed the Linux builder's
  libssh package with an early exact-source availability check.
- First hosted run: `45628569079` on `f1c570a`. macOS verification passed;
  Windows Python/Rust tests passed but Clippy exposed three findings fixed in
  `4c57a90`. Windows runtime/upgrade staging and Mac corresponding-source packaging
  passed their former PHP 404 failures. Browser review reached case 508/556 before
  the 15-minute job timeout; `f43bd80` raises that budget to 25 minutes.
- Local evidence: 103 Python regressions passed before adding a further passing
  valid-source extraction test; all three focused source-safety tests, 32 TS unit
  tests, four focused Chromium/WebKit cases, frontend build, 177 Rust regressions
  (nine opt-in tests ignored), strict headless Clippy, formatting, YAML parsing and
  whitespace checks passed. Sandbox socket/process denials were resolved by
  rerunning the isolated fixtures with those permissions. No GUI acceptance run.
- After connectivity recovered, the first run confirmed both Mac jobs passed.
  Linux runtime preparation also passed; its strict Clippy check exposed an
  unused function on unsupported platforms. Final hosted verification of the
  follow-up fixes is pending. Run `build.yml` on this branch with
  `warm_inputs=true`; fix newly exposed failures before claiming all green.
  No merge into `main` is authorized by this repair request.

## Free Mac/Windows sharing (2026-11-25)

- Owner wants friends on Apple Silicon Macs and Windows to use the fork without
  paid Apple membership. Work is isolated on `feature/free-sharing-builds`.
- Added `free-sharing-builds.yml`: unsigned Windows installer reuse and an ad-hoc
  Mac app with manual updates, corresponding sources/checksums and PostgreSQL.
  There are no Apple/updater secrets or release-publication steps in this path.
- Mac uses native certificate trust and the existing administrator-password hosts
  fallback, not the Developer ID hosts helper. Real user Gatekeeper/Keychain and
  domain/HTTPS acceptance remain unverified.
- `configure_free_sharing.py` verifies an ad-hoc PostgreSQL payload and generates
  `artifacts/free-sharing/runtime-pin.json`. Only an explicitly marked Mac sharing
  build embeds this compile-time pin from OUT_DIR; standard/release builds retain
  the committed pin. No runtime environment variable can repin a running app.
- `package_macos_sharing.py` seals a fresh owned app copy without re-signing
  PostgreSQL, includes notices/source archives and records actual build identity.
  Signed candidate recording rejects the free-sharing distribution kind.
- Owner explicitly approved committing/pushing `feature/free-sharing-builds`,
  merging it into the personal fork's `main`, and running both free CI builds.
  Teacher upstream remains untouched. See Git history/Actions for final identities.
- Local checks passed: 101 Python tests at the regression milestone plus the final
  focused sharing/source tests, 177 Rust regressions, all 25 PostgreSQL tests in
  free-sharing mode, strict Clippy, formatting, YAML parsing and documentation links.
- See [manual sharing guide](free-sharing.md). No installer result is claimed
  before CI completes; native Mac recipient approval remains operator work.

## Owner-authorized main integration (2026-11-25)

- Owner explicitly requested committing the prepared code, merging it into the
  personal fork's `main`, and pushing to `origin`. This supersedes the earlier
  instruction to leave `main` unchanged for this integration only.
- Scope is the current PostgreSQL workspace and release preparation on
  `feature/postgresql-integration`. The separate Node templates branch remains
  separate. Check Git history and remote status for the resulting commit/merge.
- Publication is still pending owner credentials, a reviewed signed PostgreSQL pin
  and successful candidate CI. Merging/pushing source does not publish a release.

## Release preparation (2026-11-25)

- User requested finishing release prerequisites. Candidate is `0.2.0-rc.1`;
  updater/publication/help links now target `RizzRioo06/fairway-xmp`.
- Owner confirmed no release credentials are configured. The inherited public
  updater key and development PostgreSQL pin are deliberately retained; preflight
  blocks signed release until reviewed owner inputs replace them.
- Added separate PostgreSQL build/sign workflow, signed artifact validation/staging
  before compilation, preserved pin during app signing, configurable Apple
  identities, source redistribution and final signed-app worker CI fixtures.
- Bundle source reads allow root-owned non-writable installer resources; managed
  storage rules remain private. Unsupported platforms show an unavailable workspace.
- [Fork guide](fork-release.md) lists features and remaining owner/CI work. Notes
  are an unpublished draft. Initial preparation did not commit, merge or push;
  source integration is now authorized above. Release signing, Keychain changes
  and publication remain unperformed.
- Local checks: frontend build/32 TS tests; 177 Rust regressions; all 25 PostgreSQL
  tests including native disposable fixtures against a copied public-mode payload;
  24 simulated Chromium/WebKit checks; Python regression/release fixtures; strict
  all-target Clippy (debug resource-copy override), formatting and doc links passed.
  The fork guide records skipped checks, initial sandbox/resource-copy failures
  and the still-unverified signed/platform behavior.

## Existing PostgreSQL workspace

- Personal fork `RizzRioo06/fairway-xmp`; branch `feature/postgresql-integration`.
  Base main `dbc03ff`; Stage 2 `3af0075`. Use `git log -1` for latest commit.
- **PostgreSQL workspace is included in the prepared source integration.**
  Native external PostgreSQL 17.x compatibility and a rebuilt private 18.6 client
  payload are prepared. Real Summit 17.12 access to `vector_one.app_user` succeeded
  (8 columns/2 rows, values withheld). Simulated UI authentication is not evidence
  of real access. [Current acceptance/evidence](postgresql-native-acceptance.md)
  contains isolated native GUI commands; the user performs Mac acceptance.
- Stage 3 explorer/SQL backend:
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
- Qualified backend fixtures: macOS arm64/client-server 18.6 and real external
  Summit server 17.12 using client 18.6, loopback only. Remote
  verify-full fails closed: pinned client has no TLS. Other platforms/versions remain
  unverified. Disposable-cluster tests use memory vaults, never the login Keychain.
- **Stage 2 signed packaging and real signed-app Keychain acceptance remain blocked.**
  No real managed-cluster release acceptance is claimed. Preserve integrity/leases,
  journal/cluster ownership, Keychain-only managed credentials and smart shutdown.
  Never adopt/delete/upgrade clusters or control external services. Interrupted
  stages/orphan servers still require manual review; generic data removal refuses PG.

The current request authorizes finishing the existing workspace/native connection;
preserve explicit SQL consent, cancellation, truncation and uncertain-commit
handling. No role/backup UI or unrelated work is authorized. Upstream remains unchanged;
`feature/node-project-templates` stays at `3f81e1e`, awaiting manual acceptance/merge.
