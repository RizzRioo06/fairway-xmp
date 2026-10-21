# RC 3 source readiness review — 2026-10-20

Reviewed `ef1deb6` on `codex/rc-polishing`, compared with released application tag `v0.1.0-rc.2` (`3ace851`). The comparison contains 46 changed files. Dependency locks and runtime inputs are unchanged.

**Current recommendation: the original source findings below are resolved. The October 21 PHP security releases are now pinned for a fresh distribution candidate. Publication still requires cross-platform signed artifacts and exact-installer acceptance.** The original review found no P0/P1 issue. Its checkpoint findings and reproductions are retained below as historical evidence; the subsequent owner-authorized remediation changes application code and stages `0.1.0-rc.3`.

## Additional preparation — 2026-10-21

The owner authorized foreign-key SQL-example dialogs, a Table SQL modal, clearer reference wording, removal of the MySQL CLI examples, inline mail attachment images and external HTTP/HTTPS mail links. The preview retains script/form isolation with a hash-authorized interaction bridge. These changes need their own final candidate and device acceptance.

The PHP removal exposed a recovery defect: both manager construction and version saving validated the unavailable previous selection before users could replace it. The desktop now opens with the selection preserved and Settings available; the Linux supervisor accepts its existing PHP-version command. Service startup remains blocked. Switching requires stopped services, including live old PID markers, and failed recovery restores the previous selection, configurations and launchers. Unknown selections outside the removed branches and invalid settings still retain the original blocking behavior.

Runtime verification now rejects a stale staged manifest and unexpected PHP directories. Release notes now identify the patched PHP 8.2.35/8.3.36/8.4.27/8.5.12 pins. See the [PHP security update checklist](rc3-php-security-update.md) for required platform packages, lock/source updates and final build gates. The original checkpoint's unchanged-pin statements below are historical, not current security-update evidence.

Local preparation checks and remaining distribution gates are recorded in that checklist. Browser regression checks complement the owner's visual testing; neither establishes exact-installer acceptance.

## Remediation — 2026-10-20

| Finding/gap | Resolution and evidence |
| --- | --- |
| 1 — Recurring service errors | Retire a cleared current-service baseline after an observed recovery/message change. File history remains cleared. Unit coverage verifies identical refreshes, recovery → same failure, A → B → A, and preservation of the original baseline. |
| 2 — UPDATE counts | MySQL query connections use changed-row counts; form connections retain FOUND_ROWS for no-op optimistic saves. SQLite explicitly says rows matched for update. Native tests cover unchanged, mixed and repeated updates; the direct bridge check also verifies a no-op form save succeeds. |
| 3 — Unit tests outside CI | `npm run test:unit` discovers and runs the TypeScript helper tests with explicit file arguments on Windows/Unix. Ordinary verification, browser review, and both desktop candidate workflows run it. |
| 4 — Windows prompt | CMD uses a green arrow and native full-current-path expansion, updating after cd without startup scripts/hooks. Native Windows expectations and launch-construction coverage are updated; Windows execution remains a candidate gate. Mac retains its folder-name prompt. |
| 5 — Stale specification | Corrected terminal maximize wording and RC 2 SQLite/Structure shipping status; documented current counting, prompt and clear semantics. |
| Import-phase visibility | Native progress events report protective backup and importing. The dialog explains backup cancellation waits, shows cancellation/refresh state, and prevents further cancel actions during recovery refresh. Missing progress subscriptions retain a truthful combined status. |
| Native bridge coverage | Added shared production Jobs cancellation and progress events to the test bridge. External launch observation resolves the real shell directory without opening a host terminal. Native browser regressions cover import cancel/error/backup/reconfirmation and external paths after cd. |
| RC 3 preparation | All six version locations agree on `0.1.0-rc.3`; draft release notes/changelog/preparation are present. Dependency versions and runtime pins remain unchanged; only application version fields changed in locks. |

