# Release follow-up work

The owner accepted the Windows MySQL CLI non-ASCII identifier limitation on 2026-10-19. It is disclosed for this RC: on non-UTF-8 system locales, unsupported backup/export names fail explicitly and updates cannot proceed without a completed backup. Broader Unicode CLI backup support remains open; normal Unicode row/query handling is retained. See [database limitations](data-browser.md#windows-backupexport-identifiers).

## RC preparation policy — 2026-10-17

The release owner accepts the existing Windows PHP 8.0/8.1 dependency-provenance gap and unsigned Windows installer as disclosed limitations for this RC cycle. They do not block publication after the other acceptance gates pass. Runtime inputs stay as-is, the provenance checklist stays open, and Authenticode remains absent. Mac signing/notarization and cryptographic Mac/Windows updater signatures remain mandatory. On 2026-10-18 the owner locked RC feature scope after per-site PHP issue #5 closed as not planned; global PHP selection is retained. See [RC preparation](rc-preparation.md).

## Beta 3 publication

The release owner authorized publishing `v0.1.0-beta.3` on 2026-10-17 from successful candidate run [38989070274](https://github.com/eimg/fairway-xmp/actions/runs/38989070274), source commit `b860ddc824a1b966a5ffaee81c22b80d07bb6357`. The release tag identifies that tested application commit; later documentation commits update the release notes and download instructions. Publication reuses the exact candidate files without rebuilding.

The [publication workflow](https://github.com/eimg/fairway-xmp/actions/runs/38992340587) verified both updater signatures and uploaded the draft, then failed because draft lookup by tag returned 404. The publisher was corrected to find drafts through the release collection and publish by numeric release ID, with regression tests and exact asset-set validation. Recovery verified all 21 existing asset digests/sizes against the candidate, BUILD-INFO, and release checksums before publishing the same draft; no binary was rebuilt or replaced. The stable `beta-feed/latest.json` was published afterward and points to the versioned beta 3 assets.

Mac app and installer signing/notarization, signed Mac/Windows updater drills, native feature/service/backup tests, browser review, and five Linux distro checks passed. The first rebuild attempt failed after a successful Mac update when Apple's ticket-validation connection dropped; the fresh candidate passed all checks. Issue #3 is fixed by recovering verified Nginx/PHP-FPM masters on Mac after an unexpected exit. Linux updates remain manual; Windows Authenticode remains deferred.

Human acceptance fields remain unchanged: the user's successful subsequent-version Mac update/restart confirmation belongs to the earlier `123ccfe` candidate. Unperformed exact-candidate Windows and protected Mac installation checks are not marked complete by publication. The release notes and candidate manifest retain these limits and the existing legacy PHP/Windows path limitations.

## Beta 2 publication

The release owner authorized publishing `v0.1.0-beta.2` on 2026-10-16 from successful candidate run [38848919567](https://github.com/eimg/fairway-xmp/actions/runs/38848919567), source commit `764bccae0287af74cca0382359ab3de7853c5f48`. Publication reuses the tested binaries and source archives without rebuilding. The tag identifies that candidate commit; the subsequent publication commit only updates documentation and the publishing workflow.

All automated platform, native feature, service/backup, installed lifecycle, and Linux distro checks passed. Individual human acceptance checks have not been recorded and remain pending in the candidate evidence. Signing, manual updates, the Windows MySQL path restriction, and the legacy PHP provenance limitations remain disclosed in the beta 2 notes.

PR #1 was merged as `2114689`. The simpler Advanced settings dropdown, selected-runtime Linux recovery, Windows ZIP configuration, settings rollback, and regression tests are implemented. Repository Mac/Windows checks passed; native Windows/Linux validation exercised all six PHP versions, including Linux recovery and shutdown.

Beta 1 is published. The Windows PHP 8.0/8.1 provenance work below remains open and is disclosed with that release. It does not require changing the published beta 1 files.

## Legacy PHP scope for beta

Beta 3 adds Mac signing/notarization and signed desktop updates; see [signed beta releases](signed-beta-releases.md). Windows Authenticode remains deferred. PHP 8.0/8.1 remain legacy compatibility options, labeled in Advanced settings and CLI help; PHP 8.5 stays the default. No additional legacy runtime expansion or source-mapping research is scheduled in this pass. The provenance work below remains open and is disclosed in candidate metadata; labeling a runtime legacy does not verify its dependency sources.

The new [beta acceptance process](beta-acceptance.md) covers candidate identity, native lifecycle checks, restore verification, diagnostic reports, and final human review. A green browser run does not complete the remaining native checklist.

## Verify older Windows dependency sources

**Open follow-up; accepted as a disclosed limitation for this RC cycle.** Those official PHP ZIPs have no embedded SBOM identifying exact dependency build commits. `scripts/windows_legacy_sources.json` supplies 49 pinned source entries, and `runtime-sources.lock.json` records archive hashes. This verifies which source files are collected; it does not prove that their build recipes and patches match the distributed binaries.

Some mappings require particular attention:

| Binary dependency package | Recorded source tag |
| --- | --- |
| `libcurl-7.76.1-3-vs16-x64.zip` | `libcurl-7.76.1-2` |
| `libffi-3.3-4-vs16-x64.zip` | `libffi-3.3` |
| `libiconv-1.16-5-vs16-x64.zip` | `libiconv-1.16-1` |

Different build suffixes do not by themselves establish a source mismatch, but the mapping needs evidence. Passing runtime tests does not resolve that question.

- [ ] Match the dependency list against the notices in the exact checksum-pinned PHP 8.0/8.1 Windows ZIPs.
- [ ] Record upstream build evidence for the source revisions and package-specific recipes/patches, including the mappings above.
- [ ] Correct or extend the legacy source manifest and source lock where that evidence identifies missing inputs.
- [ ] Collect the corresponding-source archive and verify it includes those exact sources, recipes, patches, and notices.

For future provenance remediation, establish the exact mapping, choose a verifiable input, or defer those versions. The current RC cycle retains the existing inputs under the owner decision above. Keep this item open until evidence is recorded; version-matched tags alone are not completion.

## Finish the release owner's Mac visual test

- [ ] Confirm PHP selection is hidden in collapsed Advanced settings by default and uses one dropdown.
- [ ] Confirm Save changes the PHP card and selected web/CLI/Composer runtime, and the selection persists after Quit and reopen.
- [ ] Confirm the normal start/stop, close-window, menu-bar, and Quit experience remains straightforward.

The release owner tested the Mac beta 1 installer on 2026-10-15 and authorized publication. These three visual checks were not recorded individually. See [the PHP testing guide](testing-php-versions.md) for the commands.

## Publish the feature in a new release

- [x] Publish `v0.1.0-beta.1` from candidate run 38699450419, commit `f3cf84da6270038274590860db00c58eda2f1df6`. The installers include PHP version selection and report `0.1.0-beta.1`.
- [x] Upload those candidate files unchanged. `BUILD-INFO.json` records the commit, run, and hashes.
- [ ] Keep the Windows PHP 8.0/8.1 provenance item above open. Beta 1 discloses it and ships the matching Mac/Windows and Linux source archives beside the binaries.
- [x] Update README downloads and the Linux PHP-version instructions for beta 1.

See [Contributing](../CONTRIBUTING.md#publishing-a-release) for packaging and publication. Signing/notarization requirements for stable releases remain described there.
