# Node project creation — planning, generation and owned installation

Stages 1–3 provide a backend service separate from importing/running existing
Node projects. Planning is read-only. Confirmed Generate still writes/publishes
local source files without installation. The separate confirmed Install action
generates into a private stage, runs the selected npm with scripts disabled,
verifies the result, then publishes/registers. No graphical wizard or automatic
project execution is introduced. Existing PHP and Node controls keep their behavior.

## Versioned local templates

The compiled catalog is schema 1, revision 3. Every template is revision 3,
`qualification: scriptDisabledInstall`, `lockfileStatus: npmGenerated`. Its content
revision hashes the catalog, renderer, template sources and MIT license. Plans
also contain the exact byte size and SHA-256 of each materialized file, so local
asset changes require a fresh review plan.

Common generated files:

- `package.json`: private ESM package, catalog scripts, exact direct dependencies,
  Node/npm engine ranges and exact paired `packageManager: npm@VERSION`.
- `.node-version`: selected exact Node version; no global defaults change.
- `.gitignore`: dependencies, builds, logs and real environment files excluded.
- `README.md`: setup status, workflow, local address, environment and provenance.
- `LICENSE`: unchanged Fairway MIT license and copyright notice.
- `.env.example`: nonsecret local defaults; real `.env` is never created.
- `.fairway-template.json`: random creation ID, catalog/template/asset revisions,
  design provenance and dependency/license references. This immutable source marker
  describes generation, not live installation state; the durable journal owns status.
- `package-lock.json`: genuine npm-generated complete resolution graph and integrity.
- `.fairway-install.lock`: empty private workspace ownership lock.

| Template ID | Additional files | Scripts |
| --- | --- | --- |
| `node-js` | `src/index.js`, `src/server.js`, `test/server.test.js` | dev/start/test |
| `node-ts` | `src/index.ts`, `tsconfig.json` | dev/build/start |
| `hono-ts` | `src/index.ts`, `tsconfig.json` | dev/build/start |
| `express-js` | `src/index.js`, `src/app.js`, `test/api.test.js` | dev/start/test |
| `react-vite-js` | `index.html`, `src/main.jsx`, `src/App.jsx`, `src/index.css`, `vite.config.js` | dev/build/preview |
| `react-vite-ts` | `index.html`, `src/main.tsx`, `src/App.tsx`, `src/index.css`, `vite.config.ts`, `tsconfig.json`, `tsconfig.app.json`, `tsconfig.node.json` | dev/build/preview |

API starters use loopback port 3000, a health endpoint and signal-aware shutdown.
They load `.env` only when explicitly started later. Vite defaults to loopback
5173 with strict port behavior; its example warns that `VITE_` values are public.
Generation never starts any of these listeners or runs their tests.

The sources in `src-tauri/templates/node-projects/` are Fairway-authored under the
included MIT license, informed by official Node, TypeScript, Hono, Express and
Vite references. No upstream template archive or scaffolder is copied/executed;
`upstreamRevision: null` states that honestly. Framework packages retain their
own licenses when installed. Catalog dependencies remain metadata, not new app
build dependencies.

Each template bundles a genuine npm v3 lockfile; no `node_modules` is bundled.
`locks/provenance.json` records raw SHA-256, generator Node/npm versions, registry,
and package counts. Generation normalizes only root package names. Version and
integrity entries remain npm-generated. The renderer checks exact root dependencies,
registry URLs, integrity structure and path bounds. Install rechecks the exact
reviewed file hash before invoking npm. The immutable plan's `installationAvailable`
is true; the separate Install request must include both `confirmed` and
`installConfirmed`. Generate/Plan never invokes npm ci.

## Runtime and read-only planning

Supported selections are Node `>=22.12.0 <23 || >=24.0.0 <25`, paired npm
`>=10.0.0 <12`. This bounds the initial support matrix. Discovery reuses the
existing fnm/nvm/Homebrew inventory scanner without loading login-shell startup
files or invoking fnm. Login-shell-only custom stores may therefore be absent;
`discoveryNote` reports this limit. Existing import/Settings discovery is unchanged.

