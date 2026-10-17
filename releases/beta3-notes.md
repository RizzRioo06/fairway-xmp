Fairway XMP beta 3 adds signed, notarized Mac distribution and enables in-app beta updates on macOS and Windows. It also includes the new Sites workflow with optional local `.test` domains and HTTPS, alongside the data browser and embedded terminal from beta 2.

- Mac: the app and `.pkg` installer are signed with Developer ID and notarized by Apple. The updater archive contains the same notarized app.
- Updates: signed, version-bound downloads are verified while services keep running. Before installation, Fairway backs up student databases, stops its services and terminal sessions, installs, and restarts. Failed downloads leave services running.
- Sites: save project web roots, choose a local domain, enable HTTPS when needed, and open the site, folder, or terminal from its actions menu. System approval is requested when local domain or certificate trust changes require it.
- Mac recovery: reopening Fairway after a crash or force quit recovers its own surviving Nginx and selected PHP-FPM services, so Stop and Start work normally. Executable and configuration checks keep unrelated processes untouched. Fixes #3.
- PHP, Composer, and `fairway` launchers refresh after an app update when command-line integration is enabled.

**Upgrading from beta 2 or earlier:** install beta 3 manually once. Those earlier apps do not contain the update feed and verification key. Future releases can be installed from beta 3's Updates page. Stop services and quit before the manual installation; projects, databases, settings, and saved sites are preserved.

Windows updater downloads are cryptographically signed, but the Windows executable and installer do not yet have Authenticode signatures. A fresh download can show a SmartScreen warning. Linux remains a manual installation.

Beta limitations remain: Windows MySQL needs ANSI-compatible paths or existing short-name aliases; PHP 8.0/8.1 are unsupported legacy compatibility options, and their exact Windows dependency build/patch provenance remains unverified. See the repository documentation for details.

Release validation includes native service and database checks, installer replacement/removal/reinstallation, signature rejection and native updater installation drills on both desktop platforms, and browser interaction/visual reports. Automated evidence and remaining human acceptance are recorded in the candidate manifest; a same-version updater drill does not certify a later-version restart or replace human acceptance.
