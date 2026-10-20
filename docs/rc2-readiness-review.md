# RC 2 readiness review — 2026-10-20

**Source remediation: all six findings have been addressed for staged 0.1.0-rc.2.** A fresh signed cross-platform candidate and exact-installer acceptance are still required before publication. The findings below preserve the original checkpoint and reproductions; they describe the pre-fix behavior. The existing RC 1 publication is unaffected.

## Remediation — 2026-10-20

| Finding | Change and regression coverage |
| --- | --- |
| 1 — SQLite index dependency | Compare complete referenced/index column multisets rather than order. A real SQLite test refuses removal and verifies child writes still work. |
| 2 — Mac candidate packaging | Normal candidate builds explicitly enable both native permission features and copy the helper before final signing. Embedded build identity exposes the flags. Packaging requires both flags, the fixed executable/plist payload and the exact helper Developer ID. The signed/notarized candidate checks fresh registration/cleanup without approval or hosts writes. Missing flags/payload, altered capability, symlinks and signing-command guards have regression tests. Local app compilation and feature/payload verification passed; signed distribution and approved-helper lifecycle acceptance remain pending. |
| 3 — MySQL transactions | Drain/close the cursor, reject unfinished transactions and roll them back with an explicit error. Native smoke covers START TRANSACTION, autocommit disabled, explicit COMMIT/ROLLBACK, errors during a transaction, and errors after a commit. |
| 4 — MySQL result limit | Ten is explicitly a display cap. Drain all submitted statements, report omitted result sets, and propagate later SQL errors. Native smoke verifies eleven persisted writes, COMMIT beyond the display cap, and a later error. Production UI coverage checks the display notice and ten visible results. |
| 5 — Failed table/view recovery | Publish the new catalog and selected table with empty data and an inline retry error; retain other tables and SQL repair. Native browser regressions exercise both engines, initial SQLite opening, selection changes, no stale rows and repair through SQL. |
| 6 — Formatting | Applied rustfmt; the required check passes. |

Remediation checks passed: production TypeScript/Vite build; 57 script/database/packaging tests; 64 Rust library tests with all features; Clippy with warnings denied; rustfmt; actionlint; all 270 ordinary browser tests; and 16 production-UI/native bridge checks across Chromium and WebKit, plus four targeted recovery checks at the minimum window size. The expanded native MySQL smoke passes unfinished/committed/rolled-back transaction and omitted-result regressions. A local release-mode Mac app builds and its embedded flags and copied helper payload pass the new verifier. Signed/notarized distribution, exact-installer cross-platform CI and human approval/helper upgrade checks are still pending.

All six version locations now report `0.1.0-rc.2`; runtime pins are unchanged. [RC 2 notes](../releases/0.1.0-rc.2-notes.md), [preparation](rc-preparation.md), the specification and acceptance checklist describe the staged behavior. The application is not published and no human acceptance has been marked complete.

## Reviewed checkpoint

- Branch `main`, HEAD `0b1dd6c`, compared with `v0.1.0-rc.1`.
- Includes the existing uncommitted changes in `src-tauri/src/data_browser_sqlite.php`, `src-tauri/src/desktop.rs`, and `tests/test_sqlite_browser.py`.
- Review covered the post-RC changes, database workers and native boundary, permission/helper lifecycle, notifications/navigation, service/update safeguards, candidate packaging, and acceptance requirements. MySQL findings below also affect the existing implementation; they are not all introduced after RC 1.
- No application code was changed during this review. This report is a checkpoint, not release acceptance.

## Findings

### 1. P1 — SQLite permits removing an index required by a foreign key

Location: `src-tauri/src/data_browser_sqlite.php:88`.

The dependency guard compares index columns and referenced columns in exactly the same order. SQLite accepts a foreign key referencing a permutation of the columns in a composite unique index. That valid dependency is missed, so Structure exposes Remove and the backend accepts it, leaving the child table unusable for subsequent writes.

Reproduced against bundled PHP/SQLite:

```sql
CREATE TABLE parent(a TEXT NOT NULL, b TEXT NOT NULL);
CREATE UNIQUE INDEX uniq ON parent(a,b);
CREATE TABLE child(x TEXT, y TEXT,
  FOREIGN KEY(x,y) REFERENCES parent(b,a));
INSERT INTO parent VALUES('a','b');
INSERT INTO child VALUES('b','a');
```

Structure reports `uniq.removable = true`. Removing it through the form worker succeeds. The next insert into `child` fails with `foreign key mismatch - "child" referencing "parent"`.

**Before RC 2:** protect the applicable unique index regardless of this permutation. Keep the check inside the existing write transaction and add a regression that verifies both inspection and backend refusal. The automatic backup limits recovery cost but does not make the deletion safe.

### 2. P1 — The normal Mac candidate omits the intended native permission changes

