# Fairway XMP 0.1.0-rc.5

Prepared release notes. RC 5 has not been built or published.

RC 5 adds optional Node.js management and a standalone browser for data on Linux hosts. It carries forward RC 4’s PHP settings, service versions and PHP client extensions.

- **Native fnm on every platform:** fnm 1.39.0 is bundled as its own CLI tool. Node is downloaded only when requested; existing Node managers remain available. Installed Node versions and their global npm packages live outside the app and survive Fairway updates. The bundled fnm executable updates with Fairway.
- **Node.js settings on macOS and Windows:** see the detected Node version, manager and executable path in a separate settings card above Command-line tools. Install the latest patch of each major from Node 20 onward, including non-LTS majors, activate installed versions for new terminals, or uninstall versions managed by fnm. Install preserves an existing Node selection; when none is detected, Install & use also activates the new version. No automatic project switching is added.
- **System tools in embedded terminals:** macOS terminals import exported settings from the user’s interactive login Zsh environment, including nvm and Homebrew paths. Windows refreshes system and user environment values before opening new terminals. Fairway’s PHP and Composer remain available alongside existing tools.
- **Linux data browser:** `fairway data-browser` serves a bundled production frontend and Node server for MySQL, SQLite and external PostgreSQL databases. It requires Node 22+; missing or older Node produces setup instructions using the bundled fnm. First use downloads locked runtime dependencies with npm; later launches use the prepared cache offline. Use `--prepare` to set up dependencies ahead of time and `--port` to choose another port.
- **Clearer desktop update progress:** release notes show a four-line preview with Show more / Show less for longer notes. Download and installation progress stays beside Install update below the notes.
- **Remote access and update handling:** the Linux browser listens on loopback and prints a temporary authenticated access URL, usable through SSH forwarding. Stop it with Ctrl+C before updating or uninstalling; an active browser blocks those operations. Browser updates ship in subsequent Fairway releases, with separate dependency caches when the bundled browser or Node ABI changes.

Node installation is optional for Fairway’s PHP services, desktop data browser and ordinary CLI. Linux users initialize fnm directly in their shell; Fairway does not add automatic Node activation there. On desktops, choosing Use affects new terminals; existing terminals and development servers retain their current selection until restarted. Uninstalling a Node version also removes its global npm packages.

See [Node.js and fnm](node-tools.md) and [Linux data browser](linux-data-browser.md) for setup, storage and behavior. The Linux browser runs independently of Fairway’s service supervisor; `fairway shutdown` does not stop it. Its SQL snapshots are separate from full Fairway environment backups.

Windows installers currently have no Authenticode signature; desktop updates use separate cryptographic signatures. Linux remains a terminal edition with manual updates. RC 5 installer and updater lifecycle checks remain to be run on the release artifacts before publication.
