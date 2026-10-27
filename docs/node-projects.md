# Node Projects — Stage 1

This personal extension currently provides the registry and inspection backend
on macOS. There is no Node Projects page or project execution yet. PHP sites,
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
process validation are Stage 2 work. Nothing auto-starts or stores a running PID.

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

## Verification and next stage

Rust tests in `node_projects/tests.rs` and `node_projects/store.rs` use isolated
temporary directories and an injected inventory. Fake runtime files are never
executed. Coverage includes malformed/oversized files, Unicode paths, aliases,
missing runtimes/roots, stale revisions, unsupported platforms, private writes,
failed publication/retry, corrupt registry preservation, and unchanged PHP/fnm
files. TypeScript tests cover the serialized error contract.

Run the normal checks from `docs/testing.md`. Stage 2 still needs paired npm
resolution, child-only environment construction, runtime revalidation, and tests
for independently selected Node versions. Subsequent stages add the owned
macOS runner, bounded logs, process identity/cleanup, and explicit quit/update
integration, followed by the project UI and native browser execution tests.
PostgreSQL, Redis, and other optional services are not implemented here; this is
a separate optional module, not a generic plugin framework.
