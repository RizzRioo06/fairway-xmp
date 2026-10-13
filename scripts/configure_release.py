#!/usr/bin/env python3
"""Generate a release-only Tauri overlay; development builds have no update feed."""
import argparse
import json
from pathlib import Path
from urllib.parse import urlparse

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--endpoint', default='https://github.com/eimg/fairway-xmp/releases/latest/download/latest.json', help='HTTPS URL to signed update metadata (defaults to Fairway GitHub releases)')
parser.add_argument('--public-key', required=True, help='Tauri updater public key file')
parser.add_argument('--output', default='artifacts/tauri.release.json')
args = parser.parse_args()
if urlparse(args.endpoint).scheme != 'https': parser.error('The update endpoint must use HTTPS')
key = Path(args.public_key).read_text().strip()
if not key: parser.error('The public key file is empty')
output = Path(args.output)
output.parent.mkdir(parents=True, exist_ok=True)
output.write_text(json.dumps({
    'bundle': {'createUpdaterArtifacts': True},
    'plugins': {'updater': {'pubkey': key, 'endpoints': [args.endpoint], 'windows': {'installMode': 'passive'}}},
}, indent=2) + '\n')
print(output)
