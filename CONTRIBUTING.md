# Contributing to Fairway

The application uses Tauri 2, Rust, React, and TypeScript. Source checks run on Windows and macOS. A separate manual workflow builds Windows test installers as temporary Actions artifacts; CI does not publish GitHub Releases.

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

Preparation downloads binaries into the workspace, verifies checksums, bundles required libraries, relocates macOS load commands, and checks PHP extensions. It does not install Homebrew or start services. macOS inputs use ARM macOS 15 bottles; Windows inputs use official ZIP distributions. Runtime files and caches are intentionally ignored by Git.

Refreshing the lock trusts upstream HTTPS for inputs without independently published SHA256 checksums and records their SHA256. Review those inputs and available upstream signatures before a classroom release. Normal builds require the reviewed lock and reject changed inputs.

For native UI testing without touching your real user folders, debug builds accept both `FAIRWAY_DEV_HOME` and `FAIRWAY_DEV_DATA`; use absolute workspace paths. `FAIRWAY_RUNTIME_DIR` overrides the debug runtime directory. These folder overrides are disabled in release builds.

## Verify

```sh
npm run build
npm run test:scripts
cargo test --manifest-path src-tauri/Cargo.toml --no-default-features --locked
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --locked -- -D warnings
cargo run --manifest-path src-tauri/Cargo.toml --example smoke --no-default-features --locked -- /absolute/path/to/isolated-test
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

Windows setup uses current-user NSIS hooks and an offline WebView2 prerequisite. macOS packaging creates a pkg plus the app's bundled uninstall utility. Unsigned output is a verification build, not a ready-to-distribute classroom release. `scripts/package_macos.sh` supports `APPLE_SIGNING_IDENTITY`, `APPLE_INSTALLER_IDENTITY`, and `APPLE_NOTARY_PROFILE` for distribution signing and notarization.

To configure a signed updater release:

```sh
python3 scripts/configure_release.py --public-key /path/to/updater.pub
cargo tauri build --config artifacts/tauri.release.json
```

Set Tauri's signing key environment variables on the builder. Updater signatures are separate from Apple/Windows distribution certificates. The release owner supplies the public key. The default feed is `https://github.com/eimg/fairway-xmp/releases/latest/download/latest.json`; unsigned builds link to the releases page for manual downloads. Ship all runtime resources with every release, preserve the application resource location, and verify installer/updater behavior on both OSes.

## Publishing a future release

For an unsigned x64 Windows test installer, dispatch the **Windows test installer** workflow with `gh workflow run windows-test-build.yml --ref main`. It checks the source, stages the pinned runtime, exercises real services and cleanup, then builds an NSIS installer. Download its `fairway-windows-x64-test` artifact with `gh run download RUN_ID --name fairway-windows-x64-test --dir artifacts/windows-test`. The artifact includes its SHA256, source commit, and proposed Windows runtime checksum lock for review. It expires after 14 days and does not create a GitHub Release.

Binary releases are held until native testing is complete. Publish them only from a reviewed commit after verifying installation, updates, backups, and removal on the supported systems.

The dedicated repository is [eimg/fairway-xmp](https://github.com/eimg/fairway-xmp). A future GitHub release should include:

- Signed Windows and signed/notarized macOS installers.
- Tauri updater artifacts and their signatures.
- A `latest.json` file with the release version, notes, publication date, and supported platform entries. Each entry needs its signature and an HTTPS URL to the corresponding release asset. Windows uses `windows-x86_64`; Apple Silicon uses `darwin-aarch64`.
- Checksums and the corresponding source archives required by bundled components.

Keep the updater private key outside the repository. The public key used by installed clients must match the key signing update artifacts. Build the macOS app with the release overlay, then run the packaging script to produce its installer. Do not upload unsigned test builds as releases.

See [PROJECT_SPEC.md](PROJECT_SPEC.md) for behavior and [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) for component licenses.
