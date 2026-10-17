# Preparing 0.1.0 RC

The next target is **0.1.0-rc.1**. Preparation is open; the version bump does not freeze the source or establish candidate acceptance.

## Scope and owner decisions

The release owner confirmed this scope on 2026-10-17:

- Keep the current PHP 8.0/8.1 Windows inputs and collected sources as-is. Their exact dependency build/patch provenance remains **unverified**, disclosed, and open for follow-up; it is not an RC blocker.
- Keep the Windows installer **without Authenticode signing**. Unknown-publisher/SmartScreen prompts remain possible. Cryptographic updater signatures remain mandatory.
- Keep Developer ID signing, notarization and stapling for the Mac app and installer, plus signed Mac/Windows updates and existing verification checks.
- Include [per-site PHP versions (#5)](https://github.com/eimg/fairway-xmp/issues/5) if an acceptable PR is reviewed, merged and validated before the candidate commit is frozen. If it misses the cut, record it as deferred and retain global PHP selection. Do not claim it is implemented in release notes before it lands.

The RC preparation branch includes Mailpit, the inbox, mail port migration, native Mail tray controls, and the grouped sidebar. The sidebar and release preparation still need to reach the release branch through normal review.

## Before cutting the candidate

- [ ] Finish review of the grouped sidebar and release preparation.
- [ ] Record per-site PHP as included/validated or deferred for this cut.
- [ ] Review `releases/0.1.0-rc.1-notes.md`, limitations and update instructions.
- [ ] Confirm all six version locations agree using `python3 scripts/release_candidate.py version`.
- [ ] Pass source/UI checks and resolve release-blocking regressions.
- [ ] Select the exact candidate commit and freeze features for it. Later code/runtime changes require a new candidate and new acceptance evidence.

## Build and accept

Use the existing **Release candidate** workflow from the chosen commit, or a `candidate-*` tag at that commit. Windows, Apple Silicon Mac, Linux, installer lifecycles, service/restore checks, signatures, sources and browser review must use the same commit. Building does not publish.

Download all installers, sources, UI reports, `candidate-manifest.json` and `acceptance-template.json` from that run. Verify the files with `scripts/release_candidate.py verify-files`. Complete the [native acceptance checklist](beta-acceptance.md) against those exact installers and record tester, OS, date and SHA256. Unperformed checks stay false.

Explicit RC checks:

- Beta 3 → RC upgrade preserves projects, database rows, sites/HTTPS trust, ports, PHP selection, backups and CLI integration. Automated historical alpha upgrade coverage does not replace this check.
- Old website port 8802 or another service on 11025: existing ports survive and new mail ports remain distinct.
- Mail-only Start/Stop, tray status/actions, PHP mail(), framework SMTP, inbox search/preview/delete, and attachment saving. Check stop/restart, Quit and reopening.
- Grouped navigation, terminal continuity and data-browser workspace continuity at normal/minimum native window sizes.
- Actual subsequent-version update and restart, including protected Mac installation prompts and cancellation. Ordinary local builds have no updater and cannot establish this acceptance.
- Per-site PHP selection, simultaneous runtimes, site terminals, recovery and cleanup if that feature lands.

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

Candidate freeze/build, device acceptance, optional per-site PHP inclusion, version tagging, publication and feed promotion remain pending. Accepted Windows limitations stay in candidate metadata and release notes; they are not marked verified or fixed.