Only discovered runtime IDs are accepted. The existing isolated validator checks
absolute Node/npm paths, exact versions, npm pairing, semver requirements,
fingerprints and ancestor shims. Fixed metadata probes and `npm --version` run in
the npm installation directory with a cleaned environment. No project script,
initializer or install command is executed. No profiles, activation markers,
defaults, user/global npm config or global Node settings are changed.

Plans are native snapshots retained in memory for ten minutes (monotonic deadline),
at most 64 live plans. Their random IDs/digests bind the reviewed files, destination,
parent identity, template/dependency metadata and validated tools. Identical
request IDs return the same unconsumed plan; changed inputs are rejected. Client
edits cannot replace the native plan. Plans expire on restart and never reserve a
folder. Generate consumes a plan once; recovery uses its durable journal instead.
A digest is a consistency check, not a signature or permission to execute code.

Names are 1–64 lowercase ASCII letters/digits/hyphens, starting with a letter and
ending with a letter/digit, excluding reserved device names. Unicode/spaces are
supported in parent paths. The parent must exist, canonicalize safely, and be
writable/searchable. Traversal, control characters, non-UTF-8 paths and PATH
separators are rejected. Every existing destination is refused, including an
empty folder, regular file, live symlink or broken symlink.

## Filesystem ownership and publication

macOS operations walk canonical paths with directory descriptors and `openat`
using `O_DIRECTORY`, `O_NOFOLLOW` and `O_CLOEXEC`. A descriptor holds a specific
folder; replacing its pathname cannot redirect an open handle to another folder.
Each nested parent is also checked against its journaled device/inode before
writing. This specifically prevents a replacement `src` from receiving files.

Generate revalidates native plan authority, destination and tool fingerprints
before mutation. It claims a private durable journal, then exclusively creates
`.fairway-node-<random-plan-id>` as a sibling of the destination on the same
filesystem. Directories use 0700; regular files use exclusive/no-follow creation
and 0600. Entries must be bounded relative paths, never links or traversal.
Limits are 128 files, 256 KiB/file and 2 MiB total reviewed content.

The provenance marker is written and receipted first. The journal retains its
random creation ID, file hashes/sizes, modes/modification times, and inode receipts
for the root and nested folders. Inodes alone are insufficient evidence after a
crash because numbers can eventually be reused. The full tree must match the
reviewed files before publication: no extra/missing/changed/link/special entries.

Files and directory entries are flushed. Publication uses macOS
`renameatx_np(..., RENAME_EXCL)` relative to the anchored parent, followed by a
parent sync. This atomically refuses an existing destination, including one that
appeared after preflight. There is no overwrite fallback if the filesystem does
not support this operation. Root/parent identities are checked again afterward;
unsafe changes block registration and preserve data.

These checks protect normal concurrent path replacement and accidental changes.
They are not a sandbox against an actively malicious process with the same account
permissions or administrator access. No permission elevation, recursive ownership
change or system configuration modification is used.

## Durable journals and explicit recovery

Journals live in `<Layout.data>/node-project-creation/<job-id>.json`, separately
from `node-projects.json`. The directory is private; regular bounded journals are
published with unique staging files, flush, atomic replacement and directory sync.
A nonblocking private file lock serializes generation/recovery across processes;
normal operations also have an independent service lock. History is bounded to
256 journals; it is never silently pruned to make room.

Phases: prepared, staging, ready, publishing, needsRegistration, completed,
cancelled and preserved. Important transitions and each created directory/file
are journaled. The persisted reviewed manifest is self-contained for recovery;
it is not replaced by a later template revision. Malformed/future/unsafe journals
are reported and preserved, never interpreted as an empty history.

Listing jobs is read-only. After restart, nonterminal inactive jobs report
`recoveryRequired`; nothing resumes, installs, publishes or starts automatically.
Explicit recovery requires the job ID, current journal revision and confirmation.
Stale revisions and terminal/duplicate mutations are rejected.

