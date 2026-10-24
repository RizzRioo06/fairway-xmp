#!/usr/bin/env python3
"""Check PHP clients against an isolated external PostgreSQL server, never bundled."""
import argparse
import csv
import os
from pathlib import Path
import re
import secrets
import shutil
import socket
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]


def grant_windows_fixture_access(root):
    # initdb drops Administrators/Power Users from its Windows token. Python's
    # private temporary directory can otherwise be accessible only via those
    # groups. Give the actual user access, inherited by the password and cluster.
    identity = subprocess.check_output(['whoami', '/user', '/fo', 'csv', '/nh'],
                                       text=True, encoding='utf-8', errors='replace', timeout=15)
    records = list(csv.reader(identity.splitlines()))
    if len(records) != 1 or len(records[0]) != 2 or not re.fullmatch(r'S-\d+(?:-\d+)+', records[0][1]):
        raise RuntimeError('Cannot determine the Windows user SID for the PostgreSQL test directory.')
    sid = records[0][1]
    subprocess.run(['icacls', str(root), '/inheritance:r', '/grant:r', f'*{sid}:(OI)(CI)F'],
                   check=True, stdout=subprocess.DEVNULL, timeout=30)


def postgres_bin():
    candidates = [Path(os.environ['PGBIN'])] if os.environ.get('PGBIN') else []
    if shutil.which('pg_config'):
        candidates.append(Path(subprocess.check_output(['pg_config', '--bindir'], text=True).strip()))
    candidates.extend([Path('/opt/homebrew/opt/postgresql@17/bin'), Path('/usr/local/opt/postgresql@17/bin')])
    candidates.extend(sorted(Path('/usr/lib/postgresql').glob('*/bin'), reverse=True))
    suffix = '.exe' if os.name == 'nt' else ''
    for candidate in candidates:
        if all((candidate / (tool + suffix)).is_file() for tool in ('initdb', 'pg_ctl', 'postgres')):
            return candidate
    raise RuntimeError('Install a PostgreSQL server for testing only, or set PGBIN to its bin directory.')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--runtime', type=Path, default=ROOT / 'src-tauri/resources/runtime')
    parser.add_argument('--postgres-bin', type=Path)
    args = parser.parse_args()
    binaries = args.postgres_bin or postgres_bin()
    runtime = args.runtime.resolve()
    suffix = '.exe' if os.name == 'nt' else ''
    with tempfile.TemporaryDirectory(prefix='fairway-postgres-') as temporary:
        root = Path(temporary)
        if os.name == 'nt':
            grant_windows_fixture_access(root)
        password = secrets.token_hex(24)
        password_file = root / 'password'
        password_file.write_text(password, encoding='utf-8')
        password_file.chmod(0o600)
        data = root / 'cluster'
        environment = {**os.environ, 'LC_ALL': 'C'}
        subprocess.run([str(binaries / ('initdb' + suffix)), '-D', str(data), '-U', 'fairway_client_test',
                        '--pwfile', str(password_file), '--auth=scram-sha-256', '--encoding=UTF8', '--locale=C'],
                       env=environment, check=True, stdout=subprocess.DEVNULL, timeout=60)
        password_file.unlink()
        with socket.socket() as listener:
            listener.bind(('127.0.0.1', 0))
            port = listener.getsockname()[1]
        with (data / 'postgresql.conf').open('a', encoding='utf-8') as config:
            config.write(f"\nlisten_addresses = '127.0.0.1'\nport = {port}\nunix_socket_directories = ''\n")
        control = [str(binaries / ('pg_ctl' + suffix)), '-D', str(data), '-w', '-t', '30']
        # Include startup in the cleanup boundary in case pg_ctl times out after spawning.
        try:
            subprocess.run([*control, '-l', str(root / 'server.log'), 'start'], env=environment, check=True, timeout=40)
            subprocess.run(['cargo', 'run', '--manifest-path', str(ROOT / 'src-tauri/Cargo.toml'),
                            '--example', 'postgres_client_smoke', '--no-default-features', '--locked', '--',
                            str(root / 'app'), str(runtime)], cwd=ROOT,
                           env={**environment, 'FAIRWAY_TEST_POSTGRES_PORT': str(port),
                                'FAIRWAY_TEST_POSTGRES_PASSWORD': password}, check=True)
        finally:
            if (data / 'postmaster.pid').exists():
                subprocess.run([*control, 'stop', '-m', 'fast'], env=environment, check=True, timeout=40)


if __name__ == '__main__':
    main()