Remediation verification passed: 8 TypeScript unit tests, 58 Python tests, 67 Mac Rust library tests with all features, production TypeScript/Vite build, Clippy with warnings denied, rustfmt, actionlint, whitespace checks and expanded native MySQL smoke. A separate isolated stdio check passed changed-row counts, no-op form saves, real progress/cancel commands, retained backup, request recovery, and working-directory observation after a real cd into a Unicode folder. Changed UI/native test files were explicitly typechecked.

The local release-mode Mac app builds and its bundle reports `0.1.0-rc.3`. It is the unsigned local UI-test build, not a signed release candidate. Browser and native WebView UI tests were not executed locally, respecting the owner's request to handle visual testing. No Windows execution, Linux candidate, signed candidate, device acceptance or release publication is claimed. Disposable native test data was removed after successful shutdown.

## Original checkpoint findings

### 1. P2 — Clear view suppresses a recurring service failure

Locations: `src/log-view.ts:9`, `src/log-view.ts:15`, `src/LogsPage.tsx:109`.

Synthetic service entries always use `current:<service>` as their ID. Clearing records that ID and text permanently. If the service recovers, or reports a different error, and then the original error recurs, the new failure is still excluded because it matches the old clear baseline. This undermines the incoming-output debugging use case.

Reproduced directly through the production helpers: capture a baseline for MySQL with message `MySQL stopped unexpectedly.`, supply a healthy snapshot with no message, then supply the original failure again. Both snapshots return zero visible entries after clearing. The existing tests exercise changed messages but not recurrence after recovery.

Before candidate freeze: give service-message occurrences a generation, or retire the cleared service baseline when its message disappears/changes. Keep identical refreshes hidden, while allowing an observed new occurrence through. Add a regression covering failure → clear → recovery → same failure, and A → B → A.

### 2. P2 — “Rows updated” counts matched rows, including unchanged values

Locations: `src/data/query-results.ts:54`, `src/data/query-results.ts:60`, `src-tauri/src/data_browser.php:268`.

The new summary treats the MySQL affected count as an updated count. Every worker connection enables `PDO::MYSQL_ATTR_FOUND_ROWS`, which is needed by optimistic form edits but makes UPDATE return matched rows even when values remain unchanged.

Reproduced through the native Rust/PHP bridge against isolated bundled MySQL:

```sql
CREATE TABLE t(id INT PRIMARY KEY, value INT);
INSERT INTO t VALUES(1,10);
UPDATE t SET value=10 WHERE id=1; -- affected: 1; no value changes
UPDATE t SET value=11 WHERE id=1; -- affected: 1
UPDATE t SET value=11 WHERE id=1; -- affected: 1; no value changes
```

All three results become `1 row updated`. The current native label regression only updates a different value, so it misses this distinction.

Before candidate freeze: define whether UPDATE feedback describes matches or changed values. Either label the existing count as matched rows, or use changed-row semantics for query connections while preserving FOUND_ROWS for form writes. Do not globally remove FOUND_ROWS: no-op form saves currently depend on it. Include unchanged and mixed changed/unchanged rows in native coverage; account for SQLite's own count semantics too.

### 3. P2 — The new TypeScript unit tests are absent from automated release checks

Locations: `package.json:6`, `.github/workflows/build.yml:51`, `.github/workflows/ui-review.yml:25`.

`tests/log-view.test.ts` and `tests/query-results.test.ts` pass when invoked directly, but no package script or workflow runs them. The existing checks run the production build, Python tests, Rust tests, and Playwright. Typechecking alone does not execute their assertions, so a green candidate can omit these regressions completely.

Before candidate freeze: add a unit-test script and run it in the ordinary verification/candidate verification path. Keep these fast helper tests separate from optional native browser tests.

### 4. P3 — Windows retains the old terminal prompt

Location: `src-tauri/src/terminal.rs:384`.

The Mac private zsh configuration uses `➜  <folder name>`. Windows explicitly sets the visible CMD prompt to `$G ` (`> `), preceded by an invisible working-directory report. This is a feature-parity gap in the requested prompt change; working-directory tracking itself is present.

