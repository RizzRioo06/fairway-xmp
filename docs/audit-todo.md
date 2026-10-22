# Audit remediation status

## Current status — 2026-10-22

The implemented audit fixes shipped in [RC 3](https://github.com/eimg/fairway-xmp/releases/tag/v0.1.0-rc.3). [Candidate 42](https://github.com/eimg/fairway-xmp/actions/runs/39710252813) passed the native platform, browser, service/recovery, installer, signing and updater checks. The owner confirmed the standard pre-publication device checks; see [exact-installer acceptance](release-acceptance/0.1.0-rc.3.json).

The October 21 verification paragraphs below are historical checkpoints. Their then-pending hosted checks and terminal failures are superseded by candidate 42. Actual subsequent-version Mac/Windows update and restart, including protected Mac authorization prompts, remain pending. Detailed privileged Linux trust/browser-verifier cases and individually unreported native prompt combinations remain verification follow-ups; the general device report does not establish those specific cases. Pass 11 stays deferred.

Source: audit of `a3984df` and targeted code inspection. The implemented passes were grouped by shared behavior, with focused regression checks and reviewable diffs. Deferred work below is separate from the completed RC 3 implementation.

Preserve coursework, databases, credentials, supported workflows, and user choices. Do not silently reset settings, change PHP versions, force-kill MySQL, or add database naming restrictions. Reproduce uncertain findings before changing behavior. Update this checklist with changes, evidence, and remaining limitations after each pass.

## Pass 1 — Linux installation and uninstall safety (implemented; candidate verified)

- [x] Remove only managed installation files; retain other files within custom prefixes. Retain an installation marker when user files remain so safe prefixes can be reused on reinstall.
- [x] Reject installation prefixes overlapping the document root or application data, including aliases through symlinks and parent traversal.
- [x] Refuse uninstall when managed payload deletion would overlap protected user paths or the installation layout is invalid. Reject symlinked management paths during installation.
- [x] Cover preservation, cleanup failures, extracted installs, unsafe paths, and reinstall with isolated shell fixtures; extend the real Linux lifecycle check.
- [x] Record local verification and synchronize the product specification.
- [x] Run the extended installed-CLI lifecycle check on Linux with the real bundled runtime (candidate 42).

Verification on 2026-10-21: `python3 -m unittest discover -s tests -v` passed all 69 tests; `sh -n packaging/linux/install.sh packaging/linux/uninstall.sh` and `git diff --check` passed. Shell fixtures use isolated temporary homes and a stub Fairway executable; on macOS, adapters provide GNU `realpath -m` and `mv -Tf` behavior. On Linux the tests use native coreutils. No real services, shell profiles, hosts entries, or certificate trust were changed. The extended real Linux lifecycle check was not run locally: this host is macOS and its Docker daemon is unavailable.

## Pass 2 — MySQL initialization and recovery (implemented; candidate verified)

- [x] Use secure, non-listening initialization and a private one-shot SQL file to apply the saved root password before normal startup. Remove the SQL file on success/failure and redact the saved password from setup errors.
- [x] Resume readiness/provisioning on Retry for tracked, unprovisioned instances without respawning them. Report an unprovisioned listener as Waiting, including after process recovery.
- [x] Gracefully stop owned unprovisioned instances, with explicitly passwordless compatibility shutdown only when the completed-provisioning marker is absent. Preserve ownership after shutdown failures and keep provisioned databases out of this fallback.
- [x] Preserve existing data/credentials and isolate private database clients from personal login-path settings.
- [x] Add eight Unix fixture regression tests for delayed readiness/timeouts, partial account setup before/after password assignment, secret cleanup/redaction, failed shutdown, retries, provisioned authentication failure, and Start-all rollback.
- [x] Add a focused native MySQL recovery smoke example and include the same checks in the existing full service smoke suite used by platform CI.
- [x] Verify real bundled MySQL and the complete service flow on macOS; synchronize the product specification.
- [x] Run the extended real-runtime smoke suite on Windows and Linux (candidate 42).

Verification on 2026-10-21: all 64 Rust core/terminal tests passed with `cargo test --manifest-path src-tauri/Cargo.toml --no-default-features --features terminal --locked --offline`. Clippy passed for all targets/all features with warnings denied; formatting and `git diff --check` passed. Both `mysql_recovery_smoke` and the full `smoke` example passed against bundled MySQL 9.7.2 on macOS. Native checks verified passwordless authentication rejection, recovery/provisioning of the same PID, legacy passwordless graceful shutdown/restart, stored credential and coursework preservation, and refusal to apply the fallback to a provisioned database. The full smoke also verified PHP 8.0–8.5, PHP → MySQL → HTTP, phpMyAdmin, logical backup restoration, and graceful shutdown. Tests used temporary homes/data and loopback ports; installed Fairway data, system hosts, and certificate trust were not changed. Windows/Linux runtime results remain unverified locally.

## Pass 3 — Process ownership and port checks (implemented; candidate verified)

- [x] Revalidate adopted process identity on macOS using process start time and on Windows using a retained process handle; preserve Linux start-time checks. Fail closed when adopted identity cannot be established.
- [x] Capture identity before executable/configuration validation and recheck it before adoption. Check liveness/identity again immediately before direct signals and web-process termination.
- [x] Detect IPv4 wildcard and IPv6 dual-stack port listeners on macOS in both preflight and alternative-port suggestions. Preserve IPv6-only coexistence and normal port-reuse behavior; never reassign ports silently.
- [x] Add regression coverage for stale/unverifiable identity, exit during adoption, safe signal rejection, retained Windows handles, wildcard listeners, and IPv6-only coexistence.
- [x] Verify native macOS recovery of the same Nginx/PHP/MySQL processes and normal Stop/Start behavior; synchronize the product specification.
- [x] Run native Windows and Linux process/recovery checks (candidate 42).

Verification on 2026-10-21: all 69 Rust core/terminal tests passed. Clippy passed for all targets/all features with warnings denied; formatting and `git diff --check` passed. `service_recovery_smoke` recovered the same Nginx/PHP masters after a controller crash, rejected foreign configuration/unrelated PIDs, and verified Stop/Start. `mysql_recovery_smoke` passed with the new adoption checks. The actual `process.rs` and its tests also typechecked for `x86_64-pc-windows-gnu` and `x86_64-unknown-linux-gnu` in a temporary harness with a minimal layout stub and the locked platform dependencies. Native Windows/Linux runtime validation remains pending. All running checks used disposable processes, temporary data, and random local ports.

## Pass 4 — Shutdown reliability (implemented; native recovery verified)

- [x] Add an ownership-checked Nginx shutdown fallback: graceful SIGQUIT on Unix, existing web-process termination on Windows; retain the normal bounded wait/escalation and graceful-only MySQL shutdown.
- [x] Offer Keep open / Quit anyway after failed graceful shutdown. Explicit exit preserves remaining services, closes embedded terminals, and permits recovery next launch. Normal successful Quit still stops services first.
- [x] Preserve Windows web services through explicit exit using named per-installation/service jobs, PID markers, retained process handles and verified job membership for recovery.
- [x] Add an Nginx control-failure regression and extend the full native smoke to verify recovery of every running service's same PID after leave-running.
- [x] Run retained-job recovery on native Windows (candidate 42).
- [ ] Record both native shutdown-dialog choices on macOS and Windows individually; the standard device acceptance report does not describe each failure-dialog case.

## Pass 5 — Backup durability (implemented)

- [x] Flush logical export contents before publication and flush parent directory metadata on Unix; use write-through publication on Windows. Preserve existing backups and report export/publication/flush errors.
- [x] Stage physical upgrade copies under unique .partial directories, flush the copied tree, publish it, then save and flush the completion journal. Failed/interrupted copies do not authorize database startup; retries preserve originals and create fresh copies.
- [x] Cover failed dumps/catalog reads, empty/successful exports, publication failure, interrupted/failed physical copies and recovery retries. Verify a real logical backup restores into a fresh database.

## Pass 6 — Startup and update operation recovery (implemented; native prompts pending)

- [x] Present settings/PHP-selection/certificate/configuration errors in a recovery window with persistent error details, data-folder access and Retry startup. Preserve invalid files, credentials and saved choices. CLI-refresh errors leave the manager usable; retry does not replace a working manager.
- [x] Keep services, terminals and database controls usable during update download/signature verification. Prevent duplicate downloads and wait for existing guarded work before exclusive installation.
- [x] Cancel pending download requests and verified-update waits on Quit; preserve atomic installation/shutdown ordering and retries after cancellation or installation/shutdown failures.
- [x] Cover startup failures and external repair, download cancellation, guarded-work ordering, installation retry and frontend recovery/progress without disabling normal controls.
- [x] Complete signed installer/updater lifecycle acceptance on Windows/macOS (candidate 42).
- [ ] Complete actual subsequent-version update/restart and native authorization/quit prompt checks after publication on Windows/macOS.

Batch verification on 2026-10-21: all 90 Rust library tests passed with all features; all-target/all-feature Clippy passed with warnings denied. The full bundled-runtime macOS smoke passed, including same-PID leave-running recovery, PHP 8.0–8.5, PHP → MySQL → HTTP, phpMyAdmin, restoration of a logical export into a fresh database, and graceful shutdown. Process/durability modules and their tests typechecked for Windows and Linux in the focused temporary harness; native Windows/Linux runtime execution remains pending. Tests used disposable processes, fresh temporary data and loopback ports, without changing installed application data, system hosts or certificate trust.

Frontend verification: all 36 focused startup, update-failure/progress and operation-notice checks passed on Chromium and WebKit at normal/minimum sizes. The production TypeScript/Vite build, formatting and diff checks also passed. The broader notices/workspace run passed 42/48 checks. Logs Escape failed in all four projects; Mail iframe Escape failed in both WebKit projects. Both failures also reproduced with the original App component, and are recorded for pass 7 instead of being folded into this batch.

## Pass 7 — Logs, terminal, and frontend correctness (implemented)

- [x] Fix closed Log options menus consuming the next Escape; capture maximize triggers before they hide and restore focus after the DOM commit. Preserve nested-menu/dialog dismissal and workspace state.
- [x] Verify Mail preview/restore and external links with an opaque sandbox and hash-pinned interaction bridge. Email scripts/forms stay blocked by a restrictive frame CSP; the bridge forwards Escape and boundary Tab in Chromium/WebKit, and native code validates HTTP/HTTPS links before opening the default browser.
- [x] Give backend service messages per-service occurrence generations, including identical failed retries. Clear view hides only cleared occurrences even without an observed recovery while paused, off-page or hidden; retained paused history stays cleared and file history is untouched.
- [x] Use Cmd for Find/text size on macOS and Ctrl+Shift on other platforms; preserve plain Control keys for shells and update hints. Cover shifted physical zoom/reset keys and actual Control+F delivery to the terminal bridge.
- [x] Reject UNC/device/relative/control-character Windows terminal directory reports lexically before filesystem access, and repeat the check at external-terminal entry. Retain ordinary local Unicode drive paths and the last valid directory after rejected output.
- [x] Add scoped render recovery and explicit lazy-load/app recovery without automatic renderer reloads. A failed maximized view releases modal isolation; retrying Data browser keeps a healthy sibling terminal mounted. Reload warns about terminal closure. Disable JavaScript modulepreload to avoid WebKit's failed-preload cache poisoning; retain lazy imports and CSS loading and verify manual recovery from an uncached HTTP 503.
- [x] Show every native explicit-action failure, including identical new tray/quit attempts, while retaining background-event deduplication after dismissal.
- [x] Read the current data source after pending work settles, including the gap before React commits a source switch; prevent stale MySQL catalogs from replacing SQLite while retaining sidebar-return refresh and MySQL selection memory.
- [x] Include TypeScript unit/UI/native test sources and Playwright configuration in the production typecheck. The expanded check has no errors on the current tree.

Verification on 2026-10-21: all 91 Rust library tests and 10 TypeScript unit tests passed. All-target/all-feature Clippy passed with warnings denied; the expanded TypeScript/Vite build, formatting and diff checks passed. All 20 new browser failure-path checks passed across Chromium/WebKit at normal/minimum sizes, alongside all 16 workspace checks and 24 operation-notice checks. The complete affected UI run passed 194/204 checks, including all logs, MySQL/SQLite, startup/update and new recovery checks. Tests used controlled browser bridges and disposable local Rust fixtures; no installed application data, hosts entries, trust settings or real database contents were changed. Native Windows external-terminal behavior still needs platform acceptance; lexical parsing tests exercise Windows path cases on this host.

The three terminal follow-ups from this checkpoint are resolved: fitting measures the actual available viewport, New terminal exposes its button role and disabled state, and popovers no longer trigger browser scroll anchoring. The 60 terminal/workspace regressions passed locally; candidate 42 passed browser review and native terminal checks on both desktop platforms. See the [RC 3 review](rc3-readiness-review.md).

## Pass 8 — Database browsing and editing correctness (implemented; candidate verified)

- [x] Reproduce/fix latin1 comparisons, BIT representation, and Unicode identifier lengths.
- [x] Verify the DOUBLE precision finding before changing numeric handling.
- [x] Use indexed row identity and reliable optimistic concurrency checks; preserve stale-write rejection, NULL semantics, and binary read-only rules.
- [x] Support large cells with bounded previews/full-value retrieval and compact edit requests as needed; do not merely raise payload caps.
- [x] Verify with real MySQL/PHP, including no-op saves, concurrent changes, and unrelated-row locking.

- [x] Document limits, full-value/copy behavior, native staging and remaining platform acceptance.
- [x] Repeat the native runtime/staging checks on Windows (candidate 42).
- [ ] Record platform-native WebView/clipboard edge cases separately when exercised; the standard device acceptance report does not enumerate these cases.

Reproduced every reported finding against bundled MySQL/PHP before changing behavior: latin1 comparisons failed against UTF-8 parameters, BIT values produced odd-width hex, a DOUBLE displayed as `1.2345678901235` failed its own stale check, a 4 MiB original row exceeded the request cap, and 64-character Unicode identifiers were counted as bytes. Added a related reproduced FLOAT-key case: MySQL's six-digit text formatting failed to locate its own key and could hide nearby stored changes; exact widening now preserves numeric identity and fingerprints.

MySQL now uses SQL-side UTF-8/numeric text conversion and byte-padded BIT hex. A compact non-null visible unique key selects/locks the row through an indexed predicate; a full-value/schema fingerprint rejects stale updates/deletes and is repeated in the mutation. Rechecks under the metadata lock protect schema races. Unchanged large fields stay on the server; labeled previews, bounded full-value chunks and read-in-parts/copy actions replace oversized page/original payloads. Form/copy text is bounded at 16 MiB. Large changed text uses native-created private temporary files with cleanup on success, failure and cancellation; worker JSON/response caps stay at 2/8 MiB. NULL/empty, generated/binary read-only, keyless/view safeguards and SQLite's existing protocol remain covered.

Verification on 2026-10-21: all 94 Rust library tests and 14 TypeScript unit tests passed. The existing real `data_browser_smoke` and new `row_edit_smoke` passed in disposable homes/data on macOS, including PHP 8.0–8.5, multi-megabyte Unicode round trips, stale tail/binary/NULL/schema changes, no-op saves, composite/numeric keys, keyless/view reads, concurrent row locks, private-path refusal and staged-file cancellation cleanup. All 108 affected browser checks passed across Chromium/WebKit at normal/minimum sizes, including eight real Rust/PHP/MySQL UI flows. All 28 focused value-rendering/cache/copy and native checks also passed after the final safeguards. All-target/all-feature Clippy with warnings denied, TypeScript/Vite build, formatting and diff checks passed. Native checks touched isolated test data and loopback ports; installed data, hosts and trust were not changed. Windows runtime and platform-native WebView/clipboard acceptance remain pending; prior terminal/fixture failures recorded under pass 7 are unchanged.

## Pass 9 — HTTP, certificate, and renderer boundaries (implemented; native platform acceptance pending)

- [x] Reject unknown HTTP hostnames while retaining localhost, phpMyAdmin, registered sites, custom ports, and HTTPS routing. Reject unknown HTTPS SNI and Host values as well.
- [x] Validate CA contents inside the privileged Linux helper; require `.test` DNS constraints, all-IP exclusions, TLS-server usage and zero subordinate depth. Read bounded regular certificate bytes once; reject substitutions, bundles, malformed/signature-invalid/broad roots, special files and symlinks.
- [x] Add staged CA/certificate/trust migration with durable generation/retirement state, per-profile material locking, cancellation/reload failure recovery and exact old-certificate cleanup. Preserve enabled HTTPS choices; validate cached leaf keys, issuers and names before reuse.
- [x] Narrow Tauri permissions and CSP; verify mail previews, React dialogs, native external-open requests and sidebar navigation.
- [x] Document the migration, privileged-helper boundary, verification and actual verifier differences in `docs/security-boundaries.md`; extend the site acceptance checklist and product specification.
- [ ] Repeat privileged Linux trust operations and Windows/macOS native trust migration, approval cancellation, native pickers/WebViews and browser negative-chain checks in disposable platform accounts. Verify the documented root-depth limitation with each supported verifier; do not infer client enforcement from encoded extensions alone.

HTTP default servers now reject unknown hosts on both web service ports. Known localhost/127.0.0.1, registered `.test` sites, custom ports and HTTPS remain covered. The elevated Linux helper validates the self-signature and complete restricted profile before storing any bytes, uses independent fingerprinted anchors and exact-content removal, and supplies root-owned temporary copies to p11-kit with a cleared privileged command environment. Existing certificates remain usable until replacement approval succeeds and Nginx applies the new configuration; unfinished retirement remains retryable. Root/key material is not silently replaced at startup. Sites exposes Update certificate trust only when an update/retirement is needed.

The renderer uses `core:event:default`; production CSP explicitly denies objects, base overrides and browser form navigation while retaining local frames, styling, data images and explicit native commands. The new production-CSP browser checks exercise hostile mail and blocked navigations alongside normal reading, forms and native opens. One existing site-terminal test incorrectly matched duplicate Connected labels; its assertions now target the terminal page heading, without product changes. Prior terminal-specific failures recorded in pass 7 were outside this run.

Verification on 2026-10-21: all 103 Rust library tests and 14 TypeScript unit tests passed. The expanded real runtime `smoke` passed in disposable macOS homes/data with PHP 8.0–8.5, phpMyAdmin, registered HTTP/HTTPS sites, valid/custom-port hosts, unknown Host/SNI and absolute-URI rejection, databases, mail, backup/restore and shutdown. The broader Chromium/WebKit normal/minimum-size run passed 178 checks with two pre-existing WebKit clipboard cases intentionally skipped, including all 12 production-CSP/migration checks. All-target/all-feature Clippy with warnings denied, TypeScript/Vite build, formatting and diff checks passed. Root-policy/helper logic, Linux anchor behavior and approval/cleanup failures were tested with isolated files and injected trust callbacks; no real trust-store or hosts modifications occurred. Native platform trust prompts/stores and privileged Linux execution remain pending.

Modern OpenSSL rejected public DNS, IPv4/IPv6 and subordinate-CA chains while accepting `.test` leaves. Apple's bundled LibreSSL 3.3.6 rejected DNS/IP violations but accepted the subordinate chain despite root pathLen=0. This concrete verifier limitation is documented and retained in platform acceptance; encoded constraints are not reported as universal client enforcement.

## Pass 10 — Signing safeguards and reproducible tooling (implemented; hosted execution verified)

Scope reduced by the owner on 2026-10-21: finish the practical signing/build safeguards, then defer compatibility-sensitive isolation. These choices follow the local classroom use case and the value of protecting distributed app/update signatures; they are independent of release milestones.

- [x] Compile applications, dependencies, helpers and lifecycle probes without signing secrets. Transfer unsigned inputs to fresh packaging/signing runners; never restore build caches or execute dependency lifecycle scripts there.
- [x] Restrict signing to exact commits in fetched `origin/main` history. Resolve standalone comparison refs before checking out their code, and recheck the exact build commit on the signing runner.
- [x] Import Apple identities only for signing/notarization; clean up immediately afterward, including failures and partial imports. Execute bundled PHP/app checks after cleanup. Supply updater keys only to the direct, version-bound signer command.
- [x] Lock Tauri CLI 2.12.2 and its platform binaries in npm dependencies; remove ad hoc CLI downloads from builds/signing.
- [x] Grant repository write permission only to publication jobs. Compile the publication signature verifier in a separate read-only job; preserve native installer/updater lifecycle and signature validation.
- [x] Cover source-ref policy, credential cleanup failures, deferred runtime checks, workflow boundaries and prebuilt publication verification. Document the revised workflow.

Verification on 2026-10-21: all 77 Python script/packaging tests passed, including eight new signing-policy/cleanup/packaging checks. All workflows passed actionlint; shell syntax, TypeScript/Vite build and diff checks passed. The locked CLI signed disposable installer/archive fixtures; the native verifier accepted exact bytes/version and rejected tampering and version substitution. No real signing credentials, keychains, trust stores, installed data or publication were used. Actual GitHub-hosted Windows/macOS packaging, Apple signing/notarization and the preserved native installer/updater drills still require a run after the reviewed changes reach main; local fixtures do not establish hosted acceptance.

## Pass 11 — Deferred isolation work (no active implementation pass)

The following are optional future work, not requirements for finishing this audit's implementation scope:

- **Teaching-account privilege changes:** defer until a concrete need justifies migration and coursework/phpMyAdmin compatibility testing. Preserve current credentials and database naming freedom.
- **Unix FastCGI sockets:** defer unless isolation from other OS accounts on shared machines becomes a supported requirement. Retain the current loopback transport and PHP switching/recovery behavior.
- **Shared-machine hosts/CA/helper ownership redesign:** defer until shared lab use is required or a reproducible collision establishes a practical fix.
- **GitHub Action commit-SHA pinning:** defer as ongoing supply-chain maintenance; locked CLI tooling and job/credential boundaries are implemented above.

## Remaining platform acceptance

Earlier passes retain their unperformed native checks. They are verification follow-ups, not authorization to expand the isolation scope.

- [ ] Verify optional HTTPS in supported Linux browsers using a disposable account, alongside pass 9's privileged trust/migration acceptance. Fix a demonstrated trust gap if one appears; do not introduce speculative trust-store changes.
- [x] Run the revised hosted signing/packaging workflows and their existing installer/updater drills (candidate 42 and the successful RC 3 publication).

## Deferred maintenance

Module splitting, PHP-worker deduplication, a service enum, and a general typed-error rewrite are deferred unless directly needed for a concrete fix or recovery action above.

## Interactive follow-up — Resume site address setup after macOS approval

- [x] Treat required background-helper approval as pending setup instead of a failed site save; show Waiting for macOS approval and an Open System Settings action.
- [x] Automatically finish an explicitly requested address update when the helper becomes enabled, using the current saved domains and normal operation guard. Keep unrelated controls usable while waiting and never reopen Settings or take focus from background continuation.
- [x] Persist only the explicit pending-approval intent across restarts. Clear it on completion or a genuine write failure; retain the existing manual retry and avoid automatic repair of unrelated hosts differences.
- [x] Cover pending/approved/revoked statuses and failures with injected callbacks, persistence/latest-site loading with temporary profiles, and browser save/approval/retry/removal flows.
- [ ] Verify the actual System Settings approval-to-hosts continuation and restart path in a disposable macOS account using the updated signed app. Tests do not alter the installed helper or real hosts/trust state.

Verification on 2026-10-21: all 107 Rust library tests and 14 TypeScript unit tests passed. The new approval/CSP browser run passed 28 checks; existing site/HTTPS/readiness/terminal flows passed another 34, with two pre-existing WebKit clipboard cases intentionally skipped. All-target/all-feature and CLI Clippy passed with warnings denied; production TypeScript/Vite builds, formatting and diff checks passed. Native status/application callbacks were injected and pending-intent storage used temporary profiles; browser approval events were simulated. Real System Settings approval, service registration and system hosts/trust contents were not changed by these checks. The actual approval-to-hosts continuation remains native acceptance, not a claimed automated pass.

### Follow-up — Explicit approval recheck and observed native blocker

- [x] Add a visible Check approval control alongside Open System Settings, with neutral feedback when the OS still reports approval missing and automatic dismissal after completion.
- [x] Explicitly refresh the app's helper registration before a manual recheck; preserve OS approval, existing registration and the prompt-free background monitor.
- [x] Reproduce the native pending state through read-only system records: current copy pending/disallowed, older copy allowed, current launchd service absent. Record the separate unnotarized Gatekeeper rejection and avoid claiming a proven single cause.
- [x] Verify manual recheck completion, still-pending feedback and dismissal, registration status changes and existing notices. All 109 Rust tests and 48 Chromium/WebKit checks passed; all-feature/all-target Clippy, production TypeScript/Vite build and diff checks passed.
- [x] Ship the corrected app through the signed/notarized RC 3 candidate and retire the obsolete local unnotarized test copy. The owner confirmed standard Sites/HTTPS device acceptance; separately unreported prompt combinations remain follow-ups above.
