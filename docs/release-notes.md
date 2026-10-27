# Fairway XMP 0.1.0

Fairway XMP's first stable release brings the PHP development environment to Windows x64, Apple Silicon Macs and Linux x86_64. It carries forward RC 5's features and bundled runtime versions, with improved macOS background behavior.

Published from [candidate run 40445387643](https://github.com/eimg/fairway-xmp/actions/runs/40445387643), source `c6bf2d086890fd4dc50f877c6ab28abb87e419c5`, using the tested installers and updater payloads without rebuilding. Automated installer lifecycle checks passed on all three platforms, and desktop updater lifecycle checks passed on macOS and Windows.

- **PHP development tools:** switch between PHP 8.2–8.5, manage Nginx, MySQL and Mailpit, use Composer, and keep persistent PHP settings for each bundled version. Services run locally without Docker or a system PHP/MySQL installation.
- **Sites and local HTTPS:** register project folders with `.test` addresses, configure front-controller routing, and enable local HTTPS. All sites share the selected PHP version.
- **Desktop workspace:** manage MySQL and SQLite data, run SQL, create database backups, inspect logs, preview captured email, and work in two embedded terminal tabs. phpMyAdmin and MySQL command-line tools are included on every platform.
- **Optional Node.js tools:** fnm 1.39.0 is bundled on all platforms. macOS and Windows Settings can install, select and remove Node versions in a Fairway-owned store. Downloads are cancellable, and failed activation restores the previous selection. Existing fnm installations and unrecorded populated stores remain read-only. Node versions and their global npm packages live outside the app and survive Fairway updates.
- **macOS background behavior:** closing the window hides the Dock icon while services and the menu bar remain available. Choose Show Fairway XMP from the menu bar or launch the app again to restore the window and Dock icon. Minimizing keeps the Dock icon. Quit gracefully stops services and exits.
- **Linux terminal edition:** use the terminal dashboard or CLI for services, sites, backups, PHP selection and logs. Closing the dashboard leaves services running; explicit shutdown stops them. Linux includes native fnm and uses manual installer updates.

Bundled tools: PHP 8.2.35, 8.3.36, 8.4.27 and 8.5.12 (default), Composer 2.10.3, Nginx 1.30.6, MySQL 9.7.4 LTS, phpMyAdmin 5.2.3, Mailpit 1.32.0 and fnm 1.39.0. PHP includes MySQL, SQLite and PostgreSQL clients, sockets, SOAP and LDAP; macOS/Linux also include PCNTL and POSIX worker support.

## Upgrading

Existing macOS and Windows beta/RC clients can update to 0.1.0 through Settings. Stable installations check the stable feed for future releases. Linux users rerun the new installer. Projects, databases, backups, settings and installed Node versions are preserved by normal updates.

Node installation is optional. Choosing a Node version affects new terminals; existing terminals and development servers retain their selection until restarted. A populated fnm store from an older development build without a Fairway ownership record remains read-only; the stable release does not automatically adopt it. See [Node.js and fnm](https://github.com/eimg/fairway-xmp/blob/main/docs/node-tools.md) for storage and activation details.

## Requirements and limitations

- Windows 10 22H2 or Windows 11, x64; macOS 15 or later on Apple Silicon; Linux x86_64 with glibc 2.35 or newer. Intel Macs, ARM Linux and Alpine/musl are not supported.
- The release pipeline signs and notarizes the macOS app and installer. Windows installers do not have Authenticode signatures and may show an unknown-publisher warning; desktop updater payloads use separate cryptographic signatures.
- Windows MySQL requires ANSI-compatible filesystem paths or existing short-name aliases. Non-UTF-8 system code pages restrict identifiers in MySQL CLI backup/export; failed protective backups block the related update or destructive action.
- PostgreSQL support is through PHP client extensions connecting to a server you provide. Fairway does not manage a PostgreSQL server, and the desktop Data browser supports MySQL and SQLite.

See [current limitations](https://github.com/eimg/fairway-xmp/blob/main/docs/maintenance.md#current-limitations) and the [README](https://github.com/eimg/fairway-xmp#readme) for setup and platform guidance.
