# RC 3 PHP security update — 2026-10-22

RC 3 remains unreleased. Runtime inputs are now pinned to PHP 8.2.35, 8.3.36, 8.4.27 and 8.5.12, addressing CVE-2026-109618 and CVE-2026-109619. Candidate compilation, platform checks and device acceptance remain pending.

The [October 21 PHP security advisory](https://www.php.net/archive/2026.php) confirms these fixed versions. The advisories are [CVE-2026-109618](https://github.com/php/php-src/security/advisories/GHSA-w8hw-w9fx-j79w) and [CVE-2026-109619](https://github.com/php/php-src/security/advisories/GHSA-6wmf-4rv5-p3rp). The owner authorized the input update and GitHub candidate build on October 22 and will report workflow progress; no automatic monitoring is scheduled.

## Verified upstream inputs — October 22

| Branch | Previous pin | Official fixed version | macOS bottle | Windows ZIP | Linux source |
| --- | --- | --- | --- | --- | --- |
| 8.2 | 8.2.34 | 8.2.35 | SHA256 verified | SHA256 verified | SHA256 verified |
| 8.3 | 8.3.35 | 8.3.36 | SHA256 verified | SHA256 verified | SHA256 verified |
| 8.4 | 8.4.26 | 8.4.27 | SHA256 verified | SHA256 verified | SHA256 verified |
| 8.5 (default) | 8.5.11 | 8.5.12 | SHA256 verified | SHA256 verified | SHA256 verified |

macOS uses verified Homebrew ARM macOS 15 (`arm64_sequoia`) bottles for all four patched branches. Windows uses official x64 NTS ZIPs (VS16 for 8.2/8.3 and VS17 for 8.4/8.5). Linux builds the pinned PHP source archives on Ubuntu 22.04 for the glibc 2.35 baseline. Confirm every affected bundled branch on every platform. Keep 8.0/8.1 excluded and 8.5 as the default.

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
- Replace the pending versions in this checklist and the RC 3 notes once verified. Commit the final application/runtime inputs before building. Run the cross-platform Release candidate workflow from that commit. Its platform jobs stage and validate the new inputs and can save caches themselves; avoid a separate overlapping warm-build run when the candidate already prepares the same inputs.
- Require signed/notarized Mac artifacts, signed desktop updater payloads, Linux installer checks, corresponding sources and exact artifact hashes. Record new acceptance evidence. Publication and feed promotion remain separate from preparation/building; keep previous release assets and evidence unchanged.

## Preparation evidence

Local preparation checks completed on macOS arm64, 2026-10-21:

- Production TypeScript/Vite build; 14 TypeScript unit tests; 79 Python tests.
- 113 Rust library tests with all features; 91 core/terminal tests with desktop features disabled; Clippy for all targets/features with warnings denied; rustfmt.
- 40 focused Chromium/WebKit checks at normal and minimum sizes: PHP recovery, startup repair, mail previews/CSP/external-link routing, workspace/nested-modal focus, Table SQL/foreign-key dialogs and SQL-import/footer behavior. A StrictMode effect replay losing the original modal trigger was fixed; keyboard activation/return is verified on both engines.
- Workflow actionlint, diff whitespace, all six RC 3 version locations, and current staged macOS runtime verification.
- The October 21 locked corresponding-source plan validated all 209 inputs. Removing PHP 8.0/8.1 had also removed two source pins still required by the remaining Windows PHP SBOMs (c-client and libbzip2). Their exact previously reviewed source revisions/checksums were restored with the current SBOM metadata; no binary version or unrelated source pin changed. That historical plan is superseded by the October 22 plan below.

Socket/process-dependent Rust checks passed with the required local access after sandbox restrictions blocked their initial run. Browser checks use the test backend and do not claim native visual acceptance or actual external browser launches.

The October 21 local app still contains the previous runtime pins. The patched app/installer build is delegated to GitHub; no new local native build is claimed. Final cross-platform candidate results, signed distribution artifacts and device acceptance remain pending. No publication or feed promotion is recorded by this preparation.

## Patched-input preparation — October 22

- Downloaded and verified all twelve new PHP archives against official PHP/Homebrew SHA256 metadata: four Windows x64 NTS ZIPs, four macOS ARM bottles and four source tarballs. Windows SBOMs and macOS source recipes were read from those exact verified archives.
- Updated only PHP entries in the desktop/Linux download locks. The default stays 8.5 and 8.0/8.1 remain excluded. A static check of the shipped Mac CLI/CGI/FPM binaries and extension libraries found all 28 unique non-system library references covered by 20 existing pinned dependency bottles; no unrelated Mac dependency pin changed.
- The new corresponding-source plan validates 212 inputs. Four PHP source archives and 16 new/revised Windows dependency-source archives were checksum-verified; unchanged source checksums were retained. The selected Windows SBOMs determine the required dependency revisions, rather than historical PHP-version labels.
- Local Python packaging/release/SQLite checks passed (79 tests), along with actionlint, version consistency and diff whitespace. The local SQLite checks exercised the previous staged runtime; the GitHub platform jobs must stage and test the patched runtimes.
- The GitHub candidate must complete the existing runtime, native browser, service/restore, installer, signing/notarization, updater-signature and exact-artifact checks. The owner will provide run status. Publication and feed promotion are separate actions.
