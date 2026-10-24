# Fairway XMP 0.1.0-rc.4

RC 4 adds persistent PHP settings, updates bundled services, and expands PHP client support for projects that use external services.

- **PHP settings for each version:** adjust memory, execution time, upload size, timezone, displayed errors and session upload progress. Overrides survive app and PHP patch updates. Reset to defaults affects the selected version only; configuring another version does not switch the active interpreter.
- **Coordinated web limits:** upload size also configures PHP POST and Nginx request limits. Nginx's response timeout follows the PHP execution limit plus 30 seconds; PHP unlimited mode gets a 24-hour Nginx response-idle timeout. Session upload progress remains enabled by default.
- **Updated services:** Nginx 1.30.6, MySQL 9.7.4 and Mailpit 1.32.0. Bundled PHP remains at 8.2.35, 8.3.36, 8.4.27 and 8.5.12.
- **External PostgreSQL support:** every bundled PHP version includes `pdo_pgsql` and `pgsql` for connections to a server supplied by the user. PostgreSQL server packaging and Data browser integration are not included in this release.
- **PHP clients and workers:** sockets, SOAP and LDAP are available across bundled PHP versions. macOS/Linux also include PCNTL/POSIX for CLI workers. Redis projects can install Predis through Composer and connect to their own Redis server.
- **Linux portability:** the terminal runtime passes automated checks on Ubuntu 22.04/24.04, Debian 12, Fedora and Arch Linux containers. Linux remains a terminal edition.

PHP settings apply to websites, CLI PHP and Composer for the active version. Stop Nginx/PHP before saving active settings, then restart web services and any long-running project commands to use the new values. Other PHP versions remain available under the collapsed version selector.

Windows installers currently have no Authenticode signature; desktop updates have separate cryptographic signatures. Linux is the terminal edition and uses manual updates.

Existing macOS and Windows installations can use **Check for updates** to install RC 4. Linux users can install the new `.run` package or archive; projects, databases and backups are preserved during supported upgrades. Download links and setup instructions are in the [README](https://github.com/eimg/fairway-xmp#downloads).
