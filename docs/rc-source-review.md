# RC source review — 2026-10-18

The release owner locked feature scope after [per-site PHP issue #5](https://github.com/eimg/fairway-xmp/issues/5) closed as not planned. The accepted feature baseline is `9d521506d9b6ce8e15ac4b216529595bda4487b0`. This pass changes reliability, safety, verification and release documentation within that scope; it adds no product features.

## Review coverage

Reviewed the first-party Rust service manager, process ownership/recovery, configuration and migration, local domains/HTTPS, CLI/supervisor/TUI, desktop IPC and shutdown/updater coordination, database worker and exports, mail HTTP bridge, logs, terminal lifecycle/backpressure, native trays, diagnostic allowlist, React screens/state/effects, installer hooks, runtime/source packaging, signing, candidate identity, acceptance and publication/feed gates. The existing tests were examined alongside these paths and extended for concrete regressions.

This is a first-party source and dependency review, not an independent audit of the bundled PHP/MySQL/Nginx/Mailpit source trees. Their versions and input locks remain unchanged under the existing runtime/provenance policy.

## Concrete findings addressed

| Finding | Correction and verification |
| --- | --- |
| Backup database names were parsed as escaped/trimmed text and could become mysqldump options. A leading `--` name could be omitted silently. | Read the name list as JSON and terminate option parsing before database arguments. Native backup/restore verifies an option-shaped name and a leading-space Unicode name, in addition to tables, views, triggers, routines, events and foreign keys. |
| Dropping every user database made automatic pre-update backup fail, preventing an otherwise safe upgrade. | Write an explicit empty SQL backup. Native database smoke verifies both that file and updater preparation with no user databases. |
| Attachment responses were accepted without checking their declared size; a truncated or capped response could be saved as a complete attachment. | Validate framing and exact Content-Length, reject unsupported transfer encodings and oversized responses. API previews are bounded to 8 MiB and attachment responses to 64 MiB. Regression tests cover incomplete, oversized and conflicting responses. |
| Slow mail searches or message opens could overwrite the user's newer selection, and polling could overlap slow calls. | Ignore obsolete responses and poll serially. Action errors remain separate from polling errors. Browser regression exercises deliberately reordered search and message responses. |
| Backslash-escaping `$` in Nginx paths still let variable-capable directives interpret it as a variable. | Distinguish literal filenames from variable-capable paths and use a literal dollar value in those directives. Native Nginx checks and service/restore tests cover dollar-containing roots, logs and HTTPS certificate paths. |
| An absolute MySQL socket filename could exceed the Unix socket address limit in a long application-data path. | Use `mysql.sock` relative to the managed server's data working directory. Fairway clients continue to use explicit loopback TCP. Native restore exercises the long path that originally failed. |
| The CLI log tail sought near EOF but could keep reading appended bytes; a special file could block it. | Limit the read to the snapshotted byte range and reject special files without blocking. Regression tests cover a large log and a FIFO. |
| Optional Mail was handled correctly by the manager/tray but could leave the main UI reporting partial readiness and offering an unusable Start action; diagnostics omitted its runtime version. | Align readiness and missing-runtime controls/messages, and include the allowlisted Mailpit version in support reports. Browser and diagnostic privacy regressions cover this. |
| The pinned updater TLS dependency was below the newly patched rustls release. | Narrowly update rustls 0.23.45 → 0.23.46; retain the other dependency versions and rerun native/source checks. |

| Runtime archive hashing read each large download fully into memory, sometimes twice. | Stream checksum calculation and reuse the SHA256 result; checksum-mismatch and packaging tests remain green. Compression and input pins are unchanged. |

## Dependency review

The npm audit returned no published vulnerabilities. The Rust lockfile audit queried OSV for 540 registry packages. The rustls update addresses [RUSTSEC-2026-0346](https://rustsec.org/advisories/RUSTSEC-2026-0346.html), [0347](https://rustsec.org/advisories/RUSTSEC-2026-0347.html), [0348](https://rustsec.org/advisories/RUSTSEC-2026-0348.html), and [0349](https://rustsec.org/advisories/RUSTSEC-2026-0349.html). Some require optional features or custom APIs that Fairway does not use; taking the compatible security patch avoids carrying them into the RC.

Remaining audit matches need context rather than a blanket “zero Rust advisories” claim:

- `glib` 0.18.5 has [RUSTSEC-2024-0429](https://rustsec.org/advisories/RUSTSEC-2024-0429.html). It belongs to the Linux desktop dependency graph. This release ships macOS/Windows desktop and a Linux CLI build without desktop features, so that GTK path is outside the shipped targets.
- `zip` 4.6.1 has Unix extraction advisories [RUSTSEC-2026-0367](https://rustsec.org/advisories/RUSTSEC-2026-0367.html), [0368](https://rustsec.org/advisories/RUSTSEC-2026-0368.html), and [0369](https://rustsec.org/advisories/RUSTSEC-2026-0369.html). In the updater this ZIP extraction path is Windows-only; Fairway distributes a signed EXE updater there and a signed tar archive on Mac, and verifies signatures before extraction/installation. No affected Unix ZIP extraction path was identified in shipped application code. Runtime ZIP staging uses Python's guarded extractor. A major ZIP/updater migration is not part of this patch.
- `proc-macro-error`, `zvariant`, `zvariant_derive` and `zbus_names` have upstream maintenance notices. They are not treated as fixed by this review. Revisit these with the upstream framework dependency graph before adding a Linux desktop target.

## Release gates and remaining work

Local validation: 57 all-features Rust library tests and 39 Python tooling tests passed; strict all-target/all-feature and CLI clippy, Rust formatting, workflow lint and version consistency passed. The full browser/native-bridge suite passed 210 tests with two existing WebKit clipboard skips; after the final optional-mail UI adjustment, 16 focused cross-engine/window checks passed. Native service/restore and data-browser smoke tests passed, including all six PHP versions, unusual backup names, empty-database updater preparation, dollar-containing HTTPS paths, and long data paths. The service probe now refuses a nonempty test root and cleans up even after assertions.

Record CI/candidate results against the reviewed commit. Build a fresh signed release candidate after source fixes are committed. The clean embedded identity, native service/restore checks, Mac notarization, updater drills, installer lifecycles, all supported targets and source locks must agree on that commit. Every later code/runtime change invalidates the earlier candidate's acceptance.

The exact candidate still needs human device acceptance, including beta 3 → RC preservation, Windows native behavior, protected Mac update prompts/cancellation, mail and logs, and normal/minimum window usability. Automated checks do not complete those fields. Do not tag/publish/promote an RC until the documented acceptance and owner publication authorization are in place.

Accepted limitations stay unchanged: unverified Windows PHP 8.0/8.1 dependency build provenance, no Windows Authenticode, documented Windows MySQL ANSI-path restrictions, shared PHP logs, recent bounded log history, and no continuous live log rotation. Mac Developer ID signing/notarization and cryptographic desktop updater signatures remain required.

Maintenance opportunities after the RC are narrower deadlines/cancellation for long-running external backup/export tools, concurrency hardening for rare independent CLI writes, pinning third-party Actions to reviewed commit hashes, and splitting large manager/configuration/UI modules when a concrete fix needs it. These are not new RC features or grounds for a broad refactor during the lock. The review does not prove absence of defects.
