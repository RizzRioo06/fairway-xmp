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
import shutil
import socket
import subprocess
import tempfile
import time
from urllib.request import urlopen


def execute(template, folder, node, npm, env, log):
    """Execute only explicitly authorized, reviewed disposable template sources."""
    for script in ('build', 'test'):
        if script in template['scripts']:
            run([node, npm, 'run', script], folder, env, log)
    with socket.socket() as reserved:
        reserved.bind(('127.0.0.1', 0))
        port = reserved.getsockname()[1]
    command = [node, npm, 'run', 'dev']
    if template['id'].startswith('react-vite'):
        command += ['--', '--port', str(port), '--strictPort']
    else:
        (folder / '.env').write_text(f'HOST=127.0.0.1\nPORT={port}\n')
    child = subprocess.Popen(command, cwd=folder, env=env, stdout=log,
                             stderr=subprocess.STDOUT, start_new_session=True)
    try:
        deadline = time.monotonic() + 20
        while True:
            if child.poll() is not None:
                raise RuntimeError(f'{template["id"]} dev server exited')
            try:
                with urlopen(f'http://127.0.0.1:{port}/', timeout=1) as response:
                    if response.status == 200:
                        break
            except OSError:
                pass
            if time.monotonic() >= deadline:
                raise TimeoutError(f'{template["id"]} HTTP response')
            time.sleep(0.05)
    finally:
        if child.poll() is None:
            os.killpg(child.pid, signal.SIGTERM)
            try:
                child.wait(timeout=10)
            except subprocess.TimeoutExpired:
                os.killpg(child.pid, signal.SIGKILL)
                child.wait()
    with socket.socket() as probe:
        if probe.connect_ex(('127.0.0.1', port)) == 0:
            raise RuntimeError('Disposable dev listener survived shutdown')


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
    parser.add_argument('--candidate-node26', action='store_true', help='Test Node 26 before catalog support; fixture engines only')
    parser.add_argument('--execute', action='store_true', help='Explicitly authorize build/test/dev scripts in disposable fixtures')
    parser.add_argument('--qualification-cache', type=Path, help='Use verified tarballs from an earlier qualification through a loopback-only mirror')
    args = parser.parse_args()
    if os.name != 'posix':
        parser.error('Qualification helper currently requires macOS/POSIX')
    repo = Path(__file__).resolve().parents[1]
    node = args.node.resolve(strict=True)
    npm = (node.parent / 'npm').resolve(strict=True)
    # tsx uses a Unix socket beneath TMPDIR; macOS limits its path to 104 bytes.
    root = Path(tempfile.mkdtemp(prefix='fw-qualify-', dir='/tmp'))
    print(root, flush=True)
    records = []
    mirror = None
    if args.qualification_cache:
        from test_node_installations import server, tarballs
        mirror = server(tarballs(repo, args.qualification_cache), 'success')
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
        if not (version.startswith(('v22.', 'v24.')) or (version.startswith('v26.') and (args.candidate_node26 or '>=26.0.0' in template['nodeRange']))):
            raise ValueError('Provide a catalog-supported Node runtime, or explicitly qualify a Node 26 candidate')
        package = {'name': template['id'], 'version': '0.1.0', 'private': True, 'type': 'module', 'license': 'MIT',
                   'engines': {'node': template['nodeRange'], 'npm': template['packageManager']['versionRange']},
                   'scripts': template['scripts'],
                   'dependencies': {d['name']: d['version'] for d in template['dependencies'] if not d['development']},
                   'devDependencies': {d['name']: d['version'] for d in template['dependencies'] if d['development']}}
        (folder / 'package.json').write_text(json.dumps(package, indent=2) + '\n')
        if args.candidate_node26:
            package['engines']['node'] = '>=26.0.0 <27'
            (folder / 'package.json').write_text(json.dumps(package, indent=2) + '\n')
        if not args.generate_locks:
            lock = json.loads((repo / 'src-tauri/templates/node-projects/locks' / (template['id'] + '.json')).read_text())
            lock['packages']['']['engines'] = package['engines']
            (folder / 'package-lock.json').write_text(json.dumps(lock, indent=2) + '\n')
        if args.execute:
            shutil.copytree(repo / 'src-tauri/templates/node-projects' / template['id'], folder, dirs_exist_ok=True)
        flags = ['--ignore-scripts', '--no-audit', '--no-fund', '--include=dev', '--global=false', '--workspaces=false',
                 '--registry=https://registry.npmjs.org/', '--strict-ssl=true', '--engine-strict=true',
                 '--fetch-timeout=60000', '--fetch-retries=1', '--maxsockets=4']
        if mirror:
            flags = [flag for flag in flags if not flag.startswith('--registry=')]
            flags += [f'--registry=http://127.0.0.1:{mirror.server_port}/']
        with (folder / 'qualification.log').open('w') as log:
            if args.generate_locks:
                run([node, npm, 'install', '--package-lock-only', *flags], folder, env, log)
            run([node, npm, 'ci', *flags], folder, env, log)
            if args.execute:
                execute(template, folder, node, npm, env, log)
        records.append({'templateId': template['id'], 'node': version, 'npm': npm_version,
                        'sha256': hashlib.sha256((folder / 'package-lock.json').read_bytes()).hexdigest(),
                        'registry': 'verifiedOfflineMirror' if mirror else 'https://registry.npmjs.org/',
                        'scriptsDisabled': True, 'executionPassed': args.execute, 'candidateNode26': args.candidate_node26})
        (root / 'qualification.json').write_text(json.dumps(records, indent=2) + '\n')
        print(template['id'], 'npm ci qualified', flush=True)
    if mirror:
        mirror.shutdown()
        mirror.server_close()


if __name__ == '__main__':
    main()
