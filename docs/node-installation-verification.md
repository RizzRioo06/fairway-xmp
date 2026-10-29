# Node template installation — Stage 3 verification

Work is confined to `feature/node-project-templates`, based on Stage 2
`9d55e6b7b2b2d89571ec96a0ee94ffbe2722a5e7`. Evidence was collected over
2026-10-28–29 on macOS arm64. No installed bundle, global Node default, personal
project, main branch or upstream repository is modified by this work.

## Implemented behavior

The separate creation service now supports explicitly authorized finite npm jobs.
It validates the existing immutable plan, generates only in an owned private stage,
revalidates the selected Node/npm pair, and invokes `npm ci --ignore-scripts`
using a clean child environment and private per-job home/cache/temp directories.
All six templates carry genuine npm-generated v3 lockfiles. Their exact graph,
resolved versions and SHA-512 integrity metadata are retained. Raw lock hashes,
generator versions and registry provenance are committed. Direct dependency pins,
scripts and supported runtime ranges are unchanged from Stage 2.

A job receives independent creation, installation and run identities. The existing
native supervisor owns the process group; only its live channel controls cleanup.
The installation service has separate status/log storage from ordinary Node runs.
Status, cancellation, cursor logs and explicit recovery have typed Rust/Tauri and
TypeScript contracts. Logs reuse existing bounded memory and rotating disk files.
The app quit/updater drains installers and protects runtimes from removal while used.
PHP service controls remain separate. No creation wizard or automatic app start exists.

## Cleanup race and regression coverage

An npm exit/failure notification does not prove the supervisor has finished
terminating descendants, draining output and reaping its helper. Reading dependency
files or releasing runtime ownership at that point can race with live children.
The finite worker waits for owned supervisor completion before accepting any exit
result or verifying/publishing files. Its wait has a 22-second cleanup deadline;
timeout retains ownership and reports failure. The job itself has a five-minute
execution ceiling. Worker-error and shutdown paths also drain the same owned run.

A second failure-path regression reproduced exit code 7 becoming missing after
the error handler reloaded its journal. The worker now carries the observed exit
code across that reload; failed npm status retains the actual code. A separate
late-staging regression reproduced a rejected start remaining labelled Starting.
Rejected preparation/worker starts now save a terminal Failed/Cancelled status;
the regression confirms that an occupied foreign stage and its sentinel stay intact.

The focused unit test holds a run active after simulated Exited/Failed events:
a short deadline returns an error while the runtime stays leased, and a waiter
unblocks only after simulated reaping completes. The native test has npm exit 0
while an ordinary child continues writing. It observes the child alive, an absent
destination, no dependency receipt/ciCompleted flag and an active runtime lease.
It then verifies graceful completion, Cancel, Quit and forced cleanup. Only the
fully cleaned successful case publishes; a required SIGKILL fails installation.

Additional native fixtures cover host SIGKILL/EOF cleanup, duplicate requests,
stale IDs/revisions, runtime uninstall protection (including workspace locks after
host death), missing/changed tools and locks, explicit consent, registry-save
failure/retry, preservation of published projects, source inode/content changes,
Unicode, flooding and rotation. A live installer also runs across PHP/MySQL
Start All/Stop All and the updater lifecycle reservation using disposable data.

## Dependency qualification

Public npm qualification produced genuine locks with Node 22.23.4/npm 10.9.9.
Clean installs with lifecycle scripts disabled succeeded for every variant; the
four build scripts passed. A React registry request initially timed out; a retry
succeeded without weakening TLS or integrity. No dependency versions were upgraded
after the locks were selected. Public-registry qualification is separate from routine tests.

The offline mirror serves only SHA-512-verified tarballs from those disposable
qualification caches, never an outbound proxy. All six variants passed real npm
ci, build/test where present, dev-server HTTP responses and owned shutdown on both
Node 22.23.4/npm 10.9.9 and Node 24.20.0/npm 11.19.0 (12 combinations). esbuild,
tsx/Vite and Rollup worked using the pinned macOS arm64 optional binaries with
install lifecycle scripts disabled. No script-policy exception was necessary.
Corrupt integrity and 503 fixtures failed without publication. The connection-refusal
fixture reserves a loopback TCP port without listening, so no remote network is
used. npm 10.9.9 emitted its generic “Exit handler never called!” diagnostic in this
case rather than ECONNREFUSED. Fairway records failure/exit 1, retains the stage
and publishes nothing. The npm diagnostic itself is not patched or upgraded here.

The test harness checks helper capabilities: core builds may replace an example
binary with a different feature set. Keep a mirror-enabled helper in a separate
private temporary directory before building with other Cargo features. An earlier
connection-refusal attempt correctly failed at this mismatched-helper guard; its
missing network error was a test setup failure, not evidence of a product network bug.

## Recovery and deliberate limitations

Installation is before exclusive publication, followed by the existing atomic
registry save. Failed/cancelled/interrupted partial stages are retained. Recovery
finalizes only a durably completed, intact installation or discards intact
source-only staging. It never reruns ci in partial node_modules or deletes a
published project. Retry requires a fresh reviewed plan/workspace and consent.
For an already published Stage 2 project, review a new destination for the same
template; this creates a separate project and preserves the original registration
and every original file. It is not an in-place migration or a copy of user edits.

