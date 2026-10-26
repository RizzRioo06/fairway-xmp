#!/usr/bin/env python3
"""Check the bundled native fnm and optional Node download in an isolated store."""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile


def main(runtime, install_node=False):
    with tempfile.TemporaryDirectory(prefix='fw-fnm-') as temporary:
        root = Path(temporary)
        home = root / "h's မြ"
        home.mkdir()
        shutil.copytree(runtime / 'fnm', root / 'fnm')
        fnm = root / 'fnm/bin/fnm'
        manifest = json.loads((runtime / 'manifest.json').read_text())
        env = {**os.environ, 'HOME': str(home), 'XDG_DATA_HOME': str(home / '.local/share'),
               'FNM_DIR': str(home / '.local/share/fnm'), 'ZDOTDIR': str(home)}
        version = subprocess.check_output([str(fnm), '--version'], env=env, text=True)
        assert f'fnm {manifest["fnm"]}' in version, version
        if install_node:
            subprocess.run([str(fnm), 'install', '--lts'], env=env, check=True, timeout=180)
            env.update(json.loads(subprocess.check_output([str(fnm), 'env', '--json'], env=env, text=True)))
            subprocess.run([str(fnm), 'use', 'lts-latest'], env=env, check=True)
            for tool in ('node', 'npm', 'npx'):
                assert subprocess.check_output([tool, '--version'], env=env, text=True).strip()
        print('Native fnm passed.' + (' Node download and Node/npm/npx passed.' if install_node else ''))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--runtime', type=Path, required=True)
    parser.add_argument('--install-node', action='store_true')
    args = parser.parse_args()
    main(args.runtime.resolve(), args.install_node)
