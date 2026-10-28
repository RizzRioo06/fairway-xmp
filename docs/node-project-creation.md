# Node project creation — Stage 1: catalog and dry-run plans

This is a backend foundation, separate from importing/running existing Node
projects. It exposes no Create, Install, Cancel Setup or Start Setup command and
adds no graphical wizard. It neither creates project directories nor installs
dependencies, registers a project, or executes a project/template script.

## Catalog

`src-tauri/src/node_project_creation/catalog.json` is compiled into the app. Its
schema version is 1; each design has an explicit revision. The catalog revision
returned to callers is SHA-256 of the complete embedded catalog bytes: any change
invalidates older requests. There is no runtime catalog download or initializer.

| ID | Design | Intended workflow |
| --- | --- | --- |
| `node-js` | Plain JavaScript Node HTTP starter | dev/start/test |
| `node-ts` | Plain TypeScript Node starter | dev/build/start |
| `hono-ts` | Hono TypeScript API with Node adapter | dev/build/start |
| `express-js` | Express JavaScript API | dev/start/test |
| `react-vite-js` | React + Vite JavaScript | dev/build/preview |
| `react-vite-ts` | React + Vite TypeScript | dev/build/preview |

Entries contain intended relative files, script descriptions, exact direct
package versions, dependency Node requirements, development/runtime roles,
package licenses, official version-specific registry metadata URLs, and design
provenance. Dependencies are proposed pins, not installed application dependencies.

These are Fairway-authored **designs**, informed by official Node, TypeScript,
Hono, Express and Vite references. No upstream template source has been copied;
`upstreamRevision: null` explicitly avoids claiming a pinned upstream snapshot.
Every entry is `qualification: planningOnly`, `lockfileStatus: notPrepared`.
Generated source assets, complete transitive lockfiles, license redistribution
and install/build qualification must be reviewed in later stages. Stage 1 does
not claim that these proposed dependency combinations have been installed/tested.

The initial planning policy accepts Node `>=22.12.0 <23 || >=24.0.0 <25` and paired
npm `>=10.0.0 <12`. This deliberately bounds the initial support matrix instead
of promising every Node version that an individual dependency might tolerate.
Node 22 type definitions are proposed for TypeScript starters. Future version
support is a reviewed catalog change, not an automatic selection/upgrade.

## Runtime reuse without shell side effects

The service uses the existing fnm/nvm/Homebrew inventory scanner, with a new
read-only entry point that uses process environment and standard installation
folders. It never loads a login shell/profile, calls fnm, changes a default,
claims a runtime store, or edits activation markers. Custom stores exported only
by a login profile may therefore not appear. `discoveryNote` explains this limit;
existing Node Settings/import discovery keeps its prior behavior.

Inventory probes clear inherited environment values. Planning accepts only a
freshly discovered runtime ID; callers cannot supply executable paths. It then
reuses the existing isolated Node/npm validator, exact version probes,
fingerprints, npm pairing, semver constraints and ancestor-shim checks. A narrow
internal adapter validates catalog requirements using the existing destination
parent as a working directory; no synthetic manifest is written. Tool exclusion
and shim checks use the intended child destination, so runtimes elsewhere under
the same parent are not incorrectly treated as project-owned tools.

Only fixed metadata programs and paired `npm --version` are executed, in the
npm installation directory and cleaned environment. No `npm create`, `npx`,
network request, install command or imported script is used by the service.
Metadata probes have the existing bounded time/output limits. This assumes
installed Node/npm tools are trusted; tool-selection isolation is not an OS
sandbox for a malicious executable. Fingerprints are validation receipts, not
permission to use those files indefinitely.

## Destination validation

The request supplies an existing absolute parent directory and a project slug.
Slugs are 1–64 lowercase ASCII letters/digits/hyphens, start with a letter, end
with a letter/digit, and exclude reserved device names. Unicode/spaces are
supported in parent paths; they are intentionally not package slugs.

The parent is canonicalized and must be a readable, writable/searchable folder.
Traversal, control characters, non-UTF-8 paths and PATH delimiters are rejected.
Its canonical path and Unix device/inode identity are retained internally.
`symlink_metadata` rejects every existing destination, including empty folders,
regular files, live symlinks and broken symlinks. Parent aliases resolve to their
canonical folder; changing an alias/parent invalidates the stored plan.

Validation never tests permissions by writing a file. The destination is checked
again after tool probes and on later plan validation. **A plan does not reserve
or lock the destination.** Stage 2 must add anchored filesystem operations and
exclusive/no-replacement publication rather than relying on these checks alone.

## IPC and TypeScript contracts

Contracts are in `src/node-project-creation/types.ts` and `api.ts`.
Tauri handlers live in `desktop/node_project_creation_commands.rs`; they dispatch
blocking probes off the UI thread. The service has independent native state and
does not acquire the PHP manager or Node registry locks. Construction is lazy:
adding its state performs no discovery/probes during PHP startup.

