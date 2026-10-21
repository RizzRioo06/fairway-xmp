#!/bin/bash
set -euo pipefail
: "${RUNNER_TEMP:?}"
status=0
if [ -f "$RUNNER_TEMP/fairway-keychains.txt" ]; then
  python3 - "$RUNNER_TEMP/fairway-keychains.txt" <<'PY' || status=$?
import shlex, subprocess, sys
subprocess.run(['security', 'list-keychains', '-d', 'user', '-s', *shlex.split(open(sys.argv[1]).read())], check=True)
PY
fi
if [ -f "$RUNNER_TEMP/fairway-signing.keychain-db" ]; then
  security delete-keychain "$RUNNER_TEMP/fairway-signing.keychain-db" || status=$?
fi
rm -f "$RUNNER_TEMP/fairway-identities.p12" "$RUNNER_TEMP/fairway-notary.p8" "$RUNNER_TEMP/fairway-keychains.txt"
exit "$status"
