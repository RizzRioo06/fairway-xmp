Fairway XMP beta 2 adds an embedded terminal and a first-party data browser on macOS and Windows. Both are available from the sidebar, beside the existing Environment and Settings pages.

## Downloads

| Platform | Requirements | Download |
| --- | --- | --- |
| Windows | Windows 10 22H2 / Windows 11, x64 | [Windows installer](https://github.com/eimg/fairway-xmp/releases/download/v0.1.0-beta.2/fairway-xmp-0.1.0-beta.2-windows-x64-setup.exe) |
| macOS | macOS 15+, Apple Silicon | [Mac installer](https://github.com/eimg/fairway-xmp/releases/download/v0.1.0-beta.2/fairway-xmp-0.1.0-beta.2-macos-arm64-unsigned.pkg) |
| Linux | x86_64, glibc 2.35+ | [Terminal installer](https://github.com/eimg/fairway-xmp/releases/download/v0.1.0-beta.2/fairway-xmp-0.1.0-beta.2-linux-x86_64.run) · [Archive](https://github.com/eimg/fairway-xmp/releases/download/v0.1.0-beta.2/fairway-xmp-0.1.0-beta.2-linux-x86_64.tar.gz) |

[SHA-256 checksums](https://github.com/eimg/fairway-xmp/releases/download/v0.1.0-beta.2/SHA256SUMS.txt) · [Build information](https://github.com/eimg/fairway-xmp/releases/download/v0.1.0-beta.2/BUILD-INFO.json)

Students only need their platform's installer. [Mac/Windows bundled-tool sources](https://github.com/eimg/fairway-xmp/releases/download/v0.1.0-beta.2/fairway-xmp-0.1.0-beta.2-bundled-tool-sources.tar.gz) and [Linux corresponding sources](https://github.com/eimg/fairway-xmp/releases/download/v0.1.0-beta.2/fairway-xmp-0.1.0-beta.2-linux-x86_64-sources.tar.gz) are supplied alongside the binaries.

## Embedded terminal

- Open **Terminal** from the sidebar or press **Ctrl+backtick**. PHP and Composer are ready without enabling persistent PATH integration or starting services.
- Keep up to two independent shells open, choose a project folder for a new shell, and switch pages without losing running commands or output.
- Search output, adjust text size, expand the terminal, and interrupt commands with Ctrl+C. Closing a running tab asks for confirmation; quitting Fairway ends both command trees.
- macOS can remember about 1,000 commands in private app-owned history, with an opt-out and leading-space exclusion. Windows keeps Command Prompt's in-session history.

## Data browser

- Start MySQL directly from **Data browser**, create databases and tables, browse and edit rows, and work with table structure.
- Run and cancel SQL, with bounded result grids and exact large-number values. Destructive schema changes save a SQL backup first.
- Use `fairway db` for database administration and full SQL export/import. phpMyAdmin remains available beside the new browser.
- Windows fixes cover native file cleanup, terminal startup/shutdown, and Unicode paths for MySQL and PHP startup configuration using existing compatible path aliases.

Linux keeps its terminal edition and existing CLI; the new desktop pages are for macOS and Windows.

## Install or update

On a Mac, the installer quits a running Fairway XMP and stops services before replacing the app. On Windows, **Quit from the system tray before upgrading**; closing the window leaves Fairway running. The Linux installer handles its running copy:

```sh
sh fairway-xmp-0.1.0-beta.2-linux-x86_64.run
# Reopen your shell, then:
fairway
```

Projects, databases, settings, and backups are preserved. Sign into phpMyAdmin using **fairway / fairway**.

PHP 8.5.11 remains the default, with PHP 8.0.30, 8.1.34, 8.2.34, 8.3.35, and 8.4.26 available. Other bundled tools remain MySQL 9.7.2, Nginx 1.30.5, Composer 2.10.3, and phpMyAdmin 5.2.3.

## Verification and known limitations

These exact installers come from successful [candidate run 38848919567](https://github.com/eimg/fairway-xmp/actions/runs/38848919567), commit `764bccae0287af74cca0382359ab3de7853c5f48`. They were not rebuilt for publication. Automated checks passed for native database/terminal browser flows on Mac and Windows, SQL export/import/cancellation, real services and backups, installed upgrades/removal/reinstall, browser layouts, and five Linux distro runtimes.

The release owner authorized publication. Individual human acceptance checks remain unrecorded; the published candidate manifest preserves that pending state.

- Installers are unsigned and updates are manual. This is a prerelease, not the latest stable release.
- Windows MySQL requires filesystem paths compatible with the Windows ANSI code page or existing short-name aliases. Unsupported paths produce a clear error and preserve existing data; no data is relocated automatically.
- PHP 8.0/8.1 are unsupported legacy compatibility options. Their exact Windows dependency build/patch provenance remains unverified.
- Linux ARM and Alpine/musl are not supported.