- **Discard Staging:** only for an unpublished, positively identified stage whose
  complete current contents match recorded ownership receipts. Missing ownership
  markers, unknown/modified/unjournaled files, hard links, symlinks, replaced folders
  or uncertain partial writes are preserved. No recursive wildcard removal is used.
  Cancellation before the publication commit uses the same checks.
- **Retry Registration:** only for a published folder matching the original stage
  identity and the entire reviewed manifest. Incomplete trees cannot be adopted.
  Selected tools are revalidated. The existing registry serializer/atomic writer
  is reused through an internal handoff without shell discovery. Expected registry
  revisions protect unrelated edits. A deterministic project ID makes a retry
  idempotent if the registry save succeeded just before a crash.

Once publication begins, cancellation is too late to promise rollback. A published
folder is never deleted by this service, even after registry corruption/conflict,
failed save or requested discard. It remains available for registration retry or
manual inspection/import. User edits block automatic receipt-based recovery but
are kept. Recovery of unfinished staging is discard-and-replan, not automatic
resumption/publication. A failure during cleanup can leave an uncertain remainder
for manual review; safety takes precedence over removing every temporary file.

Generation/cancellation metadata checks are bounded, but filesystem calls still
rely on OS/filesystem behavior. Actual disk-full/power-loss/network-volume faults
and signed-update/device behavior are not claimed by simulated IO/crash fixtures.

## IPC and lifecycle

TypeScript contracts/wrappers are in `src/node-project-creation/`. Blocking work
runs off the Tauri UI thread. Generate/Recover hold the existing desktop operation
guard, so Quit/updater installation wait rather than race file publication. Running
PHP/Node services keep running; their Start All/Stop All semantics are unchanged.
Cancellation and read-only job listing bypass that guard. Successful registry
handoffs invalidate Node cards using the existing real registry revision event.
No GUI creation workflow is added.

| Command | Request | Result |
| --- | --- | --- |
| `node_project_templates` | None | Catalog/runtime choices/support |
| `node_project_creation_plan` | requestId, catalogRevision, templateId, projectName, parent, runtimeId, packageManager: npm | Immutable plan with file hashes |
| `node_project_creation_validate_plan` | planId, digest | Same valid plan |
| `node_project_creation_generate` | planId, digest, confirmed | Job status after local generation/registration |
| `node_project_creation_jobs` | None | Retained jobs and individual journal errors |
| `node_project_creation_cancel` | jobId | Cancel active job before publication commit |
| `node_project_creation_recover` | jobId, expectedRevision, action, confirmed | Updated job status |

Requests use the existing `{ request }` wrapper and camelCase DTOs. Unknown fields
are rejected. Mutations expose typed errors; a job may also return `preserved` or
`needsRegistration` with an error and explicit recovery requirements. Do not treat
those statuses as completion. Unsupported systems compile guarded stubs and report
unsupported operations. Production macOS IPC is compile-checked; no device GUI
automation or installed-app mutation was performed.

## Verification and next stage

Fixtures cover every template's exact reviewed files/manifests, no installs/runs,
Unicode/spaces, permissions, traversal, live/broken symlinks, late destinations,
root/nested replacements, consumed/expired/tampered plans, partial files, journal
corruption, cancellation, failed registration and retry, registry-save replay,
interrupted staging and actual SIGKILL host death at staging/publication boundaries.
The nested-directory reproducer failed before identity checks and passed afterward.
Syntax-only TypeScript compiler parsing caught and fixed both React entry-point
quotes; it never imports/executes template code. Installation/build/run qualification
is covered separately by the explicit offline installer suite below.

Opt-in `real_local_generation_uses_selected_runtime_without_installation` uses
provisioned Node 22/24 and their paired npm to generate/register all six projects.
`real_creation_planning_uses_inventory` proves shell-free planning does not write
fixtures or execute profiles. Both download/install/start nothing. The existing
native PHP/Node lifecycle coexistence fixture is also run against disposable data
and read-only packaged tools. See [Testing](testing.md) for commands and evidence.

## Stage 3: owned npm installation

