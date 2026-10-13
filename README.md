# Fairway XMP

A simple PHP development environment for Windows and Apple Silicon Macs. Start your web server and database from one control panel, then use your usual editor and terminal. No Docker setup required.

![Fairway control panel with PHP, Nginx, MySQL, and phpMyAdmin running](docs/images/control-panel.png)

## Downloads

**Installers are not available yet.** Windows and macOS downloads will appear on the [Releases page](https://github.com/eimg/fairway-xmp/releases) after testing is complete.

## What’s included

- **PHP 8.4** with common extensions for PHP projects and Laravel.
- **Composer** for installing PHP packages.
- **Nginx** for serving your projects.
- **MySQL 8.4 LTS** for local databases.
- **phpMyAdmin** for managing tables and data in your browser.
- Start, stop, and restart controls, editable ports, service logs, and database backups.

PHP and Composer are added to your user PATH so you can use them from a terminal. Your projects live in the lowercase `fairway/www` folder inside your home folder.

## Supported computers

| Platform | Requirements |
| --- | --- |
| Windows | Windows 10 22H2 or Windows 11, 64-bit Intel/AMD |
| macOS | macOS 15 or later, Apple Silicon (M-series) |

Intel Macs and Linux are not supported. Install Node.js separately if your course or project needs it.

## Your first PHP project

Once an installer is available:

1. Install Fairway, then reopen your terminal and VS Code.
2. Open Fairway and click **Start all**. Wait for **All services ready**.
3. Click **Open folder**. Inside `fairway/www`, create a folder named `lesson-01`.
4. Add an `index.php` file:

   ```php
   <?php
   echo 'Hello from Fairway!';
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

Use these settings in your PHP projects:

| Setting | Value |
| --- | --- |
| Host | `127.0.0.1` |
| Port | `13306` |
| Database | `fairway` |
| Username | `fairway` |
| Password | `fairway` |

You can create additional databases in phpMyAdmin. These credentials are for your local lessons; Fairway’s services accept connections from your computer only.

Click **Back up databases** to save a SQL backup. Find saved backups under **Settings → Open backups**. You can import a backup through phpMyAdmin.

## Ports and settings

| Service | Default port |
| --- | --- |
| Website | `8800` |
| phpMyAdmin | `8801` |
| MySQL | `13306` |
| PHP’s internal connection to Nginx | `19000` |

If another application is using a port, Fairway explains the conflict. Stop all services, change the port in **Settings**, and save. Saved ports stay the same when you reopen Fairway. Update your project’s database connection settings if you change the MySQL port.

## Finishing a lesson

Click **Stop all** or quit Fairway to stop its services. Minimize the window if you want them to keep running.

Use **Updates** to check for a new version. Updates preserve your projects and database data; before an in-app update, Fairway backs up your databases and stops services.

To uninstall, use Windows’ installed-apps settings or **Uninstall Fairway.command** inside the macOS application bundle. The uninstaller removes Fairway’s PHP and Composer PATH entries. Your projects and database data are kept by default; deleting them is a separate choice.

## Help and license

For problems or suggestions, [open an issue](https://github.com/eimg/fairway-xmp/issues). Include your operating system, what you tried, and any error shown in Fairway.

Fairway’s application code is available under the [MIT license](LICENSE). Included tools have [their own licenses](THIRD_PARTY_NOTICES.md). To work on Fairway itself, see [Contributing](CONTRIBUTING.md).
