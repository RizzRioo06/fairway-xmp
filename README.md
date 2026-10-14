# Fairway XMP

A PHP development environment for Windows, Apple Silicon Macs and Linux x86_64. Manage your web server and database from a desktop panel on Windows/macOS or a terminal dashboard on Linux, write code in your preferred editor, and manage your data in phpMyAdmin.

![Fairway XMP control panel with PHP, Nginx, MySQL, and phpMyAdmin running](docs/images/control-panel.png)

## Downloads

Current release: **[v0.1.0-alpha.3](https://github.com/eimg/fairway-xmp/releases/tag/v0.1.0-alpha.3)**.

| Computer | Requirements | Download |
| --- | --- | --- |
| Windows | Windows 10 22H2 or Windows 11, x64 | [Windows installer](https://github.com/eimg/fairway-xmp/releases/download/v0.1.0-alpha.3/Fairway-XMP-0.1.0-alpha.3-windows-x64-setup.exe) |
| Mac | macOS 15 or later, Apple Silicon (M-series) | [Mac installer](https://github.com/eimg/fairway-xmp/releases/download/v0.1.0-alpha.3/Fairway-XMP-0.1.0-alpha.3-macos-arm64-unsigned.pkg) |
| Linux | x86_64, glibc 2.35 or newer | [Terminal installer](https://github.com/eimg/fairway-xmp/releases/download/v0.1.0-alpha.3/Fairway-XMP-0.1.0-alpha.3-linux-x86_64.run) · [Archive](https://github.com/eimg/fairway-xmp/releases/download/v0.1.0-alpha.3/Fairway-XMP-0.1.0-alpha.3-linux-x86_64.tar.gz) |

Alpha 3 reuses tested builds; the installed apps still report `0.1.0-alpha.2` internally. Download the installer for your computer and follow its prompts. The installers are unsigned, so Windows may show an unknown-publisher warning. If macOS blocks the installer or app, follow [Apple’s instructions for opening an app from an unidentified developer](https://support.apple.com/en-us/102445).

[Linux install and usage guide](#linux-terminal-edition) · [Windows/macOS first project](#your-first-php-project)

## Linux terminal edition

Linux uses a terminal dashboard (TUI) and command-line tools. It requires **x86_64 and glibc 2.35 or newer**; Alpine/musl and ARM are not supported. No desktop, Docker, systemd service, or separate PHP/MySQL installation is required. Install and run Fairway as your normal user, **without `sudo`**.

### Install and open the dashboard

Download the `.run` installer from the table above, or use these commands in a new download folder:

```sh
curl -fLO https://github.com/eimg/fairway-xmp/releases/download/v0.1.0-alpha.3/Fairway-XMP-0.1.0-alpha.3-linux-x86_64.run
curl -fLO https://github.com/eimg/fairway-xmp/releases/download/v0.1.0-alpha.3/SHA256SUMS.txt
sha256sum --ignore-missing -c SHA256SUMS.txt &&
  sh Fairway-XMP-0.1.0-alpha.3-linux-x86_64.run
```

The checksum command should show the `.run` file as **OK**. If verification fails, download the installer again before running it. Once downloaded, installation works offline. The published alpha 3 installer contains PHP 8.5 only and reports alpha 2 internally.

Reopen your terminal, then run:

```sh
fairway
```

Or open it immediately using the installation path:

```sh
"$HOME/.local/share/fairway-xmp/current/bin/fairway"
```

The installer enables `fairway`, `php`, and `composer` in Bash/Zsh. Services stay stopped until you start them. In the dashboard, press **`a`** to start all and wait for green **Ready** indicators. The website and phpMyAdmin URLs appear in the dashboard.

| Key | Action |
| --- | --- |
| ↑ / ↓ | Select Nginx or MySQL |
| `s` / `x` / `r` | Start / stop / restart the selected service |
| `a` / `z` | Start / stop all services |
| `l` | Show the selected service's recent logs |
| `f` / Esc | Show PHP logs / hide the log view |
| `b` | Back up databases; MySQL must be running |
| `p` | Change ports after stopping all services |
| `q` | Close the dashboard; services keep running |
| `Q` | Stop services and shut down Fairway |

**Closing the terminal or pressing `q` leaves services running.** Reopen the dashboard with `fairway`, or shut everything down with `fairway shutdown`. Nginx manages PHP-FPM automatically; PHP and Composer commands work while web services are stopped.

### Create your first PHP project

Put projects in `~/fairway/www`. For example:

```sh
mkdir -p "$HOME/fairway/www/hello"
cat > "$HOME/fairway/www/hello/index.php" <<'PHP'
<?php
echo 'Hello from Fairway XMP!';
PHP
fairway start
```

Open [http://127.0.0.1:8800/hello/](http://127.0.0.1:8800/hello/) in your browser, or check the response in your terminal:

```sh
curl http://127.0.0.1:8800/hello/
```

Use the website port shown by `fairway ports` if you changed it. phpMyAdmin is at [http://127.0.0.1:8801/](http://127.0.0.1:8801/) by default, with username **fairway** and password **fairway**.

### Everyday commands

You can use the CLI instead of the dashboard, including over SSH:

```sh
fairway start                # start all services
fairway status               # service status and project/data paths
fairway stop nginx           # stop Nginx and its PHP-FPM worker
fairway restart nginx
fairway start mysql          # run the database without the web server
fairway logs nginx
fairway logs php
fairway logs mysql
fairway stop                 # stop services; keep Fairway available
fairway shutdown             # stop services and the background controller
```

For scripts or a non-interactive terminal, use `fairway status --json`. Run `fairway --help` for the complete command list.

PHP and Composer use the bundled runtime. Reopen your editor and terminal after installation so they pick up the updated PATH:

```sh
php --version
composer --version
cd "$HOME/fairway/www/hello"
php index.php
```

If another PHP installation takes priority, use `fairway php --version` and `fairway composer --version` to choose Fairway explicitly. `fairway shell` opens a shell with Fairway's PHP and Composer first on PATH; type `exit` to leave that shell. Node.js/npm are installed separately when a project needs them.

### Databases and backups

Use host `127.0.0.1`, port `13306`, database `fairway`, username `fairway`, and password `fairway` in your project's database settings. Services accept connections from the same machine only.

```sh
fairway start mysql
fairway mysql fairway        # open a MySQL prompt; type exit to leave
fairway backup               # back up all student databases; prints the SQL file path
```

Backups exclude MySQL's system databases. You can also export one project database:

```sh
fairway mysqldump fairway > fairway-project.sql
```

To restore a Fairway SQL backup, keep MySQL running and import the file through phpMyAdmin, or use:

```sh
fairway mysql < "/absolute/path/to/your-backup.sql"
```

For the single-database export above, use `fairway mysql fairway < fairway-project.sql`. Restoring can replace existing tables. Make a fresh backup first.

### Change ports

The four defaults are website `8800`, phpMyAdmin `8801`, MySQL `13306`, and PHP-FPM `19000`. View them with `fairway ports`. To change them, stop all services, then supply four different ports in that order:

```sh
fairway stop
fairway ports 8810 8811 13307 19001
fairway start
```

The new website URL is `http://127.0.0.1:8810/`, and phpMyAdmin uses `http://127.0.0.1:8811/`. Update your project's MySQL connection to port `13307`. In the dashboard, `p` opens the same four-port prompt; a blank line cancels.

<details>
<summary>Advanced: switch PHP in newer builds</summary>

The published alpha 3 installer has PHP 8.5 only. These commands apply to newer builds that bundle PHP 8.0–8.5 and list `php-version` in `fairway --help`:

```sh
fairway stop
fairway php-version          # show the selected version
fairway php-version 8.4
fairway php --version
fairway composer --version
fairway start
```

One saved selection applies to websites, CLI PHP, and Composer. Stop all services before switching. PHP 8.5 is the default; switch back with `fairway php-version 8.5`. See the [PHP version test guide](docs/testing-php-versions.md) for development builds.

</details>

### Files, updates, and removal

| Location | Contents |
| --- | --- |
| `~/.local/share/fairway-xmp` | Installed program and bundled tools |
| `~/fairway/www` | Your projects |
| `~/.local/share/dev.fairway.environment` | Database files, settings, logs, CLI launchers, and backups |
| `~/.local/share/dev.fairway.environment/backups` | SQL backups and database upgrade backups |

If you set `XDG_DATA_HOME`, the data directory is `$XDG_DATA_HOME/dev.fairway.environment`. Use the paths printed by `fairway status` for your installation.

For an update, back up your databases, shut down Fairway, download the newer installer from the [release page](https://github.com/eimg/fairway-xmp/releases), and run it as your normal user. Keep the same installation prefix. The installer makes a database backup before switching builds and preserves projects and settings.

```sh
fairway start mysql
fairway backup
fairway shutdown
# Run the newer downloaded .run installer with sh.
```

Uninstall with `fairway uninstall`. It stops services, removes the installed program and managed shell integration, and **keeps projects, database files, and backups**.

<details>
<summary>Other install paths, archive installs, and shell integration</summary>

To choose a dedicated installation directory:

```sh
sh Fairway-XMP-0.1.0-alpha.3-linux-x86_64.run --prefix "$HOME/apps/fairway-xmp"
```

Use that same `--prefix` for updates. Changing the program's prefix does not move your projects or database data.

For the archive download:

```sh
tar -xzf Fairway-XMP-0.1.0-alpha.3-linux-x86_64.tar.gz
sh fairway-xmp/install.sh
```

You can also run `./fairway-xmp/bin/fairway` directly from the extracted folder. Run `./fairway-xmp/bin/fairway shutdown` before moving or deleting that folder. Programs use their bundled native libraries.

`fairway remove-cli` removes Fairway's managed Bash/Zsh profile blocks and tool launchers. To enable them again, use the absolute installed command:

```sh
"$HOME/.local/share/fairway-xmp/current/bin/fairway" install-cli
```

Fish users can use the absolute installed command, or add the data directory's `bin` folder to their shell's PATH themselves. `fairway shell` makes bundled PHP and Composer available for one shell session without editing profiles.

</details>

<details>
<summary>Use a browser when Fairway runs on an SSH host</summary>

Fairway's web services listen on the Linux host's loopback interface. In a terminal on your own computer, open a tunnel to that host:

```sh
ssh -L 18800:127.0.0.1:8800 -L 18801:127.0.0.1:8801 your-linux-host
```

Start Fairway on the Linux host and keep the SSH connection open. On your computer, visit `http://127.0.0.1:18800/hello/` or `http://127.0.0.1:18801/` for phpMyAdmin. Replace the remote ports if you changed them; Fairway remains accessible only through the local connection and tunnel.

</details>

### Common problems

| Problem | What to do |
| --- | --- |
| `fairway: command not found` | Reopen Bash/Zsh, or run `"$HOME/.local/share/fairway-xmp/current/bin/fairway"`. For a custom prefix, use its `current/bin/fairway`. |
| Wrong PHP in your editor or terminal | Reopen it, check `command -v php`, or use `fairway php` / `fairway shell` explicitly. |
| A port is already in use | Check `fairway status`, stop Fairway's services, and select unused ports with `fairway ports`. |
| A service will not start | Read `fairway logs nginx`, `fairway logs php`, or `fairway logs mysql`. For controller startup failures, inspect `logs/supervisor.log` in the data directory. |
| The dashboard needs an interactive terminal | Use a real terminal for `fairway`; use CLI commands such as `fairway status --json` in scripts. |
| Another Fairway installation owns the services | Run `shutdown` with that installation's original Fairway command before using a different installation. |
| Unsupported architecture or glibc | Check `uname -m` and `getconf GNU_LIBC_VERSION`; use Linux x86_64 with glibc 2.35+. |

The remaining sections describe the Windows/macOS desktop panels; the tool versions and database connection defaults are shared with Linux.

## What’s included

- **PHP 8.5.11** with common extensions for PHP projects and Laravel.
- **Composer 2.10.3** for installing PHP packages.
- **Nginx 1.30.5** for serving your projects.
- **MySQL 9.7.2 LTS** for local databases.
- **phpMyAdmin 5.2.3** for managing tables and data in your browser.

The control panel provides service controls, port settings, logs, and database backups.

## Your first PHP project

1. Open Fairway XMP and click **Start all**. Wait for **All services ready**.
2. Click **Open folder** to open your project folder, `fairway/www` in your home folder.
3. Create a folder named `hello` and add an `index.php` file:

   ```php
   <?php
   echo 'Hello from Fairway XMP!';
   ```

4. Open [http://127.0.0.1:8800/hello/](http://127.0.0.1:8800/hello/) in your browser.

Closing or minimizing the window keeps Fairway XMP and its services running. Use its macOS menu bar or Windows system tray icon to reopen the panel, start/stop all services or individual Nginx/MySQL services, open phpMyAdmin, or **Quit**. Nginx automatically manages its PHP worker; PHP CLI works without starting services. Quit gracefully stops all services before exiting.

## PHP and Composer in your terminal

After installation, reopen your terminal and editor so they can find PHP and Composer:

```sh
php --version
composer --version
```

Manage this integration under **Settings → Command-line tools**. You can also use **Open terminal** in the control panel.

Development builds with PHP 8.0 through 8.5 let you switch under **Settings → Service Settings → Advanced settings** after stopping all services. Select the version and click **Save**. The selection controls websites, terminal PHP, and Composer, and is saved for the next launch. On Linux, use `fairway php-version 8.0` through `fairway php-version 8.5` after `fairway stop all`. See the [step by step test guide](docs/testing-php-versions.md). The published alpha 3 installers reuse alpha 2 binaries and contain PHP 8.5 only. PHP 8.0 and 8.1 are end of life; use newer versions for internet-facing projects.

For projects with their own development server, run the usual command, such as Laravel’s `php artisan serve`.

## Your database

Click **Open phpMyAdmin**, or visit [http://127.0.0.1:8801/](http://127.0.0.1:8801/). Sign in with username **fairway** and password **fairway**.

Use these connection settings in your PHP projects:

| Setting | Value |
| --- | --- |
| Host | `127.0.0.1` |
| Port | `13306` |
| Database | `fairway` |
| Username | `fairway` |
| Password | `fairway` |

You can create additional databases in phpMyAdmin. Services accept connections from your computer only.

To save a SQL backup, click **Back up databases**. Find saved backups under **Settings → Open backups**, and import them through phpMyAdmin when needed.

## Ports

| Service | Default port |
| --- | --- |
| Website | `8800` |
| phpMyAdmin | `8801` |
| MySQL | `13306` |
| PHP’s connection to Nginx | `19000` |

If a port is already in use, Fairway XMP shows the conflict. Stop all services, change the port in **Settings**, and save. If you change the MySQL port, use the new port in your project’s database settings.

## Updates and removal

Open **Updates → View downloads** to get a newer installer. Stop all services and quit Fairway XMP before installing it. Your projects and database data are kept.

To uninstall:

- **Windows:** open **Settings → Apps**, select **Fairway XMP**, and choose **Uninstall**.
- **Mac:** quit Fairway XMP, then open **Applications** in Finder. Right-click **Fairway XMP → Show Package Contents**, open **Contents → Resources**, and double-click **Uninstall Fairway XMP.command**.

The uninstaller removes the app and its command-line integration. Your projects and database data are kept unless you choose to delete them.

## Help and license

For problems or suggestions, [open an issue](https://github.com/eimg/fairway-xmp/issues). Include your operating system, what you tried, and any error shown in Fairway XMP.

Fairway XMP’s application code uses the [MIT license](LICENSE). Included tools have [their own licenses](THIRD_PARTY_NOTICES.md). Release checksums and bundled-tool sources are available on the [release page](https://github.com/eimg/fairway-xmp/releases/tag/v0.1.0-alpha.3). For contributing to the app, see [Contributing](CONTRIBUTING.md).