`installation.rs` defines typed finite jobs; `installation_native.rs` validates
and runs them on macOS. The shared Node runner accepts a narrow internal install
launch variant. A separate `Executions` instance keeps installer status/logs out
of the ordinary Node Projects list. One installer runs at a time; project servers
and PHP services can continue independently. The desktop operation reservation
covers acceptance/staging only, so a long network download does not block PHP controls.

The exact selected Node executable runs its paired npm CLI with validated arguments:
`ci --ignore-scripts --no-audit --no-fund --include=dev --global=false
--workspaces=false --strict-ssl=true --engine-strict=true`. Fetch retries/timeouts
are bounded; the helper enforces a five-minute job deadline. No shell command,
remote initializer, dependency upgrade or development server is invoked.

Preparation revalidates exact Node version/fingerprint, paired npm CLI, metadata,
semver and the entire source receipt manifest. The child starts with Stage 2's
allowlisted environment and selected Node first on PATH. HOME, cache and TMPDIR
are newly created private app-owned per-job directories. Personal/global npm
config and inherited npm credentials/options/proxies are absent; generated stages
contain no `.npmrc`. TLS and SHA-512 dependency integrity remain enforced. The
production registry is fixed to `https://registry.npmjs.org/`. There is no IPC URL
or command override. A debug-only `node-install-fixtures` feature permits loopback
mirrors set directly by Rust tests; release builds reject them even with that feature.

`npm ci` may erase an existing dependency directory, so it is accepted only in
an unpublished stage with **no existing node_modules**. The working directory is
pinned to its opened inode with `fchdir` immediately before exec. Existing parent,
stage and nested source checks remain intact. After npm and its helper are fully
reaped, bounded descriptor walks hash dependency files, inodes, modes and safe
`.bin` links. Absolute/escaping/chained dependency links, hard links and special
files are rejected. Before publication and registration, source receipts and the
entire dependency receipt are checked again. The same exclusive atomic directory
rename and atomic registry save from Stage 2 are reused.

A dependency receipt is a safety check for later recovery, not a malware scan.
Node/runtime isolation is not an operating-system sandbox. Downloaded package code
can execute when a user later explicitly builds or starts the project.

### Ownership, cancellation and shutdown

Every installation has distinct creation, job and run IDs plus revision checks.
States are `starting`, `installing`, `completed`, `failed`, `cancelled`, `interrupted`.
`installing` means a process is running; it does not mean the application is ready.
The existing helper remains the process-group leader, so the group identity cannot
be reused while it signals. Stop uses the live private channel, SIGTERM, then
bounded SIGKILL for stubborn ordinary descendants. No saved PID, executable name
or port is used for signaling. Host death closes the channel and triggers cleanup.
Intentionally detached/escaped processes are outside this ordinary-descendant model.

Cancellation before publication sets an owned flag and drains the group; once
publication commits, cancellation reports too late and keeps the project. Worker
errors also drain the native job before releasing runtime/workspace ownership.
The workspace flock remains held by the helper across host death. Recovery and
runtime removal honor that lock while an earlier helper finishes cleanup. The
controller waits for reaping, not just npm's initial exit event, before publication.

Hiding a window leaves jobs running. Explicit quit cancels staging/install jobs,
waits for ownership cleanup, and attempts Node and PHP cleanup even if another
component fails. The updater performs the same installer drain before replacement;
a cleanup error prevents installation of the update. Runtime uninstall/replacement
checks both project runs and installer leases. PHP Start All/Stop All remain unchanged.

### Persistence and recovery policy

The version-1 creation journal gains optional install status and dependency receipt
fields; old Stage 2 journals remain readable with their persisted review manifests.
After host restart, unfinished installation status is **interrupted**, never adopted
or automatically restarted. `creation_jobs` includes installation IDs/status for
recovery discovery. No PID grants authority. App-owned cursor logs reuse bounded
256 KiB/1024-record memory and rotating 5 MiB disk segments; stdout/stderr stay
identified and Unicode-safe. Status/diagnostic contracts contain no environment map.

- `finalize`: only after durable `ciCompleted`, intact source/dependency receipts,
  current tools and explicit confirmation. It completes exclusive publication and/or
  retries registration. The deterministic creation ID prevents duplicate registration.
