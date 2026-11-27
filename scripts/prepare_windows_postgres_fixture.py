#!/usr/bin/env python3
"""Stage pinned test-only EDB server tools; never install or register a service."""
import argparse
import hashlib
import os
from pathlib import Path
import re
import shutil
import stat
import tempfile
import urllib.request
import zipfile

# Official PostgreSQL-linked EDB binary distribution and published .sha256.
URL = 'https://get.enterprisedb.com/postgresql/postgresql-17.12-2-windows-x64-binaries.zip'
SHA256 = '520c8c6f19abb3b31fa83ec5e486e2f769eedd7593d94e60a7cda8a5bfb4df91'
LIMIT = 2 * 1024**3


def extract_verified(archive, destination, expected=SHA256):
    digest = hashlib.sha256()
    with archive.open('rb') as source:
        for chunk in iter(lambda: source.read(1024**2), b''):
            digest.update(chunk)
    if digest.hexdigest() != expected:
        raise ValueError('PostgreSQL fixture archive checksum mismatch')
    # The destination must be new and all ancestors must be real directories.
    for ancestor in (destination.parent, *destination.parent.parents):
        info = ancestor.lstat()
        if not stat.S_ISDIR(info.st_mode) or getattr(info, 'st_file_attributes', 0) & 0x400:
            raise ValueError('Unsafe PostgreSQL fixture destination')
    with zipfile.ZipFile(archive) as bundle:
        entries = bundle.infolist()
        if len(entries) > 100000 or sum(entry.file_size for entry in entries) > LIMIT:
            raise ValueError('PostgreSQL fixture archive exceeds limits')
        names = set()
        selected = []
        for entry in entries:
            parts = entry.filename.rstrip('/').split('/')
            mode = entry.external_attr >> 16
            if (not parts or any(not part or part in ('.', '..') or
                    re.search(r'[\\:<>"|?*\x00-\x1f]', part) or part.endswith((' ', '.')) or
                    re.match(r'^(CON|PRN|AUX|NUL|COM[1-9]|LPT[1-9])(?:\.|$)', part, re.I)
                    for part in parts) or
                    (stat.S_IFMT(mode) not in (0, stat.S_IFREG, stat.S_IFDIR))):
                raise ValueError('Unsafe PostgreSQL fixture archive entry')
            name = '/'.join(parts).casefold()
            if name in names:
                raise ValueError('Duplicate PostgreSQL fixture archive entry')
            names.add(name)
            if len(parts) >= 2 and parts[0] == 'pgsql' and parts[1] in ('bin', 'lib', 'share'):
                selected.append((entry, parts))
        if not all(f'pgsql/bin/{tool}.exe' in names for tool in ('postgres', 'initdb', 'pg_ctl')):
            raise ValueError('PostgreSQL fixture archive lacks required tools')
        # Exclusive creation refuses existing directories, aliases and files.
        destination.mkdir()
        try:
            for entry, parts in selected:
                target = destination.joinpath(*parts)
                if entry.is_dir():
                    target.mkdir(parents=True, exist_ok=True)
                else:
                    target.parent.mkdir(parents=True, exist_ok=True)
                    with bundle.open(entry) as source, target.open('xb') as output:
                        shutil.copyfileobj(source, output)
        except BaseException:
            shutil.rmtree(destination)
            raise


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--destination', required=True, type=Path)
    args = parser.parse_args()
    if os.name != 'nt':
        parser.error('This test toolset requires Windows')
    destination = Path(os.path.abspath(args.destination))
    with tempfile.TemporaryDirectory(prefix='fairway-pg-test-download-') as folder:
        archive = Path(folder) / 'postgres.zip'
        with urllib.request.urlopen(URL, timeout=120) as source, archive.open('xb') as output:
            size = 0
            while chunk := source.read(1024**2):
                size += len(chunk)
                if size > LIMIT:
                    raise ValueError('PostgreSQL fixture download exceeds limit')
                output.write(chunk)
        extract_verified(archive, destination)
    print('Prepared checksum-verified PostgreSQL 17 test tools (no service installed).')


if __name__ == '__main__':
    main()
