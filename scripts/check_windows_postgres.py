#!/usr/bin/env python3
"""Headless Windows query-worker check using only a new disposable cluster."""
import argparse
import json
import os
import re
from pathlib import Path
import secrets
import socket
import subprocess
import tempfile
from check_postgres_clients import postgres_bin, grant_windows_fixture_access

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--authorize-isolated-server', action='store_true', required=True)
    parser.add_argument('--runtime', type=Path, default=ROOT / 'src-tauri/resources/runtime')
    parser.add_argument('--postgres-bin', type=Path)
    args = parser.parse_args()
    if os.name != 'nt': parser.error('This fixture requires Windows x64')
    candidates = [args.postgres_bin] if args.postgres_bin else []
    if not candidates:
        try: candidates.append(postgres_bin())
        except RuntimeError: pass
        candidates.extend(Path(os.environ.get('ProgramFiles', 'C:/Program Files')).joinpath('PostgreSQL', version, 'bin') for version in ('17', '18'))
    binaries = None
    for candidate in candidates:
        if candidate.is_symlink() or not all((candidate/(name+'.exe')).is_file() and not (candidate/(name+'.exe')).is_symlink() for name in ('postgres','initdb','pg_ctl')): continue
        version = subprocess.check_output([str(candidate/'postgres.exe'), '--version'], text=True, timeout=15)
        match = re.search(r'PostgreSQL\)\s+(\d+)\.(\d+)', version)
        if match and (match[1] == '17' or (match[1], match[2]) == ('18','6')):
            binaries = candidate.resolve(); break
    if binaries is None: raise RuntimeError('Native Windows fixture requires test-only PostgreSQL 17.x or 18.6; set PGBIN to its bin directory')
    subprocess.run(['cargo', 'build', '--manifest-path', str(ROOT/'src-tauri/Cargo.toml'), '--no-default-features', '--features', 'terminal', '--locked', '--example', 'postgres_windows_smoke'], cwd=ROOT, check=True)
    executable = ROOT/'src-tauri/target/debug/examples/postgres_windows_smoke.exe'
    with tempfile.TemporaryDirectory(prefix='fairway-windows-query-') as directory:
        root = Path(directory); grant_windows_fixture_access(root)
        password = secrets.token_hex(24)
        env = {k:v for k,v in os.environ.items() if not k.upper().startswith('PG')}
        env['LC_ALL'] = 'C'
        cluster = root/'cluster'
        # Without a console, native initdb prompts read the two password lines from
        # inherited stdin. No password file, argv value or environment secret.
        subprocess.run([str(binaries/'initdb.exe'), '-D', str(cluster), '-U', 'fairway_windows_test', '--pwprompt', '--auth=scram-sha-256', '--encoding=UTF8', '--locale=C'], input=((password+'\n')*2).encode(), env=env, creationflags=subprocess.CREATE_NO_WINDOW, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, check=True, timeout=60)
        with socket.socket() as listener:
            listener.bind(('127.0.0.1', 0)); port = listener.getsockname()[1]
        with (cluster/'postgresql.conf').open('a', encoding='utf-8') as config:
            config.write(f"\nlisten_addresses='127.0.0.1'\nport={port}\nunix_socket_directories=''\nlog_statement='none'\nlog_min_error_statement='panic'\n")
        control = [str(binaries/'pg_ctl.exe'), '-D', str(cluster), '-w', '-t', '30']
        try:
            subprocess.run([*control, '-l', str(root/'server.log'), 'start'], env=env, check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=40)
            request = {'root': str(root/'app'), 'runtime': str(args.runtime.resolve()), 'connection': {'confirmed': True, 'database':'postgres', 'target': {'kind':'external', 'host':'127.0.0.1', 'port':port, 'user':'fairway_windows_test', 'password':password, 'transport':'loopback'}}}
            subprocess.run([str(executable)], input=json.dumps(request).encode(), cwd=ROOT, env=env, check=True, timeout=120)
        finally:
            if (cluster/'postmaster.pid').exists():
                subprocess.run([*control, 'stop', '-m', 'fast'], env=env, check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=40)

if __name__ == '__main__': main()
