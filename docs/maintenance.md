# Maintenance

Work from current behavior in [PROJECT_SPEC.md](../PROJECT_SPEC.md), reproduce a problem, then update the relevant implementation, tests and guide. There is no inherited release freeze, audit-pass sequence or blanket deferral of new work. Git history and published releases retain prior decisions and evidence.

## Runtime updates

1. Review upstream versions, checksums and security fixes. Update `runtime-manifest.json`, including the default PHP version and every affected `phpVersions` entry.
2. Update the relevant desktop/Linux download lock entries and dependency closure. Preserve unrelated pins and both platforms' entries in the shared desktop lock.
3. Restage runtimes. `--refresh-lock` can refresh unrelated components; it is not a PHP-only updater. `--resume-lock` retains existing bottle pins and does not automatically replace an outdated PHP entry.
4. Regenerate `runtime-sources.lock.json` with `scripts/package_sources.py --refresh-lock`, using the exact bottles/Windows SBOMs. Review sources, recipes, checksums and notices. Final packaging uses `--locked`.
5. Run runtime verification, focused regressions, CLI/web checks for every bundled PHP branch, and the affected platform builds. Verify Composer, required MySQL/SQLite/PostgreSQL/socket/SOAP/LDAP extensions and Unix worker support, mail, sessions and uploads as relevant. Both common and platform-specific extension requirements belong in the runtime manifest.
6. Publish a new version through the [build and release](build-and-release.md) process when authorized. Preserve published artifacts and user data.

Linux uses Ubuntu 22.04/glibc 2.35 as its build baseline and bundles component-specific library dependencies. Do not mix PHP/Nginx libraries with MySQL's private OpenSSL/Kerberos copies. Linux PHP reuse verifies both version and required extensions. Check all source packages remain available for the copied library versions.

Removing a PHP branch requires handling saved selections explicitly: preserve the user's choice, explain the unavailable version, and allow selecting a supported one without losing settings. Current builds support PHP 8.2–8.5; version numbers and extension requirements belong in the manifest rather than duplicated test instructions.

## Current limitations

- Windows installers have no Authenticode signature. Desktop updater signatures remain enabled independently.
- Windows MySQL requires ANSI-compatible filesystem paths or existing short-name aliases. Non-UTF-8 code pages restrict MySQL CLI backup/export identifiers; Unicode row/query handling is supported. Failed protective backups prevent the related destructive action/update. Details are in [Data browser](data-browser.md#windows-backupexport-identifiers).
- Linux updates are manual. The desktop Data browser and embedded terminal are Windows/macOS features.
- PHP selection is global to the app instance. PostgreSQL support is currently PHP client connectivity to a server supplied by the user, with no Data browser adapter or managed server.
- Trust-anchor constraint enforcement differs between clients; encoded root constraints alone do not establish every browser's enforcement. [Security boundaries](security-boundaries.md) documents the verifier behavior and test controls.

These describe the current implementation and can be improved. They are not permanent scope restrictions or release-stage exceptions.

## Further improvements

The following are useful directions when a concrete project need justifies them, rather than prerequisites for ordinary work:

- Simplify engine adapters/shared UI capabilities as PostgreSQL Data browser support is added.
- Improve Windows CLI backup/export handling for Unicode identifiers and filesystem paths.
- Extend native permission, protected-update, WebView and clipboard coverage where automation or reproduction is practical.
- Consider shared-machine isolation: teaching-account privileges, Unix FastCGI sockets, and hosts/CA/helper ownership. Preserve existing credentials and coursework when changing these boundaries.
- Split large modules, deduplicate PHP worker logic, introduce a service enum or typed errors when doing so clarifies an actual change.
- Review Action revision pinning and toolchain/dependency updates as part of routine maintenance.

## Diagnosing native permissions

macOS helper approval, hosts writes, user-keychain trust and updater replacement use different authorization paths. Record the app's actual signature/notarization status, helper registration/approval and launchd availability before attributing a repeated approval prompt to one cause. A signed but unnotarized development copy can have different Gatekeeper behavior from a distributed app. Respect pending approval and explicit retry rather than repeatedly registering copies or automatically repairing unrelated hosts differences.

Do not broaden user-keychain trust to system-wide trust merely to change a prompt. Keep privileged helper requests bounded to Fairway's managed hosts block and certificate profile. Use [optional platform checks](testing.md#optional-platform-checks) for real prompt/focus behavior; unit tests cannot establish which authentication dialog macOS will offer.
