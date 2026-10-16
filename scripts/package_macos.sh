#!/bin/sh
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
APP="$ROOT/src-tauri/target/release/bundle/macos/Fairway XMP.app"
ARTIFACTS="$ROOT/artifacts"
STAGE="$ROOT/.runtime-work/pkg-root"
COMPONENTS="$ROOT/.runtime-work/fairway-components.plist"
[ -d "$APP" ] || { echo 'Build the Tauri application bundle first.' >&2; exit 1; }
mkdir -p "$ARTIFACTS" "$ROOT/.runtime-work"
VERSION=$(node -e 'console.log(JSON.parse(require("fs").readFileSync(process.argv[1])).version)' "$ROOT/package.json")
if [ "${FAIRWAY_SIGNED_RELEASE:-0}" = 1 ]; then
  : "${APPLE_SIGNING_IDENTITY:?Developer ID Application identity is required}"
  : "${APPLE_INSTALLER_IDENTITY:?Developer ID Installer identity is required}"
  : "${APPLE_NOTARY_PROFILE:?A validated notarization profile is required}"
  : "${TAURI_SIGNING_PRIVATE_KEY:?Updater signing key is required}"
fi
# Sign/notarize the final app itself. Both the pkg and updater archive must
# contain these exact stapled bytes; notarizing only the pkg is insufficient.
if [ -n "${APPLE_SIGNING_IDENTITY:-}" ]; then
  python3 "$ROOT/scripts/sign_macos.py" "$APP" "$APPLE_SIGNING_IDENTITY"
fi
if [ -n "${APPLE_NOTARY_PROFILE:-}" ]; then
  ZIP="$ROOT/.runtime-work/notarize-app.zip"
  ditto -c -k --keepParent "$APP" "$ZIP"
  xcrun notarytool submit "$ZIP" --keychain-profile "$APPLE_NOTARY_PROFILE" --wait --timeout 30m
  xcrun stapler staple "$APP"
  xcrun stapler validate "$APP"
  codesign --verify --deep --strict "$APP"
  spctl --assess --type execute --verbose=2 "$APP"
  rm -f "$ZIP"
fi
rm -rf "$STAGE"
mkdir -p "$STAGE/Applications"
ditto "$APP" "$STAGE/Applications/Fairway XMP.app"
pkgbuild --analyze --root "$STAGE" "$COMPONENTS"
plutil -replace '0.BundleIsRelocatable' -bool NO "$COMPONENTS"
UNSIGNED="$ARTIFACTS/fairway-xmp-$VERSION-macos-arm64-unsigned.pkg"
PKG="$ARTIFACTS/fairway-xmp-$VERSION-macos-arm64.pkg"
pkgbuild --root "$STAGE" --component-plist "$COMPONENTS" --identifier dev.fairway.environment --version "$VERSION" --install-location / --scripts "$ROOT/packaging/macos/scripts" "$UNSIGNED"
rm -rf "$STAGE"
rm -f "$COMPONENTS"
if [ -n "${APPLE_INSTALLER_IDENTITY:-}" ]; then
  productsign --sign "$APPLE_INSTALLER_IDENTITY" "$UNSIGNED" "$PKG"
  pkgutil --check-signature "$PKG"
  rm -f "$UNSIGNED"
fi
if [ -n "${APPLE_NOTARY_PROFILE:-}" ]; then
  xcrun notarytool submit "$PKG" --keychain-profile "$APPLE_NOTARY_PROFILE" --wait --timeout 30m
  xcrun stapler staple "$PKG"
  xcrun stapler validate "$PKG"
  spctl --assess --type install --verbose=2 "$PKG"
fi
if [ "${FAIRWAY_SIGNED_RELEASE:-0}" = 1 ]; then
  ARCHIVE="$ARTIFACTS/fairway-xmp-$VERSION-macos-arm64.app.tar.gz"
  COPYFILE_DISABLE=1 tar -czf "$ARCHIVE" -C "$(dirname "$APP")" 'Fairway XMP.app'
  # Pin the signature to the version as well as the archive bytes.
  npm exec --yes --package @tauri-apps/cli@2.12.2 -- tauri signer sign --app-version "$VERSION" "$ARCHIVE"
fi
