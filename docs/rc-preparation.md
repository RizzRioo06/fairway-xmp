# Preparing 0.1.0 RC

The next target is **0.1.0-rc.1**. Feature scope is locked by the release owner on **2026-10-18**. Only correctness/security fixes, tests and release preparation remain in scope. The exact candidate source commit and installer acceptance are recorded separately.

## Scope and owner decisions

The release owner confirmed this scope on 2026-10-17:

- Keep the current PHP 8.0/8.1 Windows inputs and collected sources as-is. Their exact dependency build/patch provenance remains **unverified**, disclosed, and open for follow-up; it is not an RC blocker.
- Keep the Windows installer **without Authenticode signing**. Unknown-publisher/SmartScreen prompts remain possible. Cryptographic updater signatures remain mandatory.
- Keep Developer ID signing, notarization and stapling for the Mac app and installer, plus signed Mac/Windows updates and existing verification checks.
- **Defer per-site PHP.** [Issue #5](https://github.com/eimg/fairway-xmp/issues/5) closed as not planned on 2026-10-18. Global PHP selection remains the behavior for all sites, terminals and CLI tools in this RC.

The RC preparation branch includes Mailpit, the inbox, mail port migration, native Mail tray controls, and the grouped sidebar. The sidebar and release preparation still need to reach the release branch through normal review.

The release owner additionally accepted a single global troubleshooting Logs page on 2026-10-18, with source/search filters and problems shown by default. See [the accepted log viewer design](log-viewer-consideration.md). The accepted implementation includes service shortcuts, exact-host request filtering with explicitly shared PHP, Unicode request display, and bounded reads/rendering. The final source review is complete; exact-candidate native device acceptance remains required before publication.

The full source review and hardening findings are recorded in [RC source review](rc-source-review.md).

## Before cutting the candidate

- [x] Finish source review of the grouped sidebar, global Logs page and release preparation; see the source-review findings.
- [x] Record per-site PHP as deferred; feature intake is closed.
- [x] Review `releases/0.1.0-rc.1-notes.md`, limitations and update instructions.
- [x] Confirm all six version locations agree using `python3 scripts/release_candidate.py version`.
- [x] Pass local source/UI checks and resolve the concrete regressions found; fresh candidate CI is required below.
- [x] Lock feature scope on 2026-10-18.
- [x] Select candidate source `d13a06b8f60688306d452b8a31c4ff649a4b26d4`, tag `candidate-0.1.0-rc.1-1`; [build run 39140420384](https://github.com/eimg/fairway-xmp/actions/runs/39140420384). Later code/runtime changes require a new candidate and new acceptance evidence.

## Build and accept

Use the existing **Release candidate** workflow from the chosen commit, or a `candidate-*` tag at that commit. Windows, Apple Silicon Mac, Linux, installer lifecycles, service/restore checks, signatures, sources and browser review must use the same commit. Building does not publish.

Download all installers, sources, UI reports, `candidate-manifest.json` and `acceptance-template.json` from that run. Verify the files with `scripts/release_candidate.py verify-files`. Complete the [native acceptance checklist](beta-acceptance.md) against those exact installers and record tester, OS, date and SHA256. Unperformed checks stay false.

Explicit RC checks:

- Beta 3 → RC upgrade preserves projects, database rows, sites/HTTPS trust, ports, PHP selection, backups and CLI integration. Automated historical alpha upgrade coverage does not replace this check.
- Old website port 8802 or another service on 11025: existing ports survive and new mail ports remain distinct.
- Mail-only Start/Stop, tray status/actions, PHP mail(), framework SMTP, inbox search/preview/delete, and attachment saving. Check stop/restart, Quit and reopening.
- Logs: real PHP errors/traces, failed and successful HTTP/HTTPS requests, source/search filters, pause/resume/copy, service failures, restart/replacement handling and responsive large-log reading.
- Grouped navigation, terminal continuity and data-browser workspace continuity at normal/minimum native window sizes.
- Actual subsequent-version update and restart, including protected Mac installation prompts and cancellation. Ordinary local builds have no updater and cannot establish this acceptance.
- Global PHP selection, site terminal paths, existing command-line integration and switching all six bundled versions remain covered; per-site PHP is excluded.

Run `python3 scripts/release_candidate.py check-acceptance --manifest /path/to/candidate-manifest.json --evidence /path/to/completed-acceptance.json` before publication.

## Publication and channels

**Publish tested release** retains the workflow file `publish-beta-release.yml`. Supply the successful candidate run ID, exact source commit, existing `v0.1.0-rc.1` tag at that commit, reviewed notes path and completed acceptance JSON path. RC/stable publication rejects missing or incomplete human evidence and mismatched installer hashes. The published files include `acceptance.json`.

Notes and completed evidence may be committed after the build in a documentation-only commit. Dispatch publication from a checkout containing them; the version tag must continue identifying the exact tested application commit. Reuse accepted bytes without rebuilding or moving published tags.

Feed promotion is an explicit input, **off by default**:

| Build | Embedded feed | Promotion |
| --- | --- | --- |
| Beta or RC | Existing `beta-feed/latest.json` | Preview feed only |
| Stable | `stable-feed/latest.json` | Stable feed and preview feed |

Promoting an RC offers it to installed beta users. Publish without promotion first when access should stay limited to manual downloads. Stable promotion lets beta/RC users move to stable; after installing stable, they use its stable-only endpoint. A newer preview for the next release is preserved when publishing an older stable patch. Feeds never move backwards.

RC releases are GitHub prereleases and never latest stable. Stable releases are ordinary releases marked latest. Moving from `0.1.0-rc.N` to `0.1.0` requires a new build and acceptance because the embedded version and endpoint change.

## Pending release work

Candidate build completion, exact-installer device acceptance, release version tagging, publication and feed promotion remain pending. The candidate source and build-only tag above are selected; the publication tag has not been created. Feature scope is locked and per-site PHP is deferred. Accepted Windows limitations stay in candidate metadata and release notes; they are not marked verified or fixed.
