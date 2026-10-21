# 0.1.0 RC release preparation

**0.1.0-rc.1 was published on 2026-10-19**. Feature scope is locked by the release owner on **2026-10-18**. Only correctness/security fixes, tests and release preparation remain in scope. The exact candidate source commit and installer acceptance are recorded separately.

## RC 3 source preparation — 2026-10-20

The owner authorized the terminal/workspace/log/data-browser changes and resolution of the [RC 3 review findings](rc3-readiness-review.md). Source versions are staged as **0.1.0-rc.3**. [RC 3 notes](../releases/0.1.0-rc.3-notes.md) describe the changes; no candidate, publication or human acceptance is claimed yet. Runtime versions and dependency pins are unchanged.

Before publication, build from the final committed inputs using the cross-platform candidate workflow, require all automated gates, and obtain new exact-installer acceptance. Verify RC 2 → RC 3 upgrade preservation and the signed Mac app's SQLite protected-folder prompts. The local unsigned app cannot settle the reported duplicate-prompt behavior. Complete actual in-app update/restart checks after feed promotion. Do not reuse RC 2 acceptance for new binaries or alter the published RC 2 assets.

## RC 2 build preparation — 2026-10-20

The owner authorized fixing the [readiness findings](rc2-readiness-review.md), including the current merged changes, and staging **0.1.0-rc.2**. RC 2 is published from [candidate run 36](https://github.com/eimg/fairway-xmp/actions/runs/39366414344) at `3ace851c69096a709549ef540955db4b630358dc` (tag `candidate-0.1.0-rc.2-7`). The version tag is `v0.1.0-rc.2` on that tested commit. The owner turned the preview update feed on for this publication. All six embedded version locations agree; runtime versions and source/download pins remain unchanged.

The source fixes cover SQLite foreign-key index protection, MySQL unfinished transactions and truthful result limits, table-load recovery, and required formatting. Normal Mac candidate packaging explicitly enables both native permission features, bundles the helper before final signing, checks embedded features/payload/Developer ID identity, and checks fresh signed-app registration/cleanup without approval or hosts writes. Approved-helper upgrade/replacement remains a device acceptance requirement.

On 2026-10-20 the owner instructed publication of that green candidate and promotion of the preview update feed. Installers are the files from run 36. The human in-app update/restart check is recorded after installed clients can receive the update.

The sections below retain the historical RC 1 preparation and publication record. For RC 2 publication, use `v0.1.0-rc.2` at the exact tested commit, the new notes and new evidence; do not move RC 1 tags or replace its assets.

## Scope and owner decisions

The release owner confirmed this scope on 2026-10-17:

- On 2026-10-17, keep the then-current PHP 8.0/8.1 Windows inputs and collected sources as-is. That decision was superseded on 2026-10-21: PHP 8.0 and 8.1 are removed from Windows, macOS, and Linux. A saved selection of either version is left in place and blocks startup until the user chooses PHP 8.2 or later. Already published installers are unchanged.
- Keep the Windows installer **without Authenticode signing**. Unknown-publisher/SmartScreen prompts remain possible. Cryptographic updater signatures remain mandatory.
- Keep Developer ID signing, notarization and stapling for the Mac app and installer, plus signed Mac/Windows updates and existing verification checks.
- **Defer per-site PHP.** [Issue #5](https://github.com/eimg/fairway-xmp/issues/5) closed as not planned on 2026-10-18. Global PHP selection remains the behavior for all sites, terminals and CLI tools in this RC.
- On 2026-10-19, accept the documented Windows MySQL non-ASCII backup/export identifier limitation on non-UTF-8 system code pages. Errors must be explicit, preserve data, and block updates whose backup cannot complete. See [database limitations](data-browser.md#windows-backupexport-identifiers).
- On 2026-10-19, move only the human in-app update/restart check after publication and feed promotion. Other exact-installer human checks and signed-updater CI remain prerequisites; unperformed update checks stay false.

The RC preparation branch includes Mailpit, the inbox, mail port migration, native Mail tray controls, and the grouped sidebar. The sidebar and release preparation reached main through merged PR #7.

The release owner additionally accepted a single global troubleshooting Logs page on 2026-10-18, with source/search filters and problems shown by default. See [the accepted log viewer design](log-viewer-consideration.md). The accepted implementation includes service shortcuts, exact-host request filtering with explicitly shared PHP, Unicode request display, and bounded reads/rendering. The final source review is complete. On 2026-10-19 the owner reported the human device tests done and passed; see the [candidate acceptance record](release-acceptance/0.1.0-rc.1.json). The record includes the owner's tested OS versions and passes pre-publication acceptance validation.

The full source review and hardening findings are recorded in [RC source review](rc-source-review.md).

## Before cutting the candidate

- [x] Finish source review of the grouped sidebar, global Logs page and release preparation; see the source-review findings.
- [x] Record per-site PHP as deferred; feature intake is closed.
- [x] Review `releases/0.1.0-rc.1-notes.md`, limitations and update instructions.
- [x] Confirm all six version locations agree using `python3 scripts/release_candidate.py version`.
- [x] Pass local source/UI checks and resolve the concrete regressions found; fresh candidate CI is required below.
- [x] Lock feature scope on 2026-10-18.
- [x] Select source `24e5c053b30eb7cca280f6ad9f5de47d8e90b489`, tag `candidate-0.1.0-rc.1-5`; [candidate validation run 39173368612](https://github.com/eimg/fairway-xmp/actions/runs/39173368612). See that run for platform results. Candidates 1–4 are superseded. Later code/runtime changes require a new candidate and new acceptance evidence.

## Build and accept

Follow the approved [acceptance phases](beta-acceptance.md#acceptance-before-and-after-publication). Only the human update into the published candidate is deferred. Other exact-installer device checks and signed-updater CI remain required before publication; no deferred check is marked passed.

Use the existing **Release candidate** workflow from the chosen commit, or a `candidate-*` tag at that commit. Windows, Apple Silicon Mac, Linux, installer lifecycles, service/restore checks, signatures, sources and browser review must use the same commit. Building does not publish.

Download all installers, sources, UI reports, `candidate-manifest.json` and `acceptance-template.json` from that run. Verify the files with `scripts/release_candidate.py verify-files`. Complete the [native acceptance checklist](beta-acceptance.md) against those exact installers and record tester, OS, date and SHA256. Unperformed checks stay false.

Explicit RC checks:

- Beta 3 → RC upgrade preserves projects, database rows, sites/HTTPS trust, ports, PHP selection, backups and CLI integration. Automated historical alpha upgrade coverage does not replace this check.
- Old website port 8802 or another service on 11025: existing ports survive and new mail ports remain distinct.
- Mail-only Start/Stop, tray status/actions, PHP mail(), framework SMTP, inbox search/preview/delete, and attachment saving. Check stop/restart, Quit and reopening.
- Logs: real PHP errors/traces, failed and successful HTTP/HTTPS requests, source/search filters, pause/resume/copy, service failures, restart/replacement handling and responsive large-log reading.
- Grouped navigation, terminal continuity and data-browser workspace continuity at normal/minimum native window sizes.
- After publication and feed promotion: actual subsequent-version update and restart into this candidate, including protected Mac installation prompts and cancellation. Ordinary local builds have no updater and cannot establish this acceptance.
- Global PHP selection, site terminal paths, existing command-line integration and switching every bundled version (8.2–8.5) remain covered; per-site PHP is excluded. A saved PHP 8.0 or 8.1 selection must stay on disk and block startup until the user chooses 8.2 or later.

Run `python3 scripts/release_candidate.py check-acceptance --phase pre-publication --manifest /path/to/candidate-manifest.json --evidence /path/to/pre-publication-acceptance.json` before publication. After the human update tests, validate the completed evidence with `--phase post-publication`; omitting the phase also requires all checks. Record the dated results without replacing the published pre-publication evidence. Any failed update stops further rollout.

## Publication and channels

**Publish tested release** retains the workflow file `publish-beta-release.yml`. Supply the successful candidate run ID, exact source commit, existing `v0.1.0-rc.1` tag at that commit, reviewed notes path and pre-publication acceptance JSON path. RC/stable publication rejects missing evidence, unfinished pre-publication checks and mismatched installer hashes. The published files include `acceptance.json`; `BUILD-INFO.json` distinguishes pre-publication approval from full human acceptance and lists pending update checks.

Notes, evidence and reviewed publication-policy tooling may be committed after the build without changing application/runtime inputs. Dispatch publication from a checkout containing them and the approved phase checker; the version tag must continue identifying the exact tested application commit. This policy follow-up leaves candidate 5's application/runtime unchanged. Reuse accepted bytes without rebuilding or moving published tags.

Feed promotion is an explicit input, **off by default**:

| Build | Embedded feed | Promotion |
| --- | --- | --- |
| Beta or RC | Existing `beta-feed/latest.json` | Preview feed only |
| Stable | `stable-feed/latest.json` | Stable feed and preview feed |

Promoting an RC offers it to installed beta users. Publish without promotion first when access should stay limited to manual downloads. Stable promotion lets beta/RC users move to stable; after installing stable, they use its stable-only endpoint. A newer preview for the next release is preserved when publishing an older stable patch. Feeds never move backwards.

RC releases are GitHub prereleases and never latest stable. Stable releases are ordinary releases marked latest. Moving from `0.1.0-rc.N` to `0.1.0` requires a new build and acceptance because the embedded version and endpoint change.

## Publication and release sign-off

Candidate 5 passed all automated gates, and the owner reported the pre-publication human device checks passed on 2026-10-19. The acceptance record retains exact installer hashes and the owner's reported macOS 27 Beta 4, Windows 11 (August 11, 2026 update, KB5121003), and Ubuntu 26.04. Pre-publication acceptance validation passes.

The owner authorized merge, publication and release on 2026-10-19. [PR #7](https://github.com/eimg/fairway-xmp/pull/7) merged as `959a0d77bd0fc8d9b523502bbe727fa55ad6fff1`. [Publication run 39179960227](https://github.com/eimg/fairway-xmp/actions/runs/39179960227) published [v0.1.0-rc.1](https://github.com/eimg/fairway-xmp/releases/tag/v0.1.0-rc.1) and promoted the preview feed. The version tag identifies tested application commit `24e5c053b30eb7cca280f6ad9f5de47d8e90b489`; no application/runtime bytes were rebuilt. Published asset digests/sizes, installer hashes, acceptance evidence, signatures and the live preview feed were checked against the exact candidate.

The owner reported a successful beta 3 → RC 1 Mac in-app update/restart on 2026-10-19 and accepted update coverage for both platforms, with regressions to arrive through GitHub issues. RC 1 release sign-off is closed with an explicit owner exception for the separate Windows post-publication device update test; no Windows test execution is claimed. The raw Windows check remains false, so the standard full checker still rejects that unperformed check. This release-specific owner decision does not change the checker or future release gates. Individual protected Mac prompt/cancellation cases were not separately described. The repository acceptance record contains the later results and sign-off; published pre-publication assets remain unchanged. Feature scope is locked and per-site PHP is deferred. Accepted Windows limitations stay disclosed; they are not marked verified or fixed.
