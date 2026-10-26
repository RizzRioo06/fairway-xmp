# Fairway XMP 0.1.0-rc.5

RC 5 adds optional Node.js management on macOS and Windows and the native fnm CLI on Linux. It carries forward RC 4’s PHP settings, service versions and PHP client extensions.

Published from [candidate run 40288383232](https://github.com/eimg/fairway-xmp/actions/runs/40288383232), source `d8646771670400591b2a5af743aa4b83d6a491da`, using the tested installers and updater payloads without rebuilding. The preview update feed offers RC 5 to installed macOS and Windows beta and RC clients; Linux updates remain manual.

- **Native fnm on every platform:** fnm 1.39.0 is bundled as its own CLI tool. Node is downloaded only when requested; existing Node managers remain available. Installed Node versions and their global npm packages live outside the app and survive Fairway updates. The bundled fnm executable updates with Fairway.
- **Existing fnm stays in control:** existing fnm installations and populated stores are read-only in Fairway, with storage details and CLI guidance. Fairway avoids shadowing an external fnm; an explicit handover removes prior Fairway activation while preserving the default and installed versions.
- **Node.js settings on macOS and Windows:** see the detected Node version, manager and executable path in a separate settings card above Command-line tools. Install the latest patch of each major from Node 20 onward, including non-LTS majors, activate installed versions for new terminals, or uninstall versions in a Fairway-owned fnm store. Install preserves an existing Node selection; when none is detected, Install & use also activates the new version. No automatic project switching is added.
- **Cancellable Node downloads and safer selection:** installations show progress with Cancel download, including after returning to Settings. Failed activation restores the previous fnm default; a completed download stays available to retry setup.
- **System tools in embedded terminals:** macOS terminals import exported settings from the user’s interactive login Zsh environment, including nvm and Homebrew paths. Windows refreshes system and user environment values before opening new terminals. Fairway’s PHP and Composer remain available alongside existing tools.
- **Reliable Windows Node activation:** Command Prompt and PowerShell select the managed Node version with Unicode characters, spaces and ampersands in storage paths.
- **Mail preview images:** supported remote images load automatically, with Retry image placeholders for failures. Downloaded images remain available when switching between HTML and Text views.
- **Clearer desktop update progress:** release notes show a four-line preview with Show more / Show less for longer notes. Download and installation progress stays beside Install update below the notes.

Node installation is optional for Fairway’s PHP services, desktop data browser and ordinary CLI. Linux users initialize fnm directly in their shell; Fairway does not add automatic Node activation there. On desktops, choosing Use affects new terminals; existing terminals and development servers retain their current selection until restarted. Uninstalling a Node version also removes its global npm packages.

See [Node.js and fnm](https://github.com/eimg/fairway-xmp/blob/main/docs/node-tools.md) for setup, storage and behavior. Linux includes phpMyAdmin and MySQL command-line tools; users can install additional database browsers independently.

Windows installers currently have no Authenticode signature; desktop updates use separate cryptographic signatures. The macOS app and installer are Developer ID signed and notarized by Apple. Automated installer lifecycle checks passed on all three platforms, and desktop updater lifecycle checks passed on macOS and Windows.