Implement an appropriate Windows equivalent and verify it in ConPTY, or explicitly retain/disclose the platform-specific prompt. This alone need not block an RC.

### 5. P3 — Old specification wording contradicts shipped/current behavior

Locations: `PROJECT_SPEC.md:102`, `PROJECT_SPEC.md:126`.

The terminal paragraph still says there is no expand/fullscreen mode, contradicting the shared maximize section. The SQLite paragraph calls the feature unreleased and outside accepted RC artifacts even though it was included in RC 2. These are stale statements, rather than leftover runtime implementations.

Update them when preparing RC 3 notes. Source searches found no retained terminal-height drag implementation or old external-terminal injection path in the current implementation. The table sidebar's horizontal resize is intentional and unrelated.

## Original checkpoint verification

| Check | Result |
| --- | --- |
| Production TypeScript/Vite build | Passed |
| TypeScript helper tests | 7 passed, run explicitly |
| Python packaging/release/SQLite suite | 57 passed |
| Rust library tests, all features, Mac | 67 passed |
| Clippy, all targets/all features, warnings denied | Passed |
| rustfmt | Passed |
| Workflow actionlint | Passed |
| Native MySQL smoke | Passed |
| Native unchanged-UPDATE reproduction | Reproduced finding 2 |
| Log recurrence helper reproduction | Reproduced finding 1 |
| Diff whitespace | Passed |
| Version consistency | All locations still agree on `0.1.0-rc.2` |
| Browser/native WebView visual testing | Not run; left to release owner as requested |
| New Windows/Linux candidate/installers | Not built or tested in this review |

The first sandboxed Rust run could not bind test sockets or inspect an owned child process. Repeating with the required access passed all 67 tests. These were environmental failures, not source findings.

The native MySQL smoke covers full-file imports, confirmation request guards, target backup creation, failure-before-import when backup fails, partial SQL errors, DELIMITER/procedures, cancellation, export/restore, CRUD, schema constraints and existing transaction/result limits. It uses disposable data and free ports; no installed user database was used. Its success does not establish OS-picker, WebView, or Windows client behavior.

No new dependency/runtime advisory assessment was performed. This review confirms unchanged locks and pins; it does not extend earlier advisory evidence to future releases.

## Candidate and publication gates

1. Findings 1–5 are resolved in source, with the native full-path Windows prompt choice documented. Retain their regression gates and freeze features; no broad component rewrite is needed for this RC.
2. `0.1.0-rc.3` and draft notes are staged. Commit the final reviewed source before candidate compilation. Keep historical RC 2 acceptance evidence unchanged.
3. Build a new candidate from the final commit through the existing cross-platform candidate workflow. Require its browser tests, native bridge tests, service/restore tests, packaging, Mac signing/notarization and updater signatures to pass.
4. Perform exact-installer acceptance on Mac and Windows, including normal/minimum-size maximize/restore; nested dialogs and keyboard focus; full terminal output with the current input line visible; first-tab restart/extra-tab closure; external terminal path with CLI integration enabled/disabled; SQL import success/error/cancel/backup recovery; SQLite source switching and protected-folder access; and mail HTML focus/script isolation. Repeat Linux install/upgrade/service checks even though its feature surface is unchanged.
5. Recheck the reported duplicate SQLite folder prompt using the signed candidate. The retained local `--no-sign` app fails strict signature verification (`code has no resources but signature indicates they must be present`), so it cannot establish how the signed distribution's identity/permissions behave. This is an open acceptance question, not a demonstrated signed-release defect.
6. Record exact candidate identity/hashes and pre-publication acceptance. Verify RC 2 → RC 3 upgrade preservation; after promotion, complete the actual in-app update/restart checks. Prior RC acceptance and exceptions do not automatically cover the new candidate.

## Remaining maintenance opportunity

- Continue using the shared workspace/modal layer and small pure helpers. Splitting the large DataBrowser component may help maintenance later, but is not a reason to broaden RC 3 scope.
