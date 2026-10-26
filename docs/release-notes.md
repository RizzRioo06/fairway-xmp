# Fairway XMP 0.1.0-rc.5

Prepared release notes. RC 5 has not been built or published.

RC 5 adds optional Node.js management on macOS and Windows and the native fnm CLI on Linux. It carries forward RC 4’s PHP settings, service versions and PHP client extensions.

- **Native fnm on every platform:** fnm 1.39.0 is bundled as its own CLI tool. Node is downloaded only when requested; existing Node managers remain available. Installed Node versions and their global npm packages live outside the app and survive Fairway updates. The bundled fnm executable updates with Fairway.
- **Existing fnm stays in control:** existing fnm installations and populated stores are read-only in Fairway, with storage details and CLI guidance. Fairway avoids shadowing an external fnm; an explicit handover removes prior Fairway activation while preserving the default and installed versions.
- **Node.js settings on macOS and Windows:** see the detected Node version, manager and executable path in a separate settings card above Command-line tools. Install the latest patch of each major from Node 20 onward, including non-LTS majors, activate installed versions for new terminals, or uninstall versions in a Fairway-owned fnm store. Install preserves an existing Node selection; when none is detected, Install & use also activates the new version. No automatic project switching is added.
- **System tools in embedded terminals:** macOS terminals import exported settings from the user’s interactive login Zsh environment, including nvm and Homebrew paths. Windows refreshes system and user environment values before opening new terminals. Fairway’s PHP and Composer remain available alongside existing tools.
- **Clearer desktop update progress:** release notes show a four-line preview with Show more / Show less for longer notes. Download and installation progress stays beside Install update below the notes.

Node installation is optional for Fairway’s PHP services, desktop data browser and ordinary CLI. Linux users initialize fnm directly in their shell; Fairway does not add automatic Node activation there. On desktops, choosing Use affects new terminals; existing terminals and development servers retain their current selection until restarted. Uninstalling a Node version also removes its global npm packages.

See [Node.js and fnm](node-tools.md) for setup, storage and behavior. Linux includes phpMyAdmin and MySQL command-line tools; users can install additional database browsers independently.

Windows installers currently have no Authenticode signature; desktop updates use separate cryptographic signatures. Linux remains a terminal edition with manual updates. RC 5 installer and updater lifecycle checks remain to be run on the release artifacts before publication.