Locations: `.github/workflows/macos-test-build.yml:129`, `src-tauri/Cargo.toml:31`, and the standalone-only helper build at `.github/workflows/macos-test-build.yml:248`.

The release app builds with default features. `native-macos-trust` and `native-macos-hosts` are optional, and only the standalone permission build enables them and copies the helper's Library payload into the app. Consequently, cutting RC 2 with the current normal workflow would ship the compatibility permission paths. In particular, the in-process certificate-removal fix would be absent. Enabling the hosts feature alone would also be insufficient without its helper executable and plist.

**Before RC 2:** integrate the chosen features and helper payload into the actual candidate path before final signing/notarization and updater-archive creation. Verify the packaged app contains both helper files, validates their signing identity, and actually exercises the intended permission implementation. Keep Windows/Linux behavior target-appropriate. Cover the real installed upgrade and removal lifecycle, not just standalone fresh registration.

### 3. P1 — MySQL reports successful SQL work that is silently rolled back

Location: `src-tauri/src/data_browser.php:158–162`.

Each request owns a new PDO connection, but successful query execution does not reject an unfinished transaction. This reports success, including changed rows in a SELECT result, then silently rolls the changes back when the worker exits. A later Run cannot commit that earlier connection.

Reproduced through the production Rust/PHP bridge against isolated bundled MySQL:

```sql
START TRANSACTION;
UPDATE t SET value='should persist' WHERE id=1;
SELECT * FROM t;
```

The response says `Query completed` and shows `should persist`. A separate request reads the original value `initial`.

**Before RC 2:** match SQLite's explicit transaction-completion rule: require COMMIT/ROLLBACK in the same run, roll back unfinished work, and report that outcome accurately. Cover explicit transactions, autocommit changes, committed scripts, and errors after a commit. This is an existing MySQL gap, newly reproduced at this checkpoint.

### 4. P2 — MySQL's statement-limit error arrives after excess writes execute

Location: `src-tauri/src/data_browser.php:159–161`.

The code checks the limit while consuming result sets after sending the whole script to MySQL. It does not limit the statements submitted to the server.

Reproduction: run `INSERT INTO counter VALUES(1);` eleven times in one request. The worker reports `Run at most ten statements at a time`, but a later `SELECT COUNT(*) FROM counter` returns **11**. The guard therefore does not provide the execution limit described by its error. This is also an existing MySQL gap.

**Before RC 2:** validate an actual statement limit before execution using MySQL-aware parsing, or explicitly make this a result-set display limit with truthful reporting and safe draining. Do not reuse SQLite's splitter without accounting for MySQL syntax. Add a test that checks persisted data after exceeding the chosen limit, rather than checking only the error text.

### 5. P2 — One unreadable table/view blocks opening the whole database

Locations: `src/data/DataBrowser.tsx:95–107`, `src/data/DataBrowser.tsx:113–118`.

`load()` fetches the first table's rows and structure before publishing the database/catalog. If either fails, none of the catalog is committed. Initial source switching also waits for all of this before selecting the source. This affects broken views and can also affect oversized or expensive first tables.

Reproduced with a real SQLite fixture and the production frontend/native bridge:

```sql
CREATE VIEW a_broken AS SELECT * FROM missing_table;
CREATE TABLE healthy(id INTEGER PRIMARY KEY, value TEXT);
```

Open SQLite file reports the view error, leaves MySQL selected, displays no SQLite file card, and provides neither the healthy table nor the SQL tab. The user cannot repair the view or browse valid tables from this workspace.

**Before RC 2:** retain a usable catalog/source and access to SQL when an individual table cannot load, while ensuring old rows never appear under a new selection. Test initial open, source switching, table switching, broken views, and oversized row results.

### 6. P2 — Rust formatting currently fails a required CI gate

Location: `src-tauri/src/desktop.rs:531`.

`cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` exits 1 because the SQLite picker's `add_filter` call needs multiline formatting. Both normal verification and the Mac candidate workflow require this check.

**Before RC 2:** apply rustfmt and rerun the check. This is small housekeeping, but the current checkpoint cannot pass the configured gate as written.

## Original checkpoint verification

| Check | Result |
| --- | --- |
| Production TypeScript/Vite build | Passed |
| Python packaging/release/database tests | 53 passed, including current SQLite edits |
| Rust library tests with all features on this Mac | 64 passed |
| Clippy, all targets/all features, warnings denied | Passed |
| Chromium/WebKit at normal and minimum sizes | 270 passed; 26 skips are native-only/platform-conditioned tests |
| Production UI → native Rust/PHP/PTY bridge, Chromium | 6 passed |
| Native SQLite smoke | Passed: original files, backups, destructive guards, cancellation, no services |
| Native MySQL browser smoke | Passed: CRUD, exact values, conflicts, export/import, cancellation, backup/update preparation |
| Real service smoke | Passed: HTTP → PHP → MySQL, phpMyAdmin, backup restoration, PHP 8.0–8.5 CLI/web, settings/shutdown/cleanup |
| Git diff whitespace check | Passed |
| Workflow actionlint | Passed |
| npm advisory scan, including development dependencies | Passed: zero reported vulnerabilities |
| Six version locations | Agree on `0.1.0-rc.1`; RC 2 has not been staged |
| Rust formatting | Failed as described above |

