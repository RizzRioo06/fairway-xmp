# Node project creation — catalog, local generation and recovery

Stages 1–2 provide a backend service separate from importing/running existing
Node projects. Planning is read-only. Explicitly confirmed Generate writes local
files, publishes a new folder and registers it. There is no dependency installer,
framework initializer, registry network access, graphical creation wizard or
automatic project execution. Existing PHP and Node controls keep their behavior.

## Versioned local templates

The compiled catalog is schema 1, revision 2. Every template is revision 2,
`qualification: localGeneration`, `lockfileStatus: notPrepared`. Its content
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
  design provenance, dependency/license references and not-installed status.

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

**No template contains a package lockfile or node_modules.** No lockfile is
fabricated. Installation, full transitive lockfile provenance, dependency/build
compatibility and any install-script policy require Stage 3 review. The displayed
prospective npm `ci` policy is disabled (`installationAvailable: false`) and is
not runnable until an appropriate lockfile/install plan exists. Node-only JS has
no third-party dependencies, but no npm lockfile is asserted for it either.

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
quotes; it never imports/executes template code. Installation/build qualification
of framework projects remains intentionally pending.

Opt-in `real_local_generation_uses_selected_runtime_without_installation` uses
provisioned Node 22/24 and their paired npm to generate/register all six projects.
`real_creation_planning_uses_inventory` proves shell-free planning does not write
fixtures or execute profiles. Both download/install/start nothing. The existing
native PHP/Node lifecycle coexistence fixture is also run against disposable data
and read-only packaged tools. See [Testing](testing.md) for commands and evidence.

Stage 3 remains unimplemented: owned dependency installation, qualified lockfiles,
network/script-policy review, install progress/logs and installer process supervision.
Stage 4's graphical wizard is also outside this work. No PostgreSQL, Redis, Caddy,
additional package managers, templates or generic plugin framework are introduced.
