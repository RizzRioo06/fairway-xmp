#!/usr/bin/env python3
"""Explicit public-registry qualification in a new disposable directory.

Never updates this repository or a personal project. By default use the bundled
reviewed lockfiles. --generate-locks asks npm to create candidate locks in the
fixture only; copying them into a catalog revision requires a separate review.
Build/run qualification uses test_node_installations.py and the owned runner.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import tempfile


def run(command, cwd, env, log):
    child = subprocess.Popen(command, cwd=cwd, env=env, stdout=log,
                             stderr=subprocess.STDOUT, start_new_session=True)
    try:
        code = child.wait(timeout=180)
    except subprocess.TimeoutExpired:
        # wait(timeout) has not reaped the group leader. Its PID cannot be
        # recycled before this signal; these are disposable qualification jobs.
        os.killpg(child.pid, signal.SIGKILL)
        child.wait()
        raise
    if code:
        raise subprocess.CalledProcessError(code, command)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--node', type=Path, required=True)
    parser.add_argument('--generate-locks', action='store_true')
    args = parser.parse_args()
    if os.name != 'posix':
        parser.error('Qualification helper currently requires macOS/POSIX')
    repo = Path(__file__).resolve().parents[1]
    node = args.node.resolve(strict=True)
    npm = (node.parent / 'npm').resolve(strict=True)
    root = Path(tempfile.mkdtemp(prefix='fairway-template-qualification-'))
    print(root, flush=True)
    records = []
    for template in json.loads((repo / 'src-tauri/src/node_project_creation/catalog.json').read_text())['templates']:
        folder = root / template['id']
        folder.mkdir(mode=0o700)
        home = folder / 'tool-home'
        home.mkdir(mode=0o700)
        env = {'PATH': str(node.parent) + ':/usr/bin:/bin:/usr/sbin:/sbin',
               'HOME': str(home), 'TMPDIR': str(home), 'npm_config_cache': str(home / 'cache'),
               'npm_config_userconfig': '/dev/null', 'npm_config_globalconfig': '/dev/null/fairway-global.npmrc',
               'npm_config_update_notifier': 'false', 'npm_config_node_options': ''}
        version = subprocess.check_output([node, '-p', 'process.version'], env=env, text=True, timeout=10).strip()
        npm_version = subprocess.check_output([node, npm, '--version'], env=env, text=True, timeout=10).strip()
        if not (version.startswith('v22.') or version.startswith('v24.')):
            raise ValueError('Provide a supported Node 22/24 runtime')
        package = {'name': template['id'], 'version': '0.1.0', 'private': True, 'type': 'module', 'license': 'MIT',
                   'engines': {'node': template['nodeRange'], 'npm': template['packageManager']['versionRange']},
                   'scripts': template['scripts'],
                   'dependencies': {d['name']: d['version'] for d in template['dependencies'] if not d['development']},
                   'devDependencies': {d['name']: d['version'] for d in template['dependencies'] if d['development']}}
        (folder / 'package.json').write_text(json.dumps(package, indent=2) + '\n')
        if not args.generate_locks:
            (folder / 'package-lock.json').write_bytes((repo / 'src-tauri/templates/node-projects/locks' / (template['id'] + '.json')).read_bytes())
        flags = ['--ignore-scripts', '--no-audit', '--no-fund', '--include=dev', '--global=false', '--workspaces=false',
                 '--registry=https://registry.npmjs.org/', '--strict-ssl=true', '--engine-strict=true',
                 '--fetch-timeout=60000', '--fetch-retries=1', '--maxsockets=4']
        with (folder / 'qualification.log').open('w') as log:
            if args.generate_locks:
                run([node, npm, 'install', '--package-lock-only', *flags], folder, env, log)
            run([node, npm, 'ci', *flags], folder, env, log)
        records.append({'templateId': template['id'], 'node': version, 'npm': npm_version,
                        'sha256': hashlib.sha256((folder / 'package-lock.json').read_bytes()).hexdigest(),
                        'registry': 'https://registry.npmjs.org/', 'scriptsDisabled': True})
        (root / 'qualification.json').write_text(json.dumps(records, indent=2) + '\n')
        print(template['id'], 'npm ci qualified', flush=True)


if __name__ == '__main__':
    main()
