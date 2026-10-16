#!/bin/bash
set -euo pipefail
if [ -f "$RUNNER_TEMP/fairway-keychains.txt" ]; then
  python3 - "$RUNNER_TEMP/fairway-keychains.txt" <<'PY'
import shlex, subprocess, sys
subprocess.run(['security', 'list-keychains', '-d', 'user', '-s', *shlex.split(open(sys.argv[1]).read())], check=True)
PY
fi
security delete-keychain "$RUNNER_TEMP/fairway-signing.keychain-db" || true
rm -f "$RUNNER_TEMP/fairway-identities.p12" "$RUNNER_TEMP/fairway-notary.p8" "$RUNNER_TEMP/fairway-keychains.txt"
