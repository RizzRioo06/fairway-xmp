# Fairway XMP development environment

## Product

Display name: **Fairway XMP** on Windows and macOS. The green application mark is shared by the app, Windows installer, and uninstaller. The macOS bundle is `/Applications/Fairway XMP.app`; installer hooks and removal target only that bundle.

A classroom PHP development environment with one consistent desktop control panel.
Supported release targets: Windows 10 22H2 / Windows 11 x64, and macOS 15+ Apple Silicon.
No Docker, WSL, Linux packages, Intel Mac packages, Node/npm bundle, or Laravel-specific project management.

## Application and runtimes

Tauri 2, Rust service manager, React/TypeScript UI. Exact versions and required PHP extensions are recorded in `runtime-manifest.json`. Download inputs are pinned by SHA256 in `runtime-downloads.lock.json` once prepared and reviewed. Runtime preparation never installs Homebrew or system services.

The alpha 2 runtime uses PHP 8.5.11 CLI/FastCGI, stable Nginx 1.30.5, MySQL 9.7.2 LTS, phpMyAdmin 5.2.3, Composer 2.10.3, CA trust roots, native runtime libraries and upstream notices. Mac Nginx is compiled from its pinned upstream source using pinned PCRE2/OpenSSL bottles and the macOS SDK's zlib. No system installation or Homebrew service is required. Windows inputs are official x64 ZIP distributions with checked SHA256 pins.

## Files

- Document root: the current user's lowercase `fairway/www` folder under their home directory.
- Application data: `dev.fairway.environment` under the platform's local user data directory.
- Application data contains settings, generated service configs, logs, MySQL data, phpMyAdmin's writable overlay, PHP sessions, temporary files, Composer home, backups, and CLI launchers.
- Runtime files remain inside the installed application resources and are not mutated at startup.
- Existing coursework is never overwritten except the initial welcome page, created only if `index.php` is absent.

## Services

Default loopback ports: website 8800, phpMyAdmin 8801, MySQL 13306, internal PHP FastCGI 19000.
Nginx owns two listeners and serves phpMyAdmin; phpMyAdmin is not an independent server process.

Start all preflights all ports before launching. Ports are validated, distinct, unprivileged, and saved only while services are stopped. Conflicts suggest an available alternative and show the occupying application where available. No silent port reassignment.

The application tracks processes it starts. Nginx startup starts PHP; stopping PHP first stops Nginx. Starting/stopping MySQL is independent. Restarting PHP restores a previously running web server. A partial Start all failure rolls back newly started services while retaining previously running ones. MySQL is shut down through authenticated mysqladmin and is never forcibly killed. A surviving MySQL instance can be recovered only when its PID and exact runtime/config command identity match this installation.

Minimizing leaves services running. Closing the window stops all; if graceful shutdown fails, the window stays open with the reason. Services do not start at login or automatically at app launch.

Status distinguishes an owned live process from a ready endpoint. Website/phpMyAdmin listeners are checked against a Fairway XMP health endpoint. Unexpected process exits surface a log hint.

## Database

Fresh initialization creates a random private root password and a local teaching account `fairway` / `fairway`, with a `fairway` database. The teaching account may create/manage student databases locally. phpMyAdmin uses cookie authentication and disallows root login.

Student connection host is explicitly `127.0.0.1` with the saved database port. Port changes update phpMyAdmin and copyable connection settings; project files are never silently edited.

Logical backups include student databases, routines, events, and triggers and exclude MySQL system schemas. Failed exports are not presented as completed backups. Existing database data and credentials are never silently reset on an error.

Before the first supported MySQL 8.4 → 9.7 startup, preserve the stopped physical database, credentials, settings and version markers under the user's backups folder. Record the completed backup before starting MySQL; failed startup retries never replace it. Record the new database family after authenticated provisioning succeeds. Reject unknown transitions and attempted downgrades. Fresh initialization records its engine version separately from account provisioning so interrupted setup can retry. Test migration against isolated alpha 1 data before release; Mac verification has passed; Windows runs the same isolated migration check in the installer workflow.

## CLI integration

Persistent user-level integration provides PHP and Composer launchers. Composer explicitly runs under bundled PHP and uses an app-owned Composer home.

Windows: add/remove only the app's exact entry in HKCU Environment Path and notify the shell. macOS: managed blocks in zsh/bash profiles with recorded locations, removed precisely while preserving unrelated edits. The UI displays inherited command resolution and explains reopening existing terminals/VS Code. Open terminal places the bundled launchers first for that session.

## Installation and removal

Windows NSIS current-user installer checks OS build, handles Visual C++ and WebView2 prerequisites, enables CLI integration, and exposes uninstall cleanup choices. macOS pkg installs the app and enables integration only for the logged-in account; a resource-bundled uninstall utility removes integration and moves the app to Trash.

Uninstall requires stopped services and preserves coursework/data by default. Deleting data and deleting coursework are separate explicit choices. Shared prerequisites are not removed. Installation/update/removal must not terminate unrelated PHP, Nginx, or MySQL processes.

## Updates

The source repository and download location are `https://github.com/eimg/fairway-xmp`. Version 0.1.0-alpha.1 is the published unsigned prerelease; 0.1.0-alpha.2 is being tested on both platforms with no publication yet. Unsigned builds offer links to the releases page and explain manual backups and service shutdown. Alpha releases include checksums and corresponding bundled-tool source; they are not marked as the latest stable release. The default signed-update endpoint is `https://github.com/eimg/fairway-xmp/releases/latest/download/latest.json`. Release overlays configure the HTTPS endpoint and Tauri public verification key; signed artifacts are mandatory for the updater. Checks are student-triggered. Before installation, Fairway XMP starts MySQL if necessary to make a student-database backup, then gracefully stops services. Runtime and UI are updated together. The only major database transition currently implemented is 8.4 → 9.7, with a stopped physical backup. Other major transitions require a separate reviewed migration strategy.

Updates actions share equal-height buttons in a wrapping row. Long version labels and file paths must fit at the 820×650 minimum desktop window size. The document root displays the actual platform path; browser previews show manifest versions but keep service controls disabled.

Installer upgrades must also preserve data and coursework. Release testing must verify clean-machine operation without Homebrew, OS signing/notarization, Windows prerequisites, non-ASCII/space-containing user paths, uninstall, update recovery, and database compatibility. A successful compile is not proof of these checks.

## Verification

Core tests cover port validation/conflicts, path quoting, and idempotent PATH-block removal. Packaging tests reject archive traversal and bad checksums. The runtime smoke example verifies actual browser → Nginx → PHP → MySQL → response, phpMyAdmin rendering, database backup, graceful shutdown, settings persistence, and removal of isolated CLI integration without deleting coursework.
