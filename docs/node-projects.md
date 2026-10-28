# Node Projects — registry, runtime isolation and owned execution

This personal extension provides registry, inspection, runtime preparation and
owned foreground npm-script execution on macOS. Stage 4 adds the **Node Projects**
workspace in the Tools navigation group. PHP sites, Start All/Stop All, ports,
hosts and certificates retain their existing behavior. Node Settings refuses to
remove or replace an installation while a managed Node project owns it. Other platforms report `supported: false`; mutations
and inspection return a typed `unsupportedPlatform` error.

## Inspection and runtimes

`node_project_inspect` reads the selected package root directly. It does not run
`npm`, install dependencies, execute project scripts, or evaluate project files.
`package.json` must be a regular UTF-8 JSON object of at most 1 MiB, with at most
256 string-valued scripts (128-character names and 16 KiB commands). Optional
`.node-version` and `.nvmrc` files are limited to 4 KiB each. Symlinked and special
inspection files are rejected; symlinked project folders resolve to their actual
root. Missing and malformed files produce actionable errors.

The response includes scripts, `packageManager`, lockfile hints, `engines.node`,
version-file contents, an inspection revision, warnings, and installed runtime
choices. Lockfile contents and dependency directories are not read. Conflicting
package managers are reported. Registration is possible for Yarn/pnpm/Bun
projects, but their launchers remain outside the planned npm-only first launcher.

Runtime discovery reuses `node_tools::status` and its fnm/nvm/Homebrew inventory,
including external read-only stores. Existing discovery executes bounded Node
probes and, on macOS, the user's login-shell startup files. It never executes the
imported package scripts. This extension does not call fnm install/use/default,
claim a store, change activation markers, or edit shell profiles.

Exact matching version files can suggest an installed runtime. Conflicting
versions, ranges, aliases, and `engines.node` do not silently select a compatible
version: the caller must choose a runtime explicitly. Only a freshly discovered
runtime ID is accepted on save; requests cannot submit an executable path.

## Registry and IPC

Registrations live separately in `<Layout.data>/node-projects.json`:

```json
{
  "schemaVersion": 1,
  "projects": [
    {
      "id": "0123456789abcdef0123456789abcdef",
      "name": "Example",
      "root": "/Users/example/code/example",
      "script": "dev",
      "runtime": { "nodePath": "/absolute/path/to/node", "version": "v22.1.0" },
      "openUrl": "http://localhost:3000/"
    }
  ]
}
```

The registry is limited to 1 MiB and 64 registrations. IDs remain stable across
edits. Canonical roots must be unique, including aliases of the same directory.
Missing project folders and runtime files remain registered and can be removed.
Snapshot availability flags describe file availability; launch-time version and
process validation is repeated immediately before execution. Nothing auto-starts
or stores a running PID as authority.

Writes use private, exclusively created same-directory staging files, flush the
file, atomically replace the registry, and sync the parent directory. Before
publication, failures leave the previous file intact and remove the partial
file. A sync failure after replacement explicitly reports that publication
already happened and asks the caller to reload. Corrupt, oversized, and future
registry versions are preserved and never replaced with an empty list. Their
errors are confined to the Node registry and do not block PHP startup.

Commands use a typed `request` argument, camelCase DTOs, and errors shaped as
`{ code, message }`. The contracts and wrappers are in
`src/node-projects/types.ts` and `src/node-projects/api.ts`.

| Command | Request | Result |
| --- | --- | --- |
| `node_projects_snapshot` | None | Support flag, registry revision/error, registrations and availability |
| `node_project_inspect` | `{ root }` | Package metadata, manifest revision and runtime choices |
| `node_project_save` | `{ config, expectedRevision, manifestRevision }` | `{ revision, project }` |
| `node_project_remove` | `{ id, expectedRevision }` | `{ revision }` |
| `node_project_prepare_runtime` | `{ id, expectedRevision, manifestRevision }` | Validated Node/npm metadata and a validation receipt; no environment values |

Save config fields are `id` (null for import), `name`, `root`, `script`,
`runtimeId`, and nullable `openUrl`. Optional URLs must use HTTP/HTTPS and a
literal `localhost`, `127.0.0.1`, or `[::1]` host without credentials. Stage 1
validates and saves them but never opens them.

