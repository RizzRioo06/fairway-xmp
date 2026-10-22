# Release follow-up work

The owner accepted the Windows MySQL CLI non-ASCII identifier limitation on 2026-10-19. It is disclosed for this RC: on non-UTF-8 system locales, unsupported backup/export names fail explicitly and updates cannot proceed without a completed backup. Broader Unicode CLI backup support remains open; normal Unicode row/query handling is retained. See [database limitations](data-browser.md#windows-backupexport-identifiers).

## Current release: RC 3 — 2026-10-22

[RC 3](https://github.com/eimg/fairway-xmp/releases/tag/v0.1.0-rc.3) is published and offered through the preview update feed. All candidate/platform/signing/installer checks passed, and the owner confirmed pre-publication device acceptance. The current remaining release checks are actual in-app update/restart on macOS and Windows, including protected Mac authorization behavior. Their booleans remain false in the [RC 3 acceptance record](release-acceptance/0.1.0-rc.3.json). See the [release record](rc-preparation.md#rc-3-release--2026-10-22).

The older publication and prototype notes below are historical. They do not describe current release blockers; published files and their original acceptance records stay unchanged.

## RC 1 publication — 2026-10-19

The owner authorized merging PR #7 and publishing [v0.1.0-rc.1](https://github.com/eimg/fairway-xmp/releases/tag/v0.1.0-rc.1). PR #7 merged as `959a0d77bd0fc8d9b523502bbe727fa55ad6fff1`; [publication run 39179960227](https://github.com/eimg/fairway-xmp/actions/runs/39179960227) reused the exact accepted files from candidate run [39173368612](https://github.com/eimg/fairway-xmp/actions/runs/39173368612), tested source `24e5c053b30eb7cca280f6ad9f5de47d8e90b489`, without rebuilding. The version tag identifies that tested source, while subsequent documentation records publication.

All published asset digests/sizes, installer hashes, checksums, acceptance evidence and live preview metadata were verified. RC 1 remains a prerelease, and `beta-feed/latest.json` now offers it to enabled beta/RC desktop clients. [Pre-publication human acceptance](release-acceptance/0.1.0-rc.1.json) passes based on the owner's report and tested OS details. After publication the owner reported a successful beta 3 → RC 1 Mac in-app update/restart and accepted update coverage for both platforms, closing RC 1 sign-off with an explicit exception for the separate Windows post-publication device update check. That Windows test was not separately reported and its raw boolean remains false; standard full acceptance validation still requires it. This release-specific owner exception does not weaken future release gates. Regressions will be handled through GitHub issues. Published pre-publication evidence remains unchanged; the repository record includes the later result and owner decision. Existing Windows and legacy PHP limitations remain disclosed.

## RC preparation policy — 2026-10-17

On 2026-10-17 the release owner accepted the existing Windows PHP 8.0/8.1 dependency-provenance gap and unsigned Windows installer as disclosed limitations for this RC cycle. They do not block publication after the other acceptance gates pass. That PHP decision was superseded on 2026-10-21 by removing 8.0 and 8.1; see the closed provenance item below. Authenticode remains absent. Mac signing/notarization and cryptographic Mac/Windows updater signatures remain mandatory. On 2026-10-18 the owner locked RC feature scope after per-site PHP issue #5 closed as not planned; global PHP selection is retained. See [RC preparation](rc-preparation.md).

## Beta 3 publication

The release owner authorized publishing `v0.1.0-beta.3` on 2026-10-17 from successful candidate run [38989070274](https://github.com/eimg/fairway-xmp/actions/runs/38989070274), source commit `b860ddc824a1b966a5ffaee81c22b80d07bb6357`. The release tag identifies that tested application commit; later documentation commits update the release notes and download instructions. Publication reuses the exact candidate files without rebuilding.

The [publication workflow](https://github.com/eimg/fairway-xmp/actions/runs/38992340587) verified both updater signatures and uploaded the draft, then failed because draft lookup by tag returned 404. The publisher was corrected to find drafts through the release collection and publish by numeric release ID, with regression tests and exact asset-set validation. Recovery verified all 21 existing asset digests/sizes against the candidate, BUILD-INFO, and release checksums before publishing the same draft; no binary was rebuilt or replaced. The stable `beta-feed/latest.json` was published afterward and points to the versioned beta 3 assets.

Mac app and installer signing/notarization, signed Mac/Windows updater drills, native feature/service/backup tests, browser review, and five Linux distro checks passed. The first rebuild attempt failed after a successful Mac update when Apple's ticket-validation connection dropped; the fresh candidate passed all checks. Issue #3 is fixed by recovering verified Nginx/PHP-FPM masters on Mac after an unexpected exit. Linux updates remain manual; Windows Authenticode remains deferred.

Human acceptance fields remain unchanged: the user's successful subsequent-version Mac update/restart confirmation belongs to the earlier `123ccfe` candidate. Unperformed exact-candidate Windows and protected Mac installation checks are not marked complete by publication. The release notes and candidate manifest retain these limits and the existing legacy PHP/Windows path limitations.

## Beta 2 publication

The release owner authorized publishing `v0.1.0-beta.2` on 2026-10-16 from successful candidate run [38848919567](https://github.com/eimg/fairway-xmp/actions/runs/38848919567), source commit `764bccae0287af74cca0382359ab3de7853c5f48`. Publication reuses the tested binaries and source archives without rebuilding. The tag identifies that candidate commit; the subsequent publication commit only updates documentation and the publishing workflow.

All automated platform, native feature, service/backup, installed lifecycle, and Linux distro checks passed. Individual human acceptance checks have not been recorded and remain pending in the candidate evidence. Signing, manual updates, the Windows MySQL path restriction, and the legacy PHP provenance limitations remain disclosed in the beta 2 notes.

PR #1 was merged as `2114689`. The simpler Advanced settings dropdown, selected-runtime Linux recovery, Windows ZIP configuration, settings rollback, and regression tests are implemented. Repository Mac/Windows checks passed; native Windows/Linux validation exercised all six PHP versions, including Linux recovery and shutdown.

Beta 1 is published. The Windows PHP 8.0/8.1 provenance work was disclosed with that release. It does not require changing the published beta 1 files. Current builds close that follow-up by removal.

## Legacy PHP scope for beta

Beta 3 adds Mac signing/notarization and signed desktop updates; see [signed beta releases](signed-beta-releases.md). Windows Authenticode remains deferred. PHP 8.0/8.1 were legacy compatibility options in that beta, labeled in Advanced settings and CLI help; PHP 8.5 stayed the default. No additional legacy runtime expansion or source-mapping research was scheduled in that pass. The provenance work was disclosed in candidate metadata; labeling a runtime legacy did not verify its dependency sources.

The new [beta acceptance process](beta-acceptance.md) covers candidate identity, native lifecycle checks, restore verification, diagnostic reports, and final human review. A green browser run does not complete the remaining native checklist.

On 2026-10-21 PHP 8.0 and 8.1 were removed from current builds. The beta policy above still describes the published beta installers.

## Verify older Windows dependency sources

Closed on 2026-10-21 by removing PHP 8.0 and 8.1 from the Windows, macOS, and Linux bundles. Those versions are end of life and will not receive the critical PHP security updates. Their Windows ZIPs had no SBOM, so the dependency build/patch mapping could not be finished. `scripts/windows_legacy_sources.json` and the 8.0/8.1 lock entries are removed. PHP 8.2 through 8.5 remain, with PHP 8.5 as the default. A saved selection of 8.0 or 8.1 is left on disk and blocks startup until the user chooses PHP 8.2 or later. Already published installers are unchanged.

- [x] Remove PHP 8.0 and 8.1 instead of reconstructing unverifiable Windows dependency provenance.

## Historical beta 1 Mac visual record

- [ ] Confirm PHP selection is hidden in collapsed Advanced settings by default and uses one dropdown.
- [ ] Confirm Save changes the PHP card and selected web/CLI/Composer runtime, and the selection persists after Quit and reopen.
- [ ] Confirm the normal start/stop, close-window, menu-bar, and Quit experience remains straightforward.

The release owner tested the Mac beta 1 installer on 2026-10-15 and authorized publication. These three visual checks were not recorded individually. See [the PHP testing guide](testing-php-versions.md) for the commands.

## Publish the feature in a new release

- [x] Publish `v0.1.0-beta.1` from candidate run 38699450419, commit `f3cf84da6270038274590860db00c58eda2f1df6`. The installers include PHP version selection and report `0.1.0-beta.1`.
- [x] Upload those candidate files unchanged. `BUILD-INFO.json` records the commit, run, and hashes.
- [x] The Windows PHP 8.0/8.1 provenance item is closed by removal on 2026-10-21. Published beta 1 still discloses the limitation for those already-shipped files.
- [x] Update README downloads and the Linux PHP-version instructions for beta 1.

See [Contributing](../CONTRIBUTING.md#publishing-a-release) for packaging and publication. Signing/notarization requirements for stable releases remain described there.
