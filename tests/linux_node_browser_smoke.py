#!/usr/bin/env python3
"""Headless native-fnm and production HTTP/SQLite check. Never opens a browser."""
import argparse
from http.cookiejar import CookieJar
import json
import os
from pathlib import Path
import re
import shutil
import signal
import socket
import sqlite3
import subprocess
import tempfile
import time
import urllib.error
import urllib.request


def main(cli, runtime, install_node=False, with_mysql=False):
    # Keep supervisor socket paths below sockaddr_un's limit, including on macOS.
    with tempfile.TemporaryDirectory(prefix='fwb-', dir='/tmp') as temporary:
        root = Path(temporary)
        home = root / "h's မြ"
        home.mkdir()
        bundle = root / 'installation'
        (bundle / 'bin').mkdir(parents=True)
        binary = bundle / 'bin/fairway'
        shutil.copy2(cli, binary)
        shutil.copytree(runtime / 'data-browser', bundle / 'runtime/data-browser')
        shutil.copytree(runtime / 'fnm', bundle / 'runtime/fnm')
        if with_mysql:
            # Only test-owned data is written; bundled runtime files are read-only inputs.
            for component in runtime.iterdir():
                destination = bundle / 'runtime' / component.name
                if not destination.exists(): destination.symlink_to(component)
        fnm = bundle / 'runtime/fnm/bin/fnm'
        env = {**os.environ, 'HOME': str(home), 'XDG_DATA_HOME': str(home / '.local/share'),
               'FAIRWAY_DEV_HOME': str(home), 'FAIRWAY_DEV_DATA': str(home / '.local/share/dev.fairway.environment'),
               'FNM_DIR': str(home / '.local/share/fnm'), 'ZDOTDIR': str(home),
               'npm_config_cache': str(root / 'npm-cache'), 'npm_config_update_notifier': 'false'}
        def call(*args, environment=None, check=True):
            result = subprocess.run([str(binary), *args], env=environment or env, text=True,
                                    capture_output=True, timeout=180)
            if check and result.returncode: raise AssertionError(result.stdout + result.stderr)
            return result
        assert 'fnm 1.39.0' in subprocess.check_output([str(fnm), '--version'], env=env, text=True)
        if install_node:
            subprocess.run([str(fnm), 'install', '--lts'], env=env, check=True, timeout=180)
            env.update(json.loads(subprocess.check_output([str(fnm), 'env', '--json'], env=env, text=True)))
            subprocess.run([str(fnm), 'use', 'lts-latest'], env=env, check=True)
            assert subprocess.check_output(['npm', '--version'], env=env, text=True).strip()
        no_node = {**env, 'PATH': str(root / 'empty-path')}
        missing = call('data-browser', environment=no_node, check=False)
        assert missing.returncode and 'Node was not detected' in missing.stderr, missing
        assert 'fnm' in missing.stderr and 'install --lts' in missing.stderr
        assert '--prepare' in call('data-browser', '--help', environment=no_node).stdout
        assert call('data-browser', '--host', '0.0.0.0', check=False).returncode
        assert call('data-browser', '--port', '80', check=False).returncode
        old_bin = root / 'old-node'
        old_bin.mkdir()
        old = old_bin / 'node'
        old.write_text('#!/bin/sh\nprintf \'%s\\n\' \'{"version":"20.20.0","modules":"115","platform":"linux","arch":"x64","executable":"/old/node"}\'\n')
        old.chmod(0o755)
        outdated = call('data-browser', environment={**env, 'PATH': str(old_bin)}, check=False)
        assert outdated.returncode and 'Node 22 or newer' in outdated.stderr, outdated
        data = home / '.local/share/dev.fairway.environment'
        assert not (data / 'supervisor.sock').exists(), 'Browser command started PHP supervisor'
        # Simultaneous first use must share the same setup rather than corrupt node_modules.
        clients = [subprocess.Popen([str(binary), 'data-browser', '--prepare'], env=env,
                                   stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True) for _ in range(2)]
        for client in clients:
            out, err = client.communicate(timeout=180)
            assert client.returncode == 0, out + err
        ready, = data.glob('data-browser/*/*/.ready')
        ready_time = ready.stat().st_mtime_ns
        # An empty npm cache with offline-only mode proves that warm starts need no install.
        offline = {**env, 'npm_config_offline': 'true', 'npm_config_cache': str(root / 'empty-npm-cache')}
        assert 'dependencies are ready' in call('data-browser', '--prepare', environment=offline).stdout
        assert ready.stat().st_mtime_ns == ready_time
        database = root / "project's data မြန်မာ.sqlite"
        with sqlite3.connect(database) as db:
            db.executescript("CREATE TABLE notes (id INTEGER PRIMARY KEY, title TEXT); INSERT INTO notes VALUES (1, 'မြန်မာ');")
        with socket.socket() as sock:
            sock.bind(('127.0.0.1', 0))
            port = sock.getsockname()[1]
        url = f'http://127.0.0.1:{port}'
        opener = urllib.request.build_opener(urllib.request.HTTPCookieProcessor(CookieJar()))
        def request(path, body=None, method=None, headers=None, expected=200):
            headers = headers or {}
            if body is not None:
                headers = {'content-type': 'application/json', **headers}
            req = urllib.request.Request(url + path, data=json.dumps(body).encode() if body is not None else None,
                                         method=method, headers=headers)
            try: response = opener.open(req, timeout=10)
            except urllib.error.HTTPError as error: response = error
            with response:
                raw = response.read()
                assert response.status == expected, (response.status, raw)
                return raw
        with (root / 'server.log').open('w+') as log:
            server = subprocess.Popen([str(binary), 'data-browser', '--port', str(port)], env=offline,
                                      stdout=log, stderr=log, start_new_session=True)
            try:
                deadline = time.monotonic() + 20
                while time.monotonic() < deadline:
                    if server.poll() is not None:
                        log.seek(0)
                        raise AssertionError(log.read())
                    printed = (root / 'server.log').read_text()
                    access = re.search(rf'http://127\.0\.0\.1:{port}/\?token=[A-Za-z0-9_-]+', printed)
                    if access:
                        with opener.open(access.group(), timeout=10) as response:
                            assert response.status == 200
                        assert json.loads(request('/api/health'))['ok']
                        break
                    time.sleep(.1)
                else: raise AssertionError('Data browser did not start')
                html = request('/').decode()
                assets = re.findall(r'(?:src|href)="(/assets/[^\"]+)"', html)
                assert assets and all(request(asset) for asset in assets)
                try:
                    urllib.request.urlopen(url + '/api/session', timeout=10)
                    raise AssertionError('Unauthenticated local requests reached the database API')
                except urllib.error.HTTPError as error: assert error.code == 401
                request('/', headers={'Cookie': f'fairway_browser_access_{port}=wrong'}, expected=401)
                for operation in ('uninstall', '__prepare-update'):
                    blocked = call(operation, check=False)
                    assert blocked.returncode and 'Ctrl+C' in blocked.stderr, blocked
                request('/api/health', headers={'Host': 'attacker.invalid'}, expected=403)
                request('/api/session', body={}, headers={'Origin': 'https://attacker.invalid'}, expected=403)
                request('/api/session', body={}, headers={'content-type': 'text/plain'}, expected=403)
                request('/%2e%2e/package.json', expected=404)
                info = json.loads(request('/api/session', {'engine': 'sqlite', 'path': str(database)}, expected=201))
                assert info['engine'] == 'sqlite'
                result = json.loads(request('/api/data', {'id': 'sqlite-query', 'request': {
                    'driver': 'sqlite', 'path': str(database), 'op': 'query', 'sql': 'SELECT title FROM notes'}}))
                assert 'မြန်မာ' in json.dumps(result, ensure_ascii=False), result
                request('/api/session?engine=sqlite', method='DELETE')
                assert json.loads(request('/api/session'))['connections'] == []
                # Binding collisions must fail clearly, leaving the first server usable.
                collision = call('data-browser', '--port', str(port), environment=offline, check=False)
                assert collision.returncode and 'EADDRINUSE' in collision.stderr, collision
                assert json.loads(request('/api/health'))['ok']
                assert not (data / 'supervisor.sock').exists()
                if with_mysql:
                    sockets = [socket.socket() for _ in range(6)]
                    try:
                        for sock in sockets: sock.bind(('127.0.0.1', 0))
                        ports = [sock.getsockname()[1] for sock in sockets]
                    finally:
                        for sock in sockets: sock.close()
                    call('ports', *map(str, ports))
                    call('start', 'mysql')
                    info = json.loads(request('/api/session', {'engine': 'mysql', 'host': '127.0.0.1',
                        'port': ports[2], 'user': 'fairway', 'password': 'fairway',
                        'database': 'fairway', 'ssl': False}, expected=201))
                    assert info['engine'] == 'mysql'
                    result = json.loads(request('/api/data', {'id': 'mysql-query', 'request': {
                        'driver': 'mysql', 'database': 'fairway', 'op': 'query', 'sql': 'SELECT 42 AS answer'}}))
                    assert '42' in json.dumps(result), result
                    request('/api/session?engine=mysql', method='DELETE')
            finally:
                if server.poll() is None: os.killpg(server.pid, signal.SIGTERM)
                server.wait(timeout=10)
                if with_mysql: call('shutdown', check=False)
        assert database.is_file()
        print('Native fnm, missing/old Node guidance, concurrent setup, offline reuse, static assets, loopback guards and real SQLite API passed.' + (' Real bundled MySQL API passed.' if with_mysql else ''))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--cli', type=Path, required=True)
    parser.add_argument('--runtime', type=Path, required=True)
    parser.add_argument('--install-node', action='store_true')
    parser.add_argument('--with-mysql', action='store_true')
    args = parser.parse_args()
    main(args.cli.resolve(), args.runtime.resolve(), args.install_node, args.with_mysql)