- `discardStaging`: only intact unpublished source-only data. A partial or installed
  `node_modules` is deliberately preserved: it can contain user changes. Published
  projects are never deleted. Source-only cleanup reuses owned Stage 2 receipts.
- **Retry:** review a fresh plan and authorize a fresh job/workspace. Never rerun
  destructive ci in a partial workspace. The old failed stage is retained for inspection.
- **Already published Stage 2 project:** select a new destination and review a fresh
  generation/install plan for the same template. This is an explicit new project,
  not an in-place upgrade or automatic migration. The old folder and registration
  remain untouched; old user edits/dependencies are not copied. Reusing its occupied
  destination is rejected. Any later manual removal is the user's separate decision.

App-owned installer homes/caches and uncertain dependency stages are retained,
not automatically recursively deleted. History remains bounded to 256 journals;
there is no automatic retry, cache pruning or background installation.

### IPC

| Command | Request | Result |
| --- | --- | --- |
| `node_project_install_start` | planId, digest, confirmed, installConfirmed | InstallStatus |
| `node_project_install_status` | jobId | InstallStatus |
| `node_project_install_logs` | jobId, afterCursor | NodeLogBatch |
| `node_project_install_cancel` | jobId, expectedRevision | InstallStatus |
| `node_project_install_recover` | jobId, expectedRevision, action, confirmed | InstallStatus |

TypeScript wrappers mirror these Rust DTOs. No graphical wizard is implemented;
a future client should refresh the Node registry when an install completes.

### Qualification and limitations

The six genuine locks were generated separately against public npm on 2026-10-28
using Node 22.23.4/npm 10.9.9 with lifecycle scripts disabled. Public clean installs
and the four build scripts passed. One React download initially failed with
EIDLETIMEOUT; retry with a 60-second fetch timeout succeeded with TLS/integrity intact.
This was a qualification network failure, not a reproducible code defect.

The controlled offline mirror then passed all six variants with real npm on both
Node 22.23.4/npm 10.9.9 and Node 24.20.0/npm 11.19.0, including normalized project names,
build/test scripts, dev-server HTTP responses and owned shutdown. esbuild/tsx/Vite
and Rollup worked without install lifecycle hooks on macOS arm64 because their
pinned platform packages supplied the binaries. No lifecycle-script exception was
needed. This does not qualify other CPU/OS combinations or future dependency versions.

Maintainer commands (only disposable output; public qualification is explicit):

```sh
python3 scripts/qualify_node_templates.py --node /absolute/node/bin/node
# Candidate new locks only, never edits repository:
python3 scripts/qualify_node_templates.py --node /absolute/node/bin/node --generate-locks
# Build the native fixture helper with --features node-install-fixtures first.
python3 scripts/test_node_installations.py --qualification-cache /tmp/fairway-template-qualification-... \
  --runner /absolute/target/debug/examples/node_projects_runner \
  --node /absolute/node22/bin/node --node /absolute/node24/bin/node
```

The offline server reads only genuine cached SHA-512 tarballs, never proxies/fetches
remote packages, and also supplies corrupt bytes and 503 responses for failure tests.
A reserved TCP port without a listener exercises unavailable-network handling.
Use `--runtime /absolute/packaged/runtime` to include the live PHP coexistence test,
or `--test-filter NAME` for focused checks. Freeze a helper built with
`--features node-install-fixtures` outside Cargo output before changing build features;
the harness checks its capabilities before running.
The production code never exposes this server configuration through IPC.

Windows/Linux native installation, signed installers, live updater replacement,
manual GUI acceptance, deliberate process-group escape, a five-minute wall-clock
expiry, power-loss simulation and a malicious same-user filesystem adversary are
not qualified by these tests. Native hooks are platform-guarded. macOS filesystem
identity checks mitigate races but do not create a sandbox. Normal user project
data and the installed app are never used as installation test workspaces.

Stage 4's graphical wizard remains outside this work. No PostgreSQL, Redis, Caddy,
additional package managers, templates or generic plugin framework are introduced.

Stage 3 test evidence and the complete file inventory are recorded in
[Node installation verification](node-installation-verification.md).