App-owned homes/caches and uncertain dependency data are retained for inspection;
no recursive dependency cleanup or automatic retry/pruning is introduced. One
installation runs at a time. Native execution/generation remains macOS-first.
Windows/Linux execution, other CPU architectures, signed installers, real updater
replacement, native window interaction, forced five-minute wall-clock expiry,
power-loss simulation and deliberately detached processes remain unverified.
Isolation is not an OS sandbox against package code or a malicious same-user actor.

## Completed checks

| Check | Observed result |
| --- | --- |
| Rust desktop/library, locked, fixture feature | 195 passed; 19 opt-in tests ignored in this default run |
| Rust core/terminal, no default features | 184 passed; 17 opt-in tests ignored in that run |
| Native installer opt-ins, executed separately | 9 distinct opt-in tests passed, including cleanup race, crash, real npm/template matrix, network errors and PHP coexistence |
| Existing native Node execution opt-ins | 5 passed: concurrency, lifecycle, logs, crash cleanup and PHP independence |
| TypeScript unit/contract tests | 25 passed |
| Existing Python regression suite | 87 passed |
| New Python helper syntax checks | Passed using a disposable pycache |
| Headless Chromium/WebKit Node Projects, Node tools and workspace | 44 passed at normal desktop sizes |
| Formatting and diff whitespace | cargo fmt --check and git diff --check passed |
| Strict Clippy | all targets/all features, locked, -D warnings passed |
| Frontend production build | TypeScript noEmit and Vite build passed |
| macOS production executable | cargo build --release --locked --bin fairway passed; not installed/launched |
| CLI build | no-default-features cli build passed |
| Lock provenance/pins audit | Six raw lock hashes and counts match; Stage 2 direct pins/scripts/runtime ranges unchanged |
| Secret/generated-file audit | No high-signal credential patterns or unintended cache/build files in the commit |

Counts overlap; they must not be added as one total. The native installer matrix
contains 12 template/runtime combinations plus controlled integrity/503 failures;
connection refusal and active-installer/PHP coexistence are separate focused runs.
The exit-code reproducer failed before the fix (None instead of 7) and passed
afterward. The rejected-start reproducer likewise failed before the fix (Starting
instead of Failed) and passed afterward. All completed final checks pass; no known
product-test failure remains.

Four older opt-ins were not repeated: standalone real planning, standalone real
local generation, original two-runtime isolation and scratch login-profile timeout.
The affected six-template generation, selected-tool preparation and both Node
versions are covered by the new installer matrix; discovery/profile behavior was
not changed. The ignored generation_crash_child is an internal subprocess transport,
exercised by its ordinary parent regression rather than run standalone. Other
ignored native tests listed above were explicitly run with controlled fixtures.
No cross-platform build/installer, signing, live updater or graphical application
acceptance was attempted. No Computer Use or desktop automation was used.

Logs used while working are app-owned/disposable under /tmp: fw-stage3-rust-final,
terminal-final, installer-final, cleanup-race, unavailable-network, install-php,
project-regression, browser, python, clippy-final, release-build and cli-build
(with .log suffix). They are not committed artifacts; this report records their
observed outcomes. Public qualification caches also remain disposable fixtures.

## Files created

- `docs/node-installation-verification.md`
- `scripts/qualify_node_templates.py`
- `scripts/test_node_installations.py`
- `src-tauri/src/node_project_creation/installation.rs`
- `src-tauri/src/node_project_creation/installation_fixture.cjs`
- `src-tauri/src/node_project_creation/installation_native.rs`
- `src-tauri/src/node_project_creation/installation_tests.rs`
- `src-tauri/src/node_project_creation/locks.rs`
- `src-tauri/templates/node-projects/locks/express-js.json`
- `src-tauri/templates/node-projects/locks/hono-ts.json`
- `src-tauri/templates/node-projects/locks/node-js.json`
- `src-tauri/templates/node-projects/locks/node-ts.json`
- `src-tauri/templates/node-projects/locks/provenance.json`
- `src-tauri/templates/node-projects/locks/react-vite-js.json`
- `src-tauri/templates/node-projects/locks/react-vite-ts.json`

## Files modified

- `docs/node-project-creation.md`
- `src-tauri/Cargo.toml`
- `src-tauri/examples/node_projects_runner.rs`
- `src-tauri/src/desktop.rs`
- `src-tauri/src/desktop/node_project_creation_commands.rs`
- `src-tauri/src/node_project_creation.rs`
- `src-tauri/src/node_project_creation/assets.rs`
- `src-tauri/src/node_project_creation/catalog.json`
- `src-tauri/src/node_project_creation/filesystem.rs`
- `src-tauri/src/node_project_creation/generation.rs`
- `src-tauri/src/node_project_creation/generation_native.rs`
- `src-tauri/src/node_project_creation/generation_tests.rs`
- `src-tauri/src/node_project_creation/journal.rs`
- `src-tauri/src/node_project_creation/tests.rs`
- `src-tauri/src/node_projects.rs`
- `src-tauri/src/node_projects/execution.rs`
- `src-tauri/src/node_projects/isolation.rs`
- `src-tauri/src/node_projects/runner.rs`
- `src/node-project-creation/api.ts`
- `src/node-project-creation/types.ts`
- `tests/node-project-creation-contract.test.ts`
