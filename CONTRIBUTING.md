# Contributing to Fairway XMP

The application uses Tauri 2, Rust, React, and TypeScript. Source checks run on Windows and macOS. A separate manual workflow builds Windows installers as Actions artifacts; CI does not publish GitHub Releases. After testing, publish the exact downloaded installer rather than rebuilding it.

## Development

Install Node 24, Rust stable (at least 1.90), and the platform Tauri development prerequisites. Windows builders need Microsoft's C++ Build Tools; Mac builders need Xcode Command Line Tools. Students do not need these build tools.

```sh
npm ci
cargo install tauri-cli --version 2.12.2 --locked
npm run dev                   # browser interface preview; service controls disabled
cargo tauri dev               # native app; real services require staged runtime
```

The browser preview explicitly labels itself and never simulates service success.

To stage native runtime resources (Python 3.12+):

```sh
python3 scripts/prepare_runtime.py --refresh-lock
# If a download was interrupted: python3 scripts/prepare_runtime.py --resume-lock
# Review and commit runtime-downloads.lock.json after initial preparation.
python3 scripts/prepare_runtime.py
```

Preparation downloads binaries into the workspace, verifies checksums, bundles required libraries, relocates macOS load commands, and checks PHP extensions. It does not install Homebrew or start services. macOS PHP/MySQL inputs use ARM macOS 15 bottles; stable Nginx is compiled locally from the pinned upstream source against pinned PCRE2/OpenSSL bottles and SDK zlib, using Xcode Command Line Tools. Its configure inputs are retained in `runtime/nginx/build.json`. Windows inputs use official ZIP distributions. Runtime files and caches are intentionally ignored by Git.

Alpha 2 is currently being tested for Mac and Windows. Corresponding-source packaging and release publication remain separate from installer builds. The published alpha 1 installers and README download links remain valid.

Refreshing the lock trusts upstream HTTPS for inputs without independently published SHA256 checksums and records their SHA256. Review those inputs and available upstream signatures before a classroom release. Normal builds require the reviewed lock and reject changed inputs.

For native UI testing without touching your real user folders, debug builds accept both `FAIRWAY_DEV_HOME` and `FAIRWAY_DEV_DATA`; use absolute workspace paths. `FAIRWAY_RUNTIME_DIR` overrides the debug runtime directory. These folder overrides are disabled in release builds.

## Verify

```sh
npm run build
npm run test:scripts
cargo test --manifest-path src-tauri/Cargo.toml --no-default-features --locked
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --locked -- -D warnings
cargo run --manifest-path src-tauri/Cargo.toml --example smoke --no-default-features --locked -- /absolute/path/to/isolated-test
# Optional MySQL migration check using the saved alpha 1 runtime:
cargo run --manifest-path src-tauri/Cargo.toml --example smoke --no-default-features --locked -- /absolute/path/to/isolated-upgrade /absolute/path/to/alpha1-runtime /absolute/path/to/alpha2-runtime
```

The smoke example starts real runtimes on available test ports and uses an isolated home/data directory. It never edits your real shell profile. Browser checks are in `scripts/check_ui.mjs` (requires Playwright and its Chromium browser).

## Package

```sh
# Windows, on a Windows builder:
cargo tauri build --bundles nsis

# Apple Silicon Mac:
cargo tauri build --bundles app
sh scripts/package_macos.sh
```

Windows setup uses current-user NSIS hooks and an offline WebView2 prerequisite. macOS packaging creates a pkg plus the app's bundled uninstall utility. Unsigned output can be published as an explicitly labeled alpha prerelease. Stable classroom releases require distribution signing and macOS notarization. `scripts/package_macos.sh` supports `APPLE_SIGNING_IDENTITY`, `APPLE_INSTALLER_IDENTITY`, and `APPLE_NOTARY_PROFILE` for distribution signing and notarization.

To configure a signed updater release:

```sh
python3 scripts/configure_release.py --public-key /path/to/updater.pub
cargo tauri build --config artifacts/tauri.release.json
```

Set Tauri's signing key environment variables on the builder. Updater signatures are separate from Apple/Windows distribution certificates. The release owner supplies the public key. The default feed is `https://github.com/eimg/fairway-xmp/releases/latest/download/latest.json`; unsigned builds link to the releases page for manual downloads. Ship all runtime resources with every release, preserve the application resource location, and verify installer/updater behavior on both OSes.

## Publishing a release

For an unsigned x64 Windows installer, dispatch the **Windows installer** workflow with `gh workflow run windows-test-build.yml --ref BUILD_COMMIT`. It checks the source, stages the pinned runtime, exercises real services and cleanup, and verifies an 8.4 → 9.7 database upgrade before building an NSIS installer. The alpha 1 migration fixtures retain its exact Windows versions and download checksums; the preparer stages them into a separate folder without replacing the current runtime.

Download its versioned artifact, for example `gh run download RUN_ID --name fairway-windows-x64-0.1.0-alpha.2 --dir artifacts/windows-alpha2`. It contains the installer, SHA256, exact build commit/run, runtime metadata, and application source archive. Actions retains it for 90 days; keep a local copy for later publication. Publish that tested installer unchanged. An application or runtime change requires another build; release notes or corresponding-source collection do not. The workflow never creates a GitHub Release.

The initial release is `v0.1.0-alpha.1`, an unsigned GitHub prerelease with manual updates. Publish alpha builds with clear installation limitations and never mark them as the latest stable release. Stable releases require installation, updates, backups, and removal checks on the supported systems.

On the Mac builder, after staging the locked runtime, run `python3 scripts/package_sources.py`. It verifies the cached bottles, saves their recipes and build metadata, downloads version-matched upstream source and patches, and creates a bundled-tool source archive under `artifacts/alpha-VERSION`. Review and commit the resulting `runtime-sources.lock.json`; subsequent runs verify those source checksums. Use `--source-ref BUILD_COMMIT` when collecting sources from a newer checkout for existing installers. The archive also contains the application source at that build commit, Windows PHP dependency sources identified by the official SBOM, and the runtime staging scripts. Source packages are distributed beside the installers; students only need their platform's installer.

GPL binary releases must provide the corresponding source code, including packaging patches and required dependency sources, beside the installers. An upstream project link alone does not replace that source distribution. `runtime-downloads.lock.json` and `runtime-sources.lock.json` record the exact inputs.

The dedicated repository is [eimg/fairway-xmp](https://github.com/eimg/fairway-xmp). A stable GitHub release should include:

- Signed Windows and signed/notarized macOS installers.
- Tauri updater artifacts and their signatures.
- A `latest.json` file with the release version, notes, publication date, and supported platform entries. Each entry needs its signature and an HTTPS URL to the corresponding release asset. Windows uses `windows-x86_64`; Apple Silicon uses `darwin-aarch64`.
- Checksums and the corresponding source archives required by bundled components.

Keep the updater private key outside the repository. The public key used by installed clients must match the key signing update artifacts. Build the macOS app with the release overlay, then run the packaging script to produce its installer. Unsigned alpha prereleases omit updater artifacts and `latest.json`; users install updates manually. Include both installers, SHA-256 checksums, pinned runtime metadata, and the bundled-tool source archive. Use `--prerelease --latest=false` when publishing an alpha with `gh release create`.

See [PROJECT_SPEC.md](PROJECT_SPEC.md) for behavior and [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) for component licenses.
