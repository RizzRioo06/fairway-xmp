# Node creation — Stage 5 verification and acceptance fixes

Based on checkpoint `c9e614c`, on `feature/node-project-templates`, 2026-10-29,
macOS arm64. This is the creation stage, separate from the older Node Projects
integration report. All work used CLI/headless fixtures and disposable profiles.
Installed Fairway, personal projects, global Node defaults, main and upstream
were not modified. No PostgreSQL server/integration was started.

## Confirmed cause and fixes

The retained manual profile's `node-project-logs/.../events.jsonl` records
`my-react-vite` exiting 1 with **Error: Port 5173 is already in use**; its last-run
metadata selects Node 24.20.0. The other Vite project's stdout records
`Local: http://127.0.0.1:5173/`. These are actual prior run logs, read without
executing or modifying either personal project. A disposable native regression
also reproduces this exact failure by starting a strict-port second server against
the first fixture's occupied port. Nothing kills or reconfigures another listener.

Catalog/template revision 4 enables Vite's available-port fallback. Concurrent
JavaScript/TypeScript projects use distinct actual ports, serve their own project
HTML, stop independently and restart successfully on Node 22, 24 and 26. Existing
project configs are not migrated: an owner of an older strict-port project must
choose another port or change `server.strictPort` to `false` explicitly.

Native run status gains backward-compatible optional `reportedUrl` metadata.
A bounded, frame-aware stdout parser recognizes the generated API listening lines
and Vite Local lines, strips ANSI CSI colors, and validates HTTP(S) loopback
origins. External links, credentials, query strings, invalid ports, arbitrary log
links and oversized lines are rejected. No script/config is executed to discover
a URL. API sources now print the socket's actual port/address, including `PORT=0`
and IPv6 formatting. The existing explicit Start consent and runtime preparation
still apply to every execution.

Cards make the current run's reported URL clickable. Starting, stopping and
terminal generations cannot supply a previous reported link. Manual URL editing
while stopped remains available and labels its URL configured. Both sources state
that readiness has not been checked; output is advisory, not HTTP health or
listener-ownership verification. There is no invented default URL or Ready state.
Unknown output formats/wildcard binds need a manual loopback URL.

## Runtime qualification

Every row passed genuine pinned `npm ci --ignore-scripts`, its build/test script
where present, dev HTTP execution and owned Stop. API start scripts also served
`/health` from the actual kernel-assigned port after `PORT=0`. The test mirror
serves only SHA-512-verified tarballs from the prior public qualification cache.

| Template | Node 22.23.4 / npm 10.9.9 | Node 24.20.0 / npm 11.19.0 | Node 26.12.1 / npm 11.20.0 |
| --- | --- | --- | --- |
| Plain Node JavaScript | Passed | Passed | Passed |
| Plain Node TypeScript | Passed | Passed | Passed |
| Hono TypeScript | Passed | Passed | Passed |
| Express JavaScript | Passed | Passed | Passed |
| React/Vite JavaScript | Passed | Passed | Passed |
| React/Vite TypeScript | Passed | Passed | Passed |

Node 26 was qualified in disposable candidate manifests before expanding all six
catalog ranges to `>=22.12.0 <23 || >=24.0.0 <25 || >=26.0.0 <27`. npm remains
`>=10.0.0 <12`. Evidence covers the exact pairs above, not every allowed patch/npm
pair. No dependency pin, resolved graph, raw lock/provenance hash or license changed.
Generated lockfiles normalize only root name and engine metadata; all package
resolution/integrity entries remain intact. Fresh native plans include these exact
bytes. Existing durable journals retain their persisted manifests and receipts.

## Verification evidence

- Rust desktop/library: 198 passed; 20 opt-in/internal cases excluded in the normal
  run. Core/terminal: 187 passed, 17 excluded, zero doc-tests.
- Genuine native matrix: 18 template/runtime combinations passed, plus controlled
  integrity/503 failures with no publication.
- Concurrent Vite regression: passed with both registrations in one native Node
  controller on all three runtimes, including
  strict-port reproduction, distinct reported URLs/HTTP bodies and independent Stop.
- Native installer suite: 13 passed serially, including completion/registration retry, cancellation, stale IDs,
  duplicate requests, consent/tool/lock revalidation, runtime leases, source/inode
  races, bounded logs, host SIGKILL cleanup, interrupted recovery, descendant drain,
  unavailable registry and PHP/MySQL/updater lifecycle coexistence.
- Existing native Node execution: five opt-ins passed (concurrency/restart, logs,
  forced cleanup, host death, Quit/update/PHP independence).
- TypeScript units/contracts/template syntax: 32 passed. Headless Chromium/WebKit
  creation/workspace: 48 passed; minimum-size follow-up: 8 passed. Final clickable-text/layout check: 4 passed across both engines and sizes.
- Python regressions: 87 passed. Real PHP/Nginx/MySQL/Composer/mail/backup smoke
  passed with all four bundled PHP branches (8.2–8.5), fresh database restoration,
  authentication/recovery, coursework preservation and graceful shutdown.
- TypeScript/Vite production build, strict all-target/all-feature Clippy and
  formatting/whitespace checks passed. The macOS release executable built and the
  non-desktop CLI configuration checked successfully; neither was installed.

Counts overlap and must not be added. The internal generation crash-child is
exercised by its parent test. Four remaining native opt-ins (standalone planning/local generation, two-runtime
isolation and scratch login-profile timeout), runtime downloads, PHP extension/settings
long-response and unrelated full-browser suites were not repeated; affected execution, service and filesystem paths were covered above.

The initial sandbox run could not bind loopback; the same authorized fixtures ran
with normal local socket/process access. The first Node 26 candidate harness used
a TMPDIR exceeding macOS's Unix socket path bound for tsx; shortening only the
harness's disposable path resolved it. An initial parallel native installer run
hit preparation journal-lock contention (and its diagnostic read had no run log
yet). Matrix and concurrency tests passed independently. The native driver now
runs cases serially, while its concurrency test still runs real simultaneous
servers. The precise cause of that parallel fixture contention is unproven; no
backend locking protection was relaxed to bypass it.

Evidence logs are disposable `/tmp/fw-creation-stage5-*.log`, including node26,
matrix, shared-concurrency, native-final, existing-node, rust, terminal, unit, browser,
minimum, clickable-final, python, services, clippy-final, cli and release. They are not committed artifacts.

## Acceptance and release limitations

Real Mac GUI acceptance remains partial: the earlier user run covered Hono and
React/Vite TypeScript with Node 24. Remaining work is in the
[isolated manual guide](node-creation-manual-testing.md): fresh simultaneous Vite
projects, exact Open URLs, Node 26, native picker/focus, reload, cancellation and
Quit/relaunch. Headless browser fixtures simulate native operations; they do not
establish Mac WebView/dialog behavior or launch a real external browser.

Windows/Linux native generation/install/execution, other CPU architectures,
Vite preview concurrency, arbitrary imported output formats, signed installers,
actual updater replacement, disk-full/power-loss behavior, forced five-minute
installer expiry and deliberately detached processes remain unverified. Parallel
native fixture execution remains unqualified. Isolation is not an OS sandbox.
One install runs at a time; uncertain dependency stages/caches are retained, and
retry requires a fresh reviewed workspace. Existing personal projects and older
strict-port configs remain unchanged. No merge or release publication is implied.
