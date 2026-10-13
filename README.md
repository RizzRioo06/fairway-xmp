# Fairway XMP

A simple PHP development environment for Windows and Apple Silicon Macs. Start your web server and database from one control panel, then use your usual editor and terminal. No Docker setup required.

![Fairway XMP control panel with PHP, Nginx, MySQL, and phpMyAdmin running](docs/images/control-panel.png)

## Downloads

**v0.1.0-alpha.2 is available for early testing.** Back up any important databases before using an alpha version.

| Computer | Download |
| --- | --- |
| Windows 10 22H2 / Windows 11, x64 | [Windows installer](https://github.com/eimg/fairway-xmp/releases/download/v0.1.0-alpha.2/Fairway-XMP-0.1.0-alpha.2-windows-x64-setup.exe) |
| macOS 15+, Apple Silicon | [Mac installer](https://github.com/eimg/fairway-xmp/releases/download/v0.1.0-alpha.2/Fairway-XMP-0.1.0-alpha.2-macos-arm64-unsigned.pkg) |

These alpha installers are unsigned. Windows may display an unknown-publisher warning. macOS may block the package or app; if you trust the download, follow [Apple’s instructions for opening an app from an unidentified developer](https://support.apple.com/en-us/102445).

Find release notes, SHA-256 checksums, and bundled-tool source code on the [alpha release page](https://github.com/eimg/fairway-xmp/releases/tag/v0.1.0-alpha.2).

If you installed a pre-alpha test build (version **0.1.0**, named **Fairway** or **Fairway XMP**), uninstall that test build first, choosing to keep your data and projects. This does not apply to other apps that happen to share the Fairway name.

## What’s included

- **PHP 8.5.11** with common extensions for PHP projects and Laravel.
- **Composer 2.10.3** for installing PHP packages.
- **Nginx 1.30.5 stable** for serving your projects.
- **MySQL 9.7.2 LTS** for local databases.
- **phpMyAdmin 5.2.3** for managing tables and data in your browser.
- Start, stop, and restart controls, editable ports, service logs, and database backups.

PHP and Composer are added to your user PATH so you can use them from a terminal. Your projects live in the lowercase `fairway/www` folder inside your home folder.

## Supported computers

| Platform | Requirements |
| --- | --- |
| Windows | Windows 10 22H2 or Windows 11, 64-bit Intel/AMD |
| macOS | macOS 15 or later, Apple Silicon (M-series) |

Intel Macs and Linux are not supported. Install Node.js separately if your course or project needs it.

Before installing, check that any existing `fairway` item in your home folder is a folder. A file with that name prevents Fairway XMP from creating `fairway/www`; move or rename the file first.

## Your first PHP project

1. Install Fairway XMP, then reopen your terminal and VS Code.
2. Open Fairway XMP and click **Start all**. Wait for **All services ready**.
3. Click **Open folder**. Inside `fairway/www`, create a folder named `lesson-01`.
4. Add an `index.php` file:

   ```php
   <?php
   echo 'Hello from Fairway XMP!';
   ```

5. Open [http://127.0.0.1:8800/lesson-01/](http://127.0.0.1:8800/lesson-01/) in your browser.

Use PHP and Composer in your terminal as usual:

```sh
php --version
composer --version
```

You can also run a project’s own PHP development server, including Laravel’s `php artisan serve`.

## Your database

Click **Open phpMyAdmin**, or visit [http://127.0.0.1:8801/](http://127.0.0.1:8801/). Sign in with username **fairway** and password **fairway**.

Root login is reserved for Fairway XMP’s internal database management. Use the `fairway` account in phpMyAdmin and your projects.

Use these settings in your PHP projects:

| Setting | Value |
| --- | --- |
| Host | `127.0.0.1` |
| Port | `13306` |
| Database | `fairway` |
| Username | `fairway` |
| Password | `fairway` |

You can create additional databases in phpMyAdmin. These credentials are for your local lessons; Fairway XMP’s services accept connections from your computer only.

Click **Back up databases** to save a SQL backup. Find saved backups under **Settings → Open backups**. You can import a backup through phpMyAdmin.

## Ports and settings

| Service | Default port |
| --- | --- |
| Website | `8800` |
| phpMyAdmin | `8801` |
| MySQL | `13306` |
| PHP’s internal connection to Nginx | `19000` |

If another application is using a port, Fairway XMP explains the conflict. Stop all services, change the port in **Settings**, and save. Saved ports stay the same when you reopen Fairway XMP. Update your project’s database connection settings if you change the MySQL port.

## Finishing a lesson

Click **Stop all** or quit Fairway XMP to stop its services. Minimize the window if you want them to keep running.

Alpha updates are installed manually. Open **Updates → View downloads**, download the newer installer, back up your databases, then stop all services and quit Fairway XMP before installing. Your projects and database data are kept.

When updating from alpha 1, the first MySQL start saves a copy of your stopped 8.4 database before upgrading it to 9.7. Keep that backup until you have checked your tables and data. Find it under **Settings → Open backups**.

To uninstall on Windows, use **Settings → Apps → Installed apps → Fairway XMP → Uninstall**.

On Mac, quit Fairway XMP, then open **Applications** in Finder. Right-click **Fairway XMP → Show Package Contents**, open **Contents → Resources**, and double-click **Uninstall Fairway XMP.command**.

The uninstaller removes Fairway XMP’s PHP and Composer PATH entries. Your projects and database data are kept by default; deleting them is a separate choice.

## Help and license

For problems or suggestions, [open an issue](https://github.com/eimg/fairway-xmp/issues). Include your operating system, what you tried, and any error shown in Fairway XMP.

Fairway XMP’s application code is available under the [MIT license](LICENSE). Included tools have [their own licenses](THIRD_PARTY_NOTICES.md). To work on Fairway XMP itself, see [Contributing](CONTRIBUTING.md).