The initial sandboxed Rust run could not bind loopback ports or inspect an owned process. Repeating its library tests with the necessary access passed all 64; those initial failures were environmental. The six native bridge browser tests were run separately, so the ordinary suite's native skips were not treated as coverage.

The targeted repro scripts and native test log are saved locally under ignored `artifacts/rc2-review/`. They use disposable data and do not modify the installed student profile. The SQLite worker-only repro bypasses Rust's protective-backup step to isolate its dependency guard; that guard is the same one used after the normal backup. A representative SQLite Structure screenshot was inspected; this does not constitute full human visual acceptance.

The owner explicitly approved sending locked npm dependency names and versions to npm's advisory service. The subsequent `npm audit --json` completed successfully on 2026-10-20 with zero reported vulnerabilities across all severities, including development dependencies. Its JSON result is saved locally in `artifacts/rc2-review/npm-audit.json`. This reflects the advisory service's coverage at scan time; this review did not perform a Rust/runtime advisory-database scan.

## Original acceptance and release gaps

1. **Fresh exact-artifact acceptance is required.** Existing RC 1 acceptance and standalone permission results do not cover this merged checkpoint plus local edits. Stage `0.1.0-rc.2` in all six version locations, commit the reviewed inputs, and build a fresh candidate after fixes.
2. **Mac permission/helper coverage must follow the packaged app.** Test fresh background approval, cancellation/retry, approved-helper reuse, rename/remove and last-site cleanup; HTTPS enable/disable/removal and password fallback; quit/reopen; RC 1 → RC 2 installation and updater replacement; RC 2 replacement with an approved helper; keep-data uninstall/reinstall. Include macOS 15, the supported minimum, rather than relying solely on results from the owner's newer system. Do not infer system-dialog focus from unit tests or notarization. Issue #11 is closed with a code-fix comment; that is not fresh device acceptance of the RC 2 artifact.
3. **Repeat Windows and Linux candidate CI and device acceptance.** This Mac review does not verify Windows ConPTY/UAC/WebView2 behavior, native Windows SQLite picker/path handling, Linux distro compatibility, or actual installers. The candidate workflows remain the place for those checks. The automated desktop installer baseline is historical alpha 3, so retain an explicit real RC 1 → RC 2 upgrade check with existing work.
4. **Exercise new data features on real projects.** In both desktop WebViews use SQLite WAL/locks/external writers, Unicode paths, backups and restoration, missing files, mixed values, composite/permuted keys, and schema changes. Verify MySQL and SQLite transaction semantics after the fixes. Extend regression coverage with the reproductions above; the green happy-path tests currently miss them.
5. **Update the release documents.** Unreleased notes omit the intended welcome/permission changes, and parts of `PROJECT_SPEC.md` and the Mac assessment still describe PR #8 as a separate/unmerged proposal. Bring them into line with the chosen RC 2 packaging and record exact tested artifacts/results without rewriting historical RC 1 evidence.
6. **Carry existing exceptions honestly.** Windows Authenticode, legacy PHP dependency provenance, and the Windows non-ASCII MySQL CLI identifier restriction were already accepted for the RC cycle. They are not newly discovered blockers. Before stable, document whether each remains an accepted stable limitation or must be resolved; never silently convert an exception into a verified claim.

## Suggested RC 2 → stable gate

After the fixes, freeze feature scope and obtain a green candidate plus exact-installer pre-publication acceptance. Publish the accepted bytes through the existing owner-authorized process, then complete real RC 1 → RC 2 in-app update/restart checks on **both** desktop platforms. RC 1's Windows update exception should not carry forward automatically.

Use RC 2 for at least one real classroom/work cycle across the supported systems. Record build identity, OS, reproduction steps and impact for each issue. Exercise several cold starts, normal PHP projects, both database engines, mail, terminal, custom sites/HTTPS, backups/restoration, and keep-data upgrade/removal. Track data loss, startup failures, failed updates and broken permission recovery as stop conditions. A lack of reports alone is insufficient if nobody exercised a platform or feature.

Resolve release-blocking defects and repeat the affected acceptance on any replacement candidate. A meaningful code/runtime change needs new artifacts and renewed testing; elapsed soak time does not validate different bytes. Once the exercised RC is quiet and both desktop update checks pass, build the stable version with the same feature scope, repeat its exact-artifact gates, and use the stable/preview feed rules already implemented. Stable requires its own embedded version and feed configuration; simply relabeling RC binaries is insufficient.