Fetch a snapshot for `expectedRevision` and inspect the root for
`manifestRevision` and a runtime ID before saving. Editing uses the existing ID;
removing never deletes project files. A `conflict` error requires refreshing
rather than retrying an old request. Revisions are opaque content hashes, not
ordered counters. Save/remove emit `node-projects-changed { revision }` as an
invalidation event; subscribe before loading and serialize subsequent refreshes.
Execution commands added in Stage 3 are documented below.

## Stage 2: runtime isolation

A registration still stores the same explicit Node path and exact version. No
registry migration is necessary. `prepare_runtime` loads the saved registration,
checks its revision and the package inspection revision, and validates its
selected tools without executing the package's scripts. It returns an in-memory
`PreparedRuntime`; IPC exposes only its `RuntimePreparation` summary.

**npm pairing:** resolve the selected `bin/node` installation's own `bin/npm`
link, or the standard `lib/node_modules/npm` / Homebrew `libexec/lib/node_modules/npm`
layout. An explicit npm link may identify a shared Homebrew package. Ambiguous,
broken, or project-contained tool paths are rejected. Another npm found on PATH
is never substituted. npm's bounded manifest must identify the standard CLI,
version, and Node requirement. Its bundled semver dependency validates version
ranges. Two bounded, eight-second tool probes run a fixed validation program and
`npm --version`, in the npm installation directory with the cleaned environment.
Neither command evaluates an imported script. Node path/version and npm version
must match. Content fingerprints and file identities detect changes during
preparation; a changed link or manifest is also rejected.

**Requirements:** the saved exact version must satisfy every `.node-version`,
`.nvmrc`, and `engines.node` constraint, and the selected npm must satisfy an
explicit `packageManager: npm@VERSION` declaration. Exact versions and npm semver
ranges are supported. Aliases such as `node` and `lts/*`, invalid ranges, and
conflicting constraints fail with `requirementConflict`; no alias is resolved
by changing fnm's default. Non-npm package managers can still be registered in
Stage 1, but preparation returns `unsupportedPackageManager`.

**Environment:** begin with an empty dictionary. Copy only ordinary user and
locale fields (`USER`, `LOGNAME`, `LANG`, `LC_*`, `TZ`); force the registered home,
working directory, selected Node/npm paths, and `/bin/sh`. PATH contains the
selected Node's bin directory followed by `/usr/bin`, `/bin`, `/usr/sbin`, and
`/sbin`. Relative entries, manager shims, and inherited PATH do not enter it.
`NODE_OPTIONS`, `NODE_PATH`, fnm/nvm/Corepack settings, inherited npm config,
shell startup hooks, dynamic-loader variables, and arbitrary secrets are not
forwarded. User/global npm config reads are disabled using distinct null-device
paths (npm rejects loading one config pathname twice); npm's script shell and
node-options settings are controlled explicitly. No profiles or global defaults
are written, and preparation never loads a login shell. Stage 1 discovery still
uses the existing discovery behavior documented above.

A child environment is just a map supplied to one subprocess. Updating
that map changes neither the app's environment nor the user's terminal. Each
project gets a separate map, so selecting Node 22 for one project does not change
another project's Node 24. Node is addressed by its absolute executable path;
PATH ordering also makes ordinary `node` commands find that installation first.

