# RC 3 PHP security update — 2026-10-21

RC 3 remains unreleased. This checklist prepares the runtime update for the reported CVE-2026-109618 and CVE-2026-109619 fixes; it does not claim that the currently pinned packages contain them.

The [PHP security announcement](https://www.php.net/archive/2026.php#2026-10-20-1) schedules the release for October 21 at 14:00 UTC (20:30 Yangon). Affected and fixed versions will be published with the release. Record the final advisories and exact versions before changing pins.

## Inputs to confirm after publication

| Branch | Current pre-update pin | Official fixed version | macOS bottle | Windows ZIP | Linux source |
| --- | --- | --- | --- | --- | --- |
| 8.2 | 8.2.34 | Pending | Pending | Pending | Pending |
| 8.3 | 8.3.35 | Pending | Pending | Pending | Pending |
| 8.4 | 8.4.26 | Pending | Pending | Pending | Pending |
| 8.5 (default) | 8.5.11 | Pending | Pending | Pending | Pending |

macOS uses Homebrew ARM macOS 15 (`arm64_sequoia`) bottles; availability may lag the upstream announcement. Windows uses official x64 NTS ZIPs (VS16 for 8.2/8.3 and VS17 for 8.4/8.5). Linux builds the pinned PHP source archives on Ubuntu 22.04 for the glibc 2.35 baseline. Confirm every affected bundled branch on every platform. Keep 8.0/8.1 excluded and 8.5 as the default.

## Update and verify the locked inputs

1. Update `runtime-manifest.json`, including both the default `php` field and `phpVersions`.
2. Update the PHP entries and required dependency closure in `runtime-downloads.lock.json` and `runtime-linux-downloads.lock.json`. Verify published checksums/signatures where available. Review the diff to retain unrelated pins.
3. Restage desktop and Linux runtimes using the new inputs. `prepare_runtime.py --refresh-lock` can refresh unrelated inputs; it is not a PHP-only updater. `--resume-lock` retains existing bottle entries, so it does not replace an old PHP pin automatically. Keep the two desktop platforms' entries when updating their shared lock.
4. Regenerate `runtime-sources.lock.json` against the new binary metadata with `scripts/package_sources.py --refresh-lock`; review source versions, receipts/SBOM-derived dependencies and checksums. Update `THIRD_PARTY_NOTICES.md` and any version-specific documentation/assertions that change. Final source packaging must pass `--locked`.
5. Prepare again with ordinary locked commands. Run `python3 scripts/prepare_runtime.py --verify-only` on each desktop platform. Runtime verification must reject an old staged manifest or extra PHP directories. Verify the final installer/app resources too, rather than relying only on the workspace manifest.

## Candidate checks

- Run production frontend build, TypeScript unit tests, Python tests, Rust library/terminal tests, Clippy, formatting and workflow validation. Keep the existing candidate browser/native gates.
- For every bundled PHP branch, verify CLI and the actual served PHP worker, required extensions, Composer, MySQL/PDO, SQLite, mail, sessions and uploads. Run any applicable upstream regression check published with the advisories.
- Verify recovery from saved 8.0 and 8.1 selections: no automatic version change; desktop Settings and Linux `fairway php-version` can select a supported branch; no services start before correction; failed saves preserve the old selection/configuration; live old service PID markers prevent switching.
- Preserve RC 2 projects, rows, sites, ports, supported PHP selection, backups and CLI integration through upgrade. User visual testing and signed Mac SQLite protected-folder checks remain exact-installer acceptance tasks.
- Replace the pending versions in this checklist and the RC 3 notes once verified. Commit the final application/runtime inputs before building. Warm the existing Windows/macOS/Linux build-input caches on that commit, then run the cross-platform Release candidate workflow.
- Require signed/notarized Mac artifacts, signed desktop updater payloads, Linux installer checks, corresponding sources and exact artifact hashes. Record new acceptance evidence. Publication and feed promotion remain separate from preparation/building; keep previous release assets and evidence unchanged.

## Preparation evidence

Local preparation checks completed on macOS arm64, 2026-10-21:

- Production TypeScript/Vite build; 14 TypeScript unit tests; 79 Python tests.
- 113 Rust library tests with all features; 91 core/terminal tests with desktop features disabled; Clippy for all targets/features with warnings denied; rustfmt.
- 40 focused Chromium/WebKit checks at normal and minimum sizes: PHP recovery, startup repair, mail previews/CSP/external-link routing, workspace/nested-modal focus, Table SQL/foreign-key dialogs and SQL-import/footer behavior. A StrictMode effect replay losing the original modal trigger was fixed; keyboard activation/return is verified on both engines.
- Workflow actionlint, diff whitespace, all six RC 3 version locations, and current staged macOS runtime verification.

Socket/process-dependent Rust checks passed with the required local access after sandbox restrictions blocked their initial run. Browser checks use the test backend and do not claim native visual acceptance or actual external browser launches.

The local app is for owner testing with the current pre-update runtime pins. Patched-input validation, final committed-input cross-platform candidate results, signed distribution artifacts and device acceptance remain pending. No publication or feed promotion is recorded by this preparation.
