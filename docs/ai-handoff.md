# AI handoff

## Windows PostgreSQL and signing request (2026-11-27)

- Owner requested trusted Windows installer signing and implementing PostgreSQL
  on Windows after the explanation. They confirmed **no signing credentials**.
  Existing signed_release is updater-only; public Authenticode signing is blocked
  pending an external certificate/service identity. See the new signing setup
  section in [Build and release](build-and-release.md#windows-authenticode-setup).
- Work is isolated on `feature/postgresql-windows` from `fd2ec18`. Preserve the
  uncommitted launcher, metadata and documentation follow-ups from the prior branch.
  Owner subsequently approved committing/pushing this feature branch and running
  the Windows-only installer workflow. No main merge or Mac workflow is authorized
  by that follow-up. Use Git history/Actions for final commit and run identities.
- Implemented the external-loopback Windows path: pinned PHP-archive libpq DLLs,
  build-embedded hashes, native dependency loading, owned Job Object workers,
  private bounded IPC and the existing pg_catalog/query/cancellation contracts.
  Windows UI now offers external connections only. Managed Windows servers and
  Linux browsing are not enabled. Mac runtime/pins and service ownership remain.
- Added a Windows installer CI step using a disposable SCRAM server/native worker
  fixture, with no personal databases or real Credential Manager mutations. That
  Windows fixture and actual Windows compilation have **not** run on this Mac.
  Updated the Windows UI fixture but did not run browser/E2E tests.
- Local evidence: frontend build/47 TS tests passed; 36 PostgreSQL Rust tests passed
  (two existing opt-in native fixtures ignored); Windows fixture contract host
  type-check test passed; three Windows archive tests and 12 runtime packaging
  regressions passed; strict desktop all-target Clippy and formatting passed.
  Initial Clippy unit-expression findings were corrected. Actual pinned Windows
  DLL APIs and native worker/server behavior still require the Windows CI run.


## Original saved connection located (2026-11-27)

- Owner saw no saved connections after switching to the new desktop shortcut.
  Read-only metadata inspection found `Vector_One_Summit` still saved under
  `.runtime-work/pg-ui-SJ5PeI/data/postgresql-connections/profiles.json`.
  The new `.runtime-work/desktop-dev` profile has no saved connection metadata.
  This does not establish database/server state; no database or password was read.
- Added `npm run dev:desktop -- --profile pg-ui-SJ5PeI` to reopen that existing
  profile and its original acceptance app identifier without moving/copying data.
  Selection rejects aliases, foreign directories and path traversal. User launch
  remains manual; no UI, Keychain, service or database operation was run here.


## One-command native development (2026-11-27)

- Owner requested a package.json shortcut instead of repeating long launch commands.
  Added `npm run dev:desktop`, using local Tauri/Vite, Cargo PATH only in the child
  environment, a separate app identifier and `.runtime-work/desktop-dev/{home,data}`.
- The owned marked profile is reused so test connections survive relaunches.
  Unsafe/aliased/unmarked existing paths are refused without overwriting them.
  Older temporary acceptance profiles are not imported or deleted.
- `-- --check` prepares/validates without starting an app or any services. Launcher
  tests use disposable directories, never installed Fairway or personal settings.
- Three focused fixtures, Node syntax, TypeScript and whitespace checks passed;
  no app launch, build or UI/E2E test ran. Changes remain local on the feature branch.


## Fork metadata cleanup (2026-11-27)

- Owner reports the UI improvements were merged into personal fork `main` and
  Mac/Windows sharing builds are running there. Node templates remain separate.
- On `feature/postgresql-connection-profiles`, corrected Cargo's repository URL
  to `RizzRioo06/fairway-xmp` and removed the identical homepage field reported
  by CI's `cargo::redundant_homepage` warning. License/notices are unchanged.
- This local follow-up is not committed/pushed or included in the running build.
  No application rebuild or UI/E2E test is needed for this metadata-only change.

## SQL editor layout correction (2026-11-27)

- Owner's manual screenshot showed the SQL textarea at its default narrow width
  while the note and mode controls stretched across the panel.
- The PostgreSQL editor now has an explicit full-width, border-box textarea with
  readable monospace text, vertical resizing and contained horizontal scrolling.
  The query heading, target database, mode badge and action toolbar are grouped;
  long limits/safety notes are expandable and results have their own heading.
- Read-only defaults, SQL size/statement limits, write-confirmation dialog,
  cancellation and outcome behavior are unchanged. No new editor dependency.
- Production frontend build and all 43 TypeScript unit tests passed. No UI/E2E
  automation or real database operation ran; visual acceptance is still owner work.


## Browse Data clarity and PostgreSQL table polish (2026-11-27)

- Owner explicitly requested removing the PostgreSQL sidebar entry. PostgreSQL
  is now accessed only through Browse Data → PostgreSQL; the duplicate route and
  shortcut request state have been removed.
- The tab adds a local-server introduction, schema guidance and browsing notes
  explaining read-only rows, authorized SQL changes and existing-server ownership.
  Filter operators have readable labels; pagination shows actual row ranges.
- Results use numbered rows, sticky column/type headers and row numbers, zebra
  striping, hover/focus highlighting, clearer NULL/empty badges and a wrap toggle.
  Long Unicode previews are bounded, with the exact value preserved in the viewer.
  Outcome summaries retain committed/not-started/uncommitted/unknown distinctions.
- Frontend production build and 43 TS unit tests passed; no UI/E2E tests or real
  database checks ran. Existing UI fixtures were updated but not executed. Visual
  layout remains for owner Mac acceptance. Existing ColumnFields edit preserved.


## PostgreSQL in Browse Data (2026-11-27)

- Owner requested PostgreSQL beside MySQL/SQLite in the normal Browse Data screen.
- `data/BrowseData.tsx` hosts all three source tabs. The embedded PostgreSQL
  tab/controller preserves saved
  connections, sessions, table/schema browsing and SQL safeguards.
- MySQL/SQLite keep their existing Activity visibility cleanup. PostgreSQL is
  retained outside that Activity so source navigation does not dispose its
  controller. Returning to a SQL source waits for any old request to settle and
  uses its existing source-switch path. No cross-engine SQL/backend reuse.
- Production build and 40 TypeScript unit tests passed. No browser/E2E/UI automation
  or real database checks ran. Manual Mac source switching/layout remains unverified.
  The existing ColumnFields edit is preserved; no commit/push/release performed.


## Missing-client connection recovery (2026-11-27)

- Owner's manual Mac screenshots showed a saved profile and “Explicit runtime
  provisioning is required” in a fresh isolated app profile. This is a missing
  client runtime, before a database connection attempt, not an authentication result.
- The form and workspace now explain the missing connection tools and expose
  **Prepare connection tools** directly. That explicit action calls only runtime
  provisioning; it does not initialize/start a cluster or connect automatically.
  Form metadata stays in place. Successful provisioning clears only the matching
  missing-runtime error; unrelated SQL errors/outcome uncertainty are preserved.
- Production frontend build and 13 focused workspace/guidance tests passed.
  No UI/E2E automation or real database/runtime/Keychain operation was performed.
  User should retry Connect after preparing tools; actual database access is unverified.


## Cross-platform saved-password providers (2026-11-27)

- Owner explicitly selected **secure password storage first**, rather than full
  Windows/Linux PostgreSQL browsing/runtime support. Continue on
  `feature/postgresql-connection-profiles`; preserve the pre-existing data edit.
- Profiles/metadata IPC now selects Keychain, Windows Credential Manager or Linux
  direct libsecret Secret Service. Mac namespace and metadata format are retained.
  No new dependency/runtime installation, real credential access or database work.
- Windows metadata uses pinned ancestor handles, rejects reparse/hard links and
  requires a protected current-user-only DACL. Linux uses descriptor-relative
  private Unix storage. Both retain the existing credential recovery journal.
- Linux requires system libsecret plus a Secret Service keyring; missing/locked/
  cancelled operations fail without plaintext or a file-backend fallback. Native
  cancellation is requested after 30 seconds. Windows blobs are wiped before free.
- UI labels select the vault name per platform. The existing non-Mac workspace
  restriction remains in place until a separately scoped native runtime port.
- Local evidence: production frontend build and 37 TS unit tests passed; 11 focused
  profile/provider/Unix-storage tests passed using scratch data and injected APIs.
  Strict desktop all-target Clippy passed. Windows storage had a host-only Rust
  type check; this is not a Windows build or OS credential acceptance result.
- A no-feature test attempt exposed an existing `shell_environment` test that
  requires the terminal feature. Focused checks/CI use `--features terminal`;
  that unrelated source was not changed.
- Added `credential-storage.yml` for Windows/Linux/Mac fixture checks without UI
  or real account vault mutations; not dispatched. Windows/Linux builds and real
  vault sessions remain unverified. No UI/E2E tests, commit, push or release.
- [Platform contracts and limits](postgresql-integration.md#saved-connections).

## PostgreSQL saved connections and UI improvement (2026-11-27)

- Owner says the app is released/usable and requested saved external connections,
  optional remembered passwords, native database switching and focused UI polish.
  Work is on `feature/postgresql-connection-profiles`, based on `fc919ce`.
  Preserve the pre-existing `src/data/ColumnFields.tsx` edit unchanged.
- Native profiles store only bounded nonsecret metadata under the app data folder.
  Optional passwords use a separate Keychain namespace, never plaintext files.
  Revision checks, private descriptor-relative storage, interprocess locking and
  a durable credential cleanup journal protect edits and interrupted operations.
- Existing native sessions now supply credentials when switching databases.
  Reconnect refreshes the current descriptor; failed initial authentication/catalog
  attempts release their descriptors and keep form metadata for retry.
- UI adds saved connection cards, edit/rename/remove, optional password retention,
  connection status, collapsed managed controls, remembered source and table search.
  [Contracts and limits](postgresql-integration.md#saved-connections).
- Verification: frontend production build and 36 TypeScript unit tests passed;
  desktop cargo check and strict all-target desktop Clippy passed using the existing
  debug resource-copy override. Headless Rust regression: 183 passed, nine opt-in
  fixtures ignored after rerunning sandbox-denied socket/process tests with access.
  The final eight profile fixtures passed, including three added recovery/validation
  cases after that regression run. Native vault fixtures use memory only.
- No GUI, browser or end-to-end tests ran, and no real Keychain or existing database
  was accessed. Existing UI test selectors were updated but remain unrun. User Mac
  acceptance is still needed for visual layout, real Keychain and packaged relaunch.
  No commit, push, merge, installer build or release publication was performed.


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