| Command | Request | Response |
| --- | --- | --- |
| `node_project_templates` | None | Support flag, catalog/digest, runtime choices and discovery note |
| `node_project_creation_plan` | `{ requestId, catalogRevision, templateId, projectName, parent, runtimeId, packageManager: "npm" }` | Immutable creation plan |
| `node_project_creation_validate_plan` | `{ planId, digest }` | Same plan after current destination/tool checks |

All request fields use camelCase inside the existing `{ request }` wrapper.
Unknown request fields are rejected. Errors are typed `{ code, message }` values,
including destination/name errors, stale catalog, runtime/npm failures, duplicate
request, unavailable/tampered/stale/expired plan, and capacity errors.
Unsupported systems return the catalog with `supported: false`; planning and
validation fail with `unsupportedPlatform`. No creation UI is added.

## Immutable, bounded review plans

Each native plan has an unpredictable 128-bit ID and a digest binding its schema,
catalog/template revisions, destination/name, intended files/scripts/dependencies,
Node/npm versions/paths/fingerprint, timestamps and installation policy. Internal
parent identity also contributes to the digest. IPC returns a copy; client edits
do not change the stored plan. Validation accepts a reference, not a replacement
plan body; a mismatched digest is rejected against the native record.

The prospective policy is npm `ci`, install scripts disabled, official registry,
and an app-owned cache. It explicitly says `dependencyLock: notPrepared` and
`installationAvailable: false`. It is a review description, not a runnable command
or a promise of working dependencies. Any change to files, dependency graph or
script policy in later stages requires a fresh plan and explicit confirmation.

Plans are retained only in memory, at most 64 live plans for ten minutes, using a
monotonic expiration deadline. Wall-clock timestamps are display metadata. Expired
entries are reclaimed on new requests. App restart loses every plan, preserving
all existing configuration. Revalidation returns the original plan rather than
silently extending its expiry or changing its contents.

A 32-hex `requestId` makes repeat/concurrent identical submissions idempotent:
they return the same plan. Reusing that ID with different inputs is rejected.
Expired/unavailable plans require a fresh request ID. Validation is read-only and
non-consuming; there is no execution submission in Stage 1. Later job creation
must atomically consume a confirmed plan and reject duplicate execution requests.
A digest is not a signature or an authorization token; native storage is the
authority. Missing runtimes, changed npm fingerprints, occupied destinations and
replaced parents prevent reuse. There is no auto-repair or automatic retry.

## Verification and next stage

Rust fixtures test invalid names/paths, permissions, Unicode/spaces, occupied
files/folders/live or broken links, unsupported/missing runtimes and npm, invalid
catalog/template IDs, tampering, stale parent/tools, expiration, restart, bounded
storage and concurrent duplicate requests. Filesystem snapshots verify planning
leaves fixtures unchanged and creates no registry/data directory.

The opt-in real test exercises all six designs with two externally provisioned
Node/npm installations, poisoned inherited runtime settings, a personal-style
npm config and a shell profile containing a marker command. It checks inventory,
exact paired versions, plan revalidation, incompatible requirements and unchanged
scratch files/defaults. It downloads nothing and creates no project server.

```sh
FAIRWAY_TEST_NODE_RUNTIMES=/absolute/node22/bin/node:/absolute/node24/bin/node \
  cargo test --manifest-path src-tauri/Cargo.toml --lib --all-features --locked \
  real_creation_planning_uses_inventory -- --ignored --test-threads=1
```

Run this alone/serially: its discovery variables belong to its disposable test
process and are restored afterward. TypeScript tests check error/DTO boundaries
and catalog policy. Existing runtime isolation/process/PHP coexistence tests cover
the shared validation/scanner reuse. See [Testing](testing.md).

Stage 2 remains unimplemented: materialized local assets, complete lockfiles,
private staging, durable recovery journals, exclusive filesystem publication,
explicit consent/plan consumption and safe registration handoff. Dependency
installation/supervision and the wizard remain later work. No global environment,
PHP behavior, installed application, release feed/signing identity or service is
changed by this foundation.

Stage 1 verification on macOS (2026-10-28): the all-features Rust suite completed
with 190 passes before one final IPC-shape test was added; the focused final
planner run passed all 13 fast cases (the real-runtime case is opt-in) and covers
that additional assertion. All eight opt-in real tests passed
separately, including planning on Node 22.23.4/24.20.0 and the existing owned-run,
crash cleanup, logs, runtime isolation and PHP independence fixtures. Terminal
feature tests passed (166), TypeScript tests passed (24), and the frontend build,
Clippy, CLI check and formatting passed. Full browser/Python/optimized release
suites were not repeated: no UI, Python, packaging or release behavior changed.
Windows/Linux native compilation and device IPC interactions remain unverified.
No dependencies were installed to qualify the catalog designs.
