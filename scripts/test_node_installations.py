#!/usr/bin/env python3
"""Offline native npm regressions using genuine tarballs from qualification caches.

This server never fetches from the network. Supply a disposable cache produced
by explicit public-registry qualification, two installed Node executables, and
an already-built node_projects_runner example (with node-install-fixtures).
No personal npm configuration/cache or installed Fairway bundle is changed.
"""
import argparse
import base64
import hashlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import subprocess
import socket
import threading
from urllib.parse import unquote, urlsplit


def tarballs(repo, qualification):
    result = {}
    for lock in (repo / 'src-tauri/templates/node-projects/locks').glob('*.json'):
        if lock.name == 'provenance.json':
            continue
        for package in json.loads(lock.read_text())['packages'].values():
            if 'resolved' not in package:
                continue
            integrity = package['integrity'].removeprefix('sha512-')
            expected = base64.b64decode(integrity, validate=True)
            key = expected.hex()
            for cache in qualification.glob('*/tool-home/cache/_cacache/content-v2/sha512'):
                path = cache / key[:2] / key[2:4] / key[4:]
                if path.is_file():
                    data = path.read_bytes()
                    if hashlib.sha512(data).digest() != expected:
                        raise ValueError(f'Corrupted fixture cache: {path}')
                    result[unquote(urlsplit(package['resolved']).path)] = data
                    break  # Wrong-platform optional tarballs may be absent.
    if not result:
        raise ValueError('No genuine tarballs found; qualify explicitly first')
    return result


def server(packages, mode):
    class Handler(BaseHTTPRequestHandler):
        def log_message(self, *_args):
            pass

        def do_GET(self):
            path = unquote(urlsplit(self.path).path)
            if mode == 'error':
                self.send_error(503, 'Controlled unavailable registry')
                return
            data = packages.get(path)
            if data is None:
                self.send_error(404, 'Tarball absent from offline fixture cache')
                return
            if mode == 'integrity':
                data = b'Controlled integrity mismatch; never extracted'
            self.send_response(200)
            self.send_header('Content-Length', str(len(data)))
            self.send_header('Content-Type', 'application/octet-stream')
            self.end_headers()
            try:
                self.wfile.write(data)
            except (BrokenPipeError, ConnectionResetError):
                pass
    http = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
    http.daemon_threads = True
    threading.Thread(target=http.serve_forever, daemon=True).start()
    return http


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--qualification-cache', type=Path, required=True)
    parser.add_argument('--runner', type=Path, required=True)
    parser.add_argument('--runtime', type=Path, help='Optional read-only packaged PHP runtime for coexistence test')
    parser.add_argument('--node', type=Path, action='append', required=True)
    parser.add_argument('--test-filter', default='node_project_creation::installation')
    args = parser.parse_args()
    if len(args.node) != 2:
        parser.error('Supply Node 22 and Node 24 with two --node arguments')
    capabilities = json.loads(subprocess.check_output([args.runner.resolve(), "--fixture-capabilities"], text=True, timeout=10))
    if not capabilities.get("nodeInstallationMirrors"):
        parser.error("Rebuild node_projects_runner with --features node-install-fixtures; copy it outside Cargo output before builds with other features")
    repo = Path(__file__).resolve().parents[1]
    packages = tarballs(repo, args.qualification_cache)
    servers = {mode: server(packages, mode) for mode in ('success', 'integrity', 'error')}
    env = dict(os.environ)
    env['FAIRWAY_TEST_NODE_RUNNER'] = str(args.runner.resolve())
    env['FAIRWAY_TEST_NODE_RUNTIMES'] = os.pathsep.join(str(node.resolve()) for node in args.node)
    for mode, http in servers.items():
        env[f'FAIRWAY_TEST_NPM_{mode.upper()}'] = f'http://127.0.0.1:{http.server_port}/'
    if args.runtime:
        env['FAIRWAY_TEST_RUNTIME'] = str(args.runtime.resolve())
    excluded = [] if args.runtime else ['--skip', 'installer_php_start_stop']
    unavailable = socket.socket()
    unavailable.bind(('127.0.0.1', 0))  # Reserve TCP port without listening.
    env['FAIRWAY_TEST_NPM_UNAVAILABLE'] = f'http://127.0.0.1:{unavailable.getsockname()[1]}/'
    try:
        subprocess.run(['cargo', 'test', '--manifest-path', 'src-tauri/Cargo.toml', '--lib',
                        '--features', 'node-install-fixtures', args.test_filter,
                        '--', '--include-ignored', '--nocapture', *excluded], cwd=repo, env=env, check=True, timeout=600)
    finally:
        unavailable.close()
        for http in servers.values():
            http.shutdown()
            http.server_close()


if __name__ == '__main__':
    main()
