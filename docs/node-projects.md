# Node Projects — registry and runtime isolation

This personal extension provides the registry, inspection, and runtime-preparation
backend on macOS. There is no Node Projects page or project execution yet. PHP sites,
service controls, ports, hosts, certificates, and existing Node Settings retain
their current behavior. Other platforms report `supported: false`; mutations
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
process validation is required again immediately before any future execution. Nothing auto-starts or stores a running PID.

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
There are no start, stop, restart, or log IPC commands in this stage.

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

A child environment is just a map supplied to one future subprocess. Updating
that map changes neither the app's environment nor the user's terminal. Each
project gets a separate map, so selecting Node 22 for one project does not change
another project's Node 24. Node is addressed by its absolute executable path;
PATH ordering also makes ordinary `node` commands find that installation first.

npm itself prepends project/ancestor `node_modules/.bin` paths to scripts' PATH.
Preparation therefore rejects a Node/npm shim in those locations unless it
resolves to the selected tool. See the [npm run documentation](https://docs.npmjs.com/cli/commands/npm-run/).
This is tool-selection isolation, not a security sandbox for future project
code. Scripts that explicitly invoke another executable remain project code.
Custom SDK paths, shell functions, and arbitrary exported application variables
are intentionally not imported by this first environment policy.

The validation receipt is not a launch token. `node_command()` only constructs
an unspawned Node command, without script arguments. Stage 3 must re-prepare
under its operation guard immediately before a managed start; files can change
after a successful preparation. Nothing in Stage 2 starts, supervises, or logs
a project process. Environment values are never serialized or persisted.

New failures use `runtimeChanged`, `npmUnavailable`, `environmentUnavailable`,
`requirementConflict`, and `unsupportedPackageManager`, alongside Stage 1 error
codes. Invalid paths, missing runtimes, non-executable binaries, malformed npm
metadata, stale registrations, and changed package files fail without changing
the saved selection.

## Verification and next stage

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

Stage 3 still needs the owned macOS runner, bounded logs, process identity and
cleanup, start/stop/restart operations, runtime-uninstall protection, and explicit
quit/update integration. The graphical project workspace remains later work.
PostgreSQL, Redis, and other optional services are not implemented here; this is
a separate optional module, not a generic plugin framework.