npm itself prepends project/ancestor `node_modules/.bin` paths to scripts' PATH.
Preparation therefore rejects a Node/npm shim in those locations unless it
resolves to the selected tool. See the [npm run documentation](https://docs.npmjs.com/cli/commands/npm-run/).
This is tool-selection isolation, not a security sandbox for project code. Scripts that explicitly invoke another executable remain project code.
Custom SDK paths, shell functions, and arbitrary exported application variables
are intentionally not imported by this first environment policy.

The validation receipt is not a launch token. `node_command()` only constructs
an unspawned Node command, without script arguments. The Stage 3 runner re-prepares
immediately before a managed start; files can change after a successful preparation.
Preparation alone never starts, supervises, or logs a project process. Environment values are never serialized or persisted.

New failures use `runtimeChanged`, `npmUnavailable`, `environmentUnavailable`,
`requirementConflict`, and `unsupportedPackageManager`, alongside Stage 1 error
codes. Invalid paths, missing runtimes, non-executable binaries, malformed npm
metadata, stale registrations, and changed package files fail without changing
the saved selection.

## Registry and isolation verification

Rust tests in `node_projects/tests.rs` and `node_projects/store.rs` use isolated
temporary directories and an injected inventory. Fake runtime files are never
executed. Coverage includes malformed/oversized files, Unicode paths, aliases,
missing runtimes/roots, stale revisions, unsupported platforms, private writes,
failed publication/retry, corrupt registry preservation, and unchanged PHP/fnm
files. TypeScript tests cover the serialized error contract.

Run the normal checks from `docs/testing.md`. `isolation_tests.rs` also covers
npm pairing, environment poisoning, replaced files/links, missing tools, and
unchanged preferences. The real-runtime integration test is deliberately opt-in
and runs separately because it owns temporary process-local discovery variables:

```sh
FAIRWAY_TEST_NODE_RUNTIMES=/absolute/node22/bin/node:/absolute/node24/bin/node \
  cargo test --manifest-path src-tauri/Cargo.toml --lib --all-features --locked \
  real_two_runtime_isolation_and_requirements -- --ignored --test-threads=1
```

It uses the existing fnm inventory with a scratch store, prepares two genuinely
different installed runtimes, runs only constant metadata probes, checks ranges
and conflicts, and confirms the scratch default and activation markers remain
unchanged. No project script is executed and no Node version is installed or
activated by the feature. Supply externally provisioned runtimes for this test.

## Stage 3: execution and process ownership

`execution.rs` owns run state, operation leases, observers and app-side draining.
`runner.rs` implements a dedicated native supervisor, launched from the **same
Fairway executable** with `--node-project-runner` before Tauri single-instance or
plugin initialization. `run_logs.rs` stores logs. The example executable is only
a controlled test transport; production does not require another bundled binary.

A Start request must contain `confirmed: true` from an explicit user action.
Import, inspection, startup and status/log reads never execute scripts. The
runner reloads the registration, checks registry and manifest revisions, reuses
Stage 2 to validate exact Node/npm versions, fingerprints and constraints, then
checks the manifest again. It executes the absolute selected Node with the
paired npm CLI and separate arguments `--silent`, `run-script`, and the selected
script name. Fairway never builds a shell command by interpolating project
paths or commands. npm itself executes the authorized script and its usual
pre/post lifecycle hooks using its script shell. Leading-dash script names are
rejected. v1 has no extra script arguments or dependency installation.

Each launch receives an unpredictable run ID and a private inherited Unix socket.
There is no public control listener. One helper remains the process-group leader
for the npm process and its ordinary descendants. A process group is an OS label
shared by related processes: signalling it reaches the npm wrapper, script and
ordinary children together. Stop requests travel over the private socket; the
app never signals a saved PID or searches for an executable name or port.

The helper sends `SIGTERM` to its own group and allows two seconds for graceful
cleanup. If non-zombie members remain, it sends `SIGKILL` to that same group,
including itself. Keeping the leader alive until the last signal reserves the
group ID and prevents it from being recycled for an unrelated run. The app reaps
its direct helper child before releasing the runtime lease. A private flock on
the project's app-owned `run.lock` also prevents a new session from overlapping
an older helper still cleaning up.

The private socket is a liveness channel: abrupt Fairway termination closes it,
and the surviving helper cleans up the group. No renderer subscription or
window is needed. Closing/hiding a window keeps native runs alive. Explicit Quit
waits for the current operation and attempts **both** Node and PHP cleanup even
if one fails. The final exit path disconnects Node channels as a fallback.
Updater preparation stops Node before bundle replacement; a cleanup error aborts
installation. Its existing lifecycle reservation prevents Start/uninstall/Quit
from racing installation. PHP Start All/Stop All never starts or stops Node.

A starting/running/stopping run holds a lease on the selected runtime. Node
Settings checks that lease before its fnm install/replacement/uninstall path,
including before changing activation markers during uninstall. A failed helper
retains ownership until reaping. External tools can still remove an installation;
the next launch then fails revalidation rather than selecting another runtime.

Limits: one active run per project, eight active projects, two recent in-memory
runs per registered project. Editing/removing a running registration requires
Stop. Restart waits for cleanup and creates a fresh run ID; it never adopts the
old process. Removed registrations release in-memory history; app-owned rotated
logs and last-run metadata remain available until manually removed. Imported
project files are never rewritten or deleted by the controller. Project code
itself can write files as usual when explicitly executed.

### Execution IPC

All commands use the existing typed `{ request }` convention; read-only
`node_project_runs` has no request. Start/Stop/Restart participate in the existing
native lifecycle operation guard. Stop and log retrieval do not need a healthy
registry, so registry corruption cannot prevent cleanup.

| Command | Request | Result |
| --- | --- | --- |
| `node_project_start` | `{ id, expectedRevision, manifestRevision, confirmed: true }` | Starting `NodeRunStatus` |
| `node_project_stop` | `{ id, runId }` | Terminal `NodeRunStatus`, after cleanup |
| `node_project_restart` | Start fields plus `runId` | Starting status with a new run ID |
| `node_project_runs` | None | `{ supported, runs }` |
| `node_project_logs` | `{ id, runId, afterCursor }` | `NodeLogBatch` |

`NodeRunStatus` contains `projectId`, `runId`, monotonic `revision`, `state`,
millisecond timestamps (`createdAt`, nullable `startedAt`/`endedAt`), nullable
`exitCode`/`signal`/typed `error`, `forcedCleanup`, and selected `runtime` metadata.
It exposes no PID, child environment or readiness claim. `running` means npm was
spawned; it does not mean the framework has opened a port or is healthy.

States are `starting`, `running`, `stopping`, `stopped`, `exited`, and `failed`.
Natural completion enters cleanup (`stopping`) before `exited` (zero) or `failed`
(nonzero/signal/validation/runner failure). Requested cleanup ends as `stopped`.
Duplicate Start returns `projectBusy`; stale Stop/Restart returns `staleRun`.
Stop for the same current terminal run is idempotent. New typed errors also
include `confirmationRequired`, `runNotFound`, `executionFailed`, `logUnavailable`.

Subscribe to `node-project-run-status` before obtaining a run snapshot. Compare
`runId` and `revision` to discard delayed events, and refresh after reconnect.
Observers carry low-rate status changes only; stdout/stderr do not flood Tauri
events. Snapshot metadata survives restarts in private `last-run.json` files.
A previously active run is reported as interrupted/failed, with no adoption or
signalling based on the old metadata and no automatic restart.

### Logs and streaming

Native workers drain stdout/stderr continuously in 4 KiB chunks even without a
view. Incremental UTF-8 decoding preserves split code points, multiline text and
stream identity; malformed bytes become replacement characters. Stream ordering
is preserved individually; stdout and stderr have no guaranteed combined order.
Each record has `{ runId, cursor, time, stream, text }`, where stream is `stdout`,
`stderr` or controller-generated `system`.

Memory retains at most 256 KiB of text and 1,024 records per run. App-owned private
JSONL files under `<Layout.data>/node-project-logs/<projectId>/` rotate between
`events.jsonl` and `events.1.jsonl`, each limited to 5 MiB. Unix directories use
0700 and files 0600; symlink/special-file log targets are rejected. Disk-write
failure is reported in the log batch while native draining and memory logs
continue. Fairway does not serialize its environment or log the npm command;
project output can contain whatever the authorized script prints.

Start retrieval at cursor zero, then pass the returned `nextCursor`. Each batch
contains at most 256 records / 64 KiB of text, plus `truncated` (a retention gap),
`eof` (finished and caught up), and nullable `error`. Poll while a view is active;
append records using their stream and cursor. Cursor values are local to a run.
Retained disk logs support bounded recent-history retrieval after app restart;
they are not an unlimited archive or a second process owner.

### Foreground-process boundary

The cleanup guarantee covers ordinary descendants that keep the inherited
process group. An intentionally daemonizing script that calls `setsid`/moves to
another group escapes this contract and should not be managed by v1. Killing
both Fairway and its supervisor externally also defeats the liveness helper.
This is a foreground development runner, not a security sandbox, container or
OS daemon service. Sleep/power-loss and signed updater installation still need
device-level checks; the controller does not claim application readiness.

### Controlled execution verification

Build the test helper and use two independently provisioned real Node/npm
installations. The tests create all scripts and project files in disposable
folders; no existing project script is used. Include a read-only packaged Fairway
runtime for the PHP coexistence/lifecycle test:

```sh
cargo build --manifest-path src-tauri/Cargo.toml --example node_projects_runner \
  --no-default-features --locked
FAIRWAY_TEST_NODE_RUNNER="$PWD/src-tauri/target/debug/examples/node_projects_runner" \
FAIRWAY_TEST_NODE_RUNTIMES=/absolute/node22/bin/node:/absolute/node24/bin/node \
FAIRWAY_TEST_RUNTIME=/absolute/Fairway.app/Contents/Resources/runtime \
  cargo test --manifest-path src-tauri/Cargo.toml --lib --all-features --locked \
  owned_ -- --ignored --test-threads=1
```

Coverage includes concurrent runtime versions, duplicate Start, stale run IDs,
Restart, Stop during starting, immediate exit/failure, stubborn descendants,
abrupt host kill, cursor gaps, 12 MiB floods/rotation, Unicode, persisted recovery,
registry corruption, runtime leases, lifecycle reservations, and real PHP
Start All/Stop All independence. Core lifecycle tests also verify both cleanup
stacks are attempted if either fails. Browser regression checks exercise the
existing PHP UI; native tray hiding, OS dialogs and a signed updater replacement
remain separate device-level checks.

## Stage 4: graphical workspace

Open **Node Projects** in the Tools navigation group. Import uses the existing
native folder picker, inspects scripts/requirements/runtime inventory, and saves
through the typed registry API. Saving never starts a script or installs packages.
npm selections are validated after saving; incompatible selections stay saved
with an actionable warning so the user can choose Edit and correct them. Other
package managers can be registered but cannot be launched by the npm runner.

Start and Restart inspect the folder and show the selected script in an explicit
execution confirmation. They submit the reviewed registry/manifest revisions and
`confirmed: true`. Stale run IDs and conflicts require another user review; the
UI never automatically retries execution against a different run. Editing and
removal require a stopped project. Remove preserves project files. Open URL uses
the existing native HTTP-link opener; the configured URL is not a readiness check.

Frontend responsibilities:

- `NodeProjectsPage.tsx`: registration cards, confirmations, controls and recovery.
  Active runs without a loaded registration still expose Stop and Logs; registry
  errors also expose retained run history. Normal removal hides inactive metadata
  retained by the backend.
- `ProjectEditor.tsx`: import/edit drafts and native inspection. A failed
  inspection clears the previous result; request generations discard delayed
  results from an older folder. Shared `data/Dialog.tsx` supplies focus trapping
  and optional explicit return focus for WebKit.
- `useProjects.ts`: awaits registry/run subscriptions before initial snapshots,
  serializes refreshes, and rejects stale snapshots using event epochs. Within a
  run only higher revisions replace state. Across runs timestamps distinguish
  generations; equal timestamps require an authoritative snapshot. Three-second
  visible-page refreshes and focus/visibility refreshes recover missed events.
- `ProjectLogs.tsx`: keyed by project/run ID, retrieves one cursor batch at a
  time, at most every 500 ms, while the page/document is visible. EOF stops
  polling; errors offer Retry from the last cursor. React owns no native process.
- `model.ts`: pure, tested merge/URL/error/log-retention rules. The console keeps
  at most 1,024 records and 256 KiB UTF-8 text, deduplicates cursors and renders
  React text nodes. It identifies timestamps and stdout/stderr/system. Clear view
  keeps the cursor and native files; manual scrolling disables Follow output.
  The console reuses the maximizable workspace and existing log styling.

Navigation pauses/unsubscribes UI work without stopping native projects. Reentry
obtains current snapshots. A complete WebView reload reconstructs status; select
Logs again to read retained output. Unsupported platforms show an explanatory
page with disabled import/execution controls. PHP navigation, dashboard and
service contracts remain unchanged. Stage 4 changed no production Rust backend
module; the example bridge was extended only for disposable frontend tests.

TypeScript tests cover revision/generation ordering, local URLs, Unicode/log
bounds and recovery instructions. `node-projects.spec.ts` covers dialogs,
import/edit/remove, concurrent runs, conflicts/errors, console scrolling/retry,
hidden-page polling and PHP independence in Chromium/WebKit at both window sizes.
`node-projects-native.spec.ts` uses the production frontend, real Rust controller
and controlled Node 22/24 fixtures, including UI reload, Restart, Stop and file
preservation. These command-line browser tests do not control the user's desktop.

See [manual acceptance testing](node-projects-manual-testing.md) for safe MacBook
checks and issue reporting. Signed packaging, Windows/Linux device checks and
final user acceptance remain separate. Reverse proxies, dependency installation,
automatic restarts, PostgreSQL, Redis and other services are outside this work.
The Node backend stays separate from PHP without a generic plugin framework.
