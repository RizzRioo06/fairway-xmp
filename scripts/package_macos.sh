#!/bin/sh
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
APP="$ROOT/src-tauri/target/release/bundle/macos/Fairway.app"
ARTIFACTS="$ROOT/artifacts"
STAGE="$ROOT/.runtime-work/pkg-root"
[ -d "$APP" ] || { echo 'Build the Tauri application bundle first.' >&2; exit 1; }
mkdir -p "$ARTIFACTS"
rm -rf "$STAGE"
mkdir -p "$STAGE/Applications"
ditto "$APP" "$STAGE/Applications/Fairway.app"
cp "$ROOT/packaging/macos/Uninstall Fairway.command" "$STAGE/Applications/Fairway.app/Contents/Resources/Uninstall Fairway.command"
chmod +x "$ROOT/packaging/macos/scripts/preinstall" "$ROOT/packaging/macos/scripts/postinstall" "$STAGE/Applications/Fairway.app/Contents/Resources/Uninstall Fairway.command"
if [ -n "${APPLE_SIGNING_IDENTITY:-}" ]; then
  # Sign innermost binaries first; never use --deep as a substitute for signing dependencies.
  python3 "$ROOT/scripts/sign_macos.py" "$STAGE/Applications/Fairway.app" "$APPLE_SIGNING_IDENTITY"
fi
VERSION=$(node -e 'console.log(JSON.parse(require("fs").readFileSync(process.argv[1])).version)' "$ROOT/package.json")
pkgbuild --root "$STAGE" --identifier dev.fairway.environment --version "$VERSION" --install-location / --scripts "$ROOT/packaging/macos/scripts" "$ARTIFACTS/Fairway-$VERSION-arm64-unsigned.pkg"
if [ -n "${APPLE_INSTALLER_IDENTITY:-}" ]; then
  productsign --sign "$APPLE_INSTALLER_IDENTITY" "$ARTIFACTS/Fairway-$VERSION-arm64-unsigned.pkg" "$ARTIFACTS/Fairway-$VERSION-arm64.pkg"
fi
if [ -n "${APPLE_NOTARY_PROFILE:-}" ]; then
  xcrun notarytool submit "$ARTIFACTS/Fairway-$VERSION-arm64.pkg" --keychain-profile "$APPLE_NOTARY_PROFILE" --wait
  xcrun stapler staple "$ARTIFACTS/Fairway-$VERSION-arm64.pkg"
fi
