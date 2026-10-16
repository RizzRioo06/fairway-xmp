#!/bin/bash
# Only used on a disposable GitHub-hosted release runner. Never enable xtrace.
set -euo pipefail
: "${RUNNER_TEMP:?}"
: "${GITHUB_ENV:?}"
: "${APPLE_CERTIFICATES_P12:?Missing Apple certificate secret}"
: "${APPLE_CERTIFICATES_PASSWORD:?Missing Apple certificate password}"
: "${APPLE_API_PRIVATE_KEY:?Missing notarization private key}"
: "${APPLE_API_KEY:?Missing notarization Key ID}"
: "${APPLE_API_ISSUER:?Missing notarization Issuer ID}"
umask 077
KEYCHAIN="$RUNNER_TEMP/fairway-signing.keychain-db"
CERTIFICATE="$RUNNER_TEMP/fairway-identities.p12"
API_KEY="$RUNNER_TEMP/fairway-notary.p8"
KEYCHAIN_PASSWORD=$(openssl rand -base64 48)
# Preserve the runner's trust/identity search list and append our temporary one.
security list-keychains -d user > "$RUNNER_TEMP/fairway-keychains.txt"
printf '%s' "$APPLE_CERTIFICATES_P12" | base64 --decode > "$CERTIFICATE"
printf '%s' "$APPLE_API_PRIVATE_KEY" > "$API_KEY"
security create-keychain -p "$KEYCHAIN_PASSWORD" "$KEYCHAIN"
security set-keychain-settings -lut 21600 "$KEYCHAIN"
security unlock-keychain -p "$KEYCHAIN_PASSWORD" "$KEYCHAIN"
python3 - "$KEYCHAIN" "$RUNNER_TEMP/fairway-keychains.txt" <<'PY'
import shlex, subprocess, sys
subprocess.run(['security', 'list-keychains', '-d', 'user', '-s', *shlex.split(open(sys.argv[2]).read()), sys.argv[1]], check=True)
PY
security import "$CERTIFICATE" -k "$KEYCHAIN" -P "$APPLE_CERTIFICATES_PASSWORD" -T /usr/bin/codesign -T /usr/bin/productsign -T /usr/bin/security
security set-key-partition-list -S apple-tool:,apple:,codesign: -s -k "$KEYCHAIN_PASSWORD" "$KEYCHAIN" >/dev/null
security find-identity -v -p codesigning "$KEYCHAIN"
xcrun notarytool store-credentials fairway-release --key "$API_KEY" --key-id "$APPLE_API_KEY" --issuer "$APPLE_API_ISSUER" --keychain "$KEYCHAIN"
# notarytool searches the user keychain search list for this profile.
printf 'APPLE_NOTARY_PROFILE=fairway-release\n' >> "$GITHUB_ENV"
rm -f "$CERTIFICATE" "$API_KEY"
