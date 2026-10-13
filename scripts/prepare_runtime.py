#!/usr/bin/env python3
"""Stage a self-contained, version-pinned native runtime. Never installs system software."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import tarfile
import tempfile
import time
import urllib.parse
import urllib.request
import zipfile

ROOT = Path(__file__).resolve().parent.parent
CACHE = ROOT / '.runtime-cache'
DEST = ROOT / 'src-tauri/resources/runtime'
MANIFEST = json.loads((ROOT / 'runtime-manifest.json').read_text())
LOCK_PATH = ROOT / 'runtime-downloads.lock.json'
HEADERS = {'User-Agent': 'Fairway-runtime-builder/0.1'}
RESUME_LOCK = False


def request(url, headers=None):
    return urllib.request.urlopen(urllib.request.Request(url, headers={**HEADERS, **(headers or {})}), timeout=90)


def read_json(url):
    with request(url) as response:
        return json.load(response)


def download(url, expected=None, algorithm='sha256', headers=None):
    CACHE.mkdir(exist_ok=True)
    target = CACHE / hashlib.sha256(url.encode()).hexdigest()
    if not target.exists():
        print(f'Downloading {url}', flush=True)
        partial = target.with_suffix('.partial')
        try:
            for attempt in range(3):
                try:
                    with request(url, headers) as response, partial.open('wb') as output:
                        shutil.copyfileobj(response, output)
                    partial.replace(target)
                    break
                except (OSError, urllib.error.URLError):
                    if attempt == 2: raise
                    print(f'Retrying interrupted download ({attempt + 1}/2)', flush=True)
                    time.sleep(2)
        finally:
            partial.unlink(missing_ok=True)
    digest = hashlib.new(algorithm, target.read_bytes()).hexdigest()
    if expected and digest.lower() != expected.lower():
        target.unlink()
        raise RuntimeError(f'Checksum mismatch for {url}; refused to extract')
    return target, hashlib.sha256(target.read_bytes()).hexdigest()


def safe_extract(archive: Path, target: Path, kind: str, skip_external_links=False):
    """Reject traversal and archive links before unpacking third-party releases."""
    target.mkdir(parents=True, exist_ok=True)
    base = target.resolve()
    if kind == 'zip':
        with zipfile.ZipFile(archive) as bundle:
            for item in bundle.infolist():
                path = Path(item.filename.replace('\\', '/'))
                if path.is_absolute() or '..' in path.parts or ':' in item.filename:
                    raise ValueError('Unsafe archive path')
                if (item.external_attr >> 16) & 0o170000 == 0o120000:
                    raise ValueError('Archive symlinks are not allowed')
                if not (base / path).resolve().is_relative_to(base):
                    raise ValueError('Archive escapes destination')
            bundle.extractall(target)
    else:
        with tarfile.open(archive, 'r:*') as bundle:
            # Python's data filter handles harmless internal Homebrew symlinks,
            # rejects devices and links escaping the extraction root.
            def safe_filter(member, destination):
                try:
                    return tarfile.data_filter(member, destination)
                except (tarfile.LinkOutsideDestinationError, tarfile.AbsoluteLinkError):
                    # Some bottles contain optional links to Homebrew's global www
                    # folder. Never extract those links; Fairway has its own roots.
                    if skip_external_links and member.issym(): return None
                    raise
            bundle.extractall(target, filter=safe_filter)


def copy(source, destination):
    destination.parent.mkdir(parents=True, exist_ok=True)
    if source.is_dir():
        shutil.copytree(source, destination, dirs_exist_ok=True, symlinks=False)
    else:
        shutil.copy2(source, destination)


def lock_download(name, url, expected=None, algorithm='sha256', *, refresh=False, lock=None, headers=None):
    lock = lock if lock is not None else {}
    entry = lock.get(name)
    if entry and entry['url'] != url and not refresh:
        raise RuntimeError(f'{name} changed upstream; refresh and review the runtime download lock')
    checksum = None if refresh else (entry or {}).get('sha256')
    if not checksum and not refresh:
        raise RuntimeError(f'No reviewed checksum for {name}. Run --refresh-lock once, review it, then commit the lock.')
    path, sha = download(url, checksum or expected, 'sha256' if checksum else algorithm, headers)
    lock[name] = {'url': url, 'sha256': sha}
    return path


def common(target, lock, refresh):
    version = MANIFEST['composer']
    url = f'https://getcomposer.org/download/{version}/composer.phar'
    if lock.get('composer', {}).get('url') == url:
        sha = lock['composer']['sha256']
    else:
        with request(url + '.sha256sum') as response:
            sha = response.read().decode().split()[0]
    copy(lock_download('composer', url, sha, refresh=refresh, lock=lock), target / 'composer.phar')
    version = MANIFEST['phpmyadmin']
    url = f'https://files.phpmyadmin.net/phpMyAdmin/{version}/phpMyAdmin-{version}-all-languages.zip'
    if lock.get('phpmyadmin', {}).get('url') == url:
        sha = lock['phpmyadmin']['sha256']
    else:
        with request(url + '.sha256') as response:
            sha = response.read().decode().split()[0]
    archive = lock_download('phpmyadmin', url, sha, refresh=refresh, lock=lock)
    with tempfile.TemporaryDirectory() as folder:
        safe_extract(archive, Path(folder), 'zip')
        copy(Path(folder) / f'phpMyAdmin-{version}-all-languages', target / 'phpmyadmin')
    # Certifi packages Mozilla's trust roots and publishes immutable wheel URLs.
    # Pin the wheel rather than relying on a mutable certificate download URL.
    entry = lock.get('ca-certificates')
    if entry:
        url, sha = entry['url'], entry['sha256']
    else:
        package = read_json('https://pypi.org/pypi/certifi/json')
        wheel = next(file for file in package['urls'] if file['filename'].endswith('.whl'))
        url, sha = wheel['url'], wheel['digests']['sha256']
    archive = lock_download('ca-certificates', url, sha, refresh=refresh, lock=lock)
    with zipfile.ZipFile(archive) as wheel:
        (target / 'cacert.pem').write_bytes(wheel.read('certifi/cacert.pem'))
        for name in wheel.namelist():
            if 'LICENSE' in name.upper():
                path = target / 'licenses/ca-certificates' / Path(name).name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(wheel.read(name))


def windows(target, lock, refresh):
    php = MANIFEST['php']
    sources = {
        'php-windows': (f'https://downloads.php.net/~windows/releases/php-{php}-nts-Win32-vs17-x64.zip', 'php'),
        'mysql-windows': (f'https://cdn.mysql.com/Downloads/MySQL-8.4/mysql-{MANIFEST["mysql"]}-winx64.zip', 'mysql'),
        'nginx-windows': (f'https://nginx.org/download/nginx-{MANIFEST["nginx"]}.zip', 'nginx'),
    }
    for name, (url, service) in sources.items():
        archive = lock_download(name, url, refresh=refresh, lock=lock)
        with tempfile.TemporaryDirectory() as folder:
            unpacked = Path(folder)
            safe_extract(archive, unpacked, 'zip')
            if service == 'php':
                # DLLs must be alongside PHP executables; extensions remain separate.
                for file in unpacked.iterdir():
                    if file.suffix.lower() in ('.exe', '.dll'): copy(file, target / 'php/bin' / file.name)
                copy(unpacked / 'ext', target / 'php/ext')
                for file in unpacked.glob('*LICENSE*'): copy(file, target / 'licenses/php' / file.name)
                for file in unpacked.glob('*license*'): copy(file, target / 'licenses/php' / file.name)
            elif service == 'nginx':
                base = next(unpacked.iterdir())
                copy(base / 'nginx.exe', target / 'nginx/bin/nginx.exe')
                copy(base / 'conf/mime.types', target / 'nginx/conf/mime.types')
                for file in base.glob('*LICENSE*'): copy(file, target / 'licenses/nginx' / file.name)
            else:
                base = next(unpacked.iterdir())
                for directory in ('bin', 'lib', 'share'): copy(base / directory, target / 'mysql' / directory)
                for file in base.glob('*LICENSE*'): copy(file, target / 'licenses/mysql' / file.name)
    # The Microsoft redistributable is installed by our NSIS hook if required.
    url = 'https://aka.ms/vs/17/release/vc_redist.x64.exe'
    copy(lock_download('vc-runtime', url, refresh=refresh, lock=lock), target / 'vc_redist.x64.exe')


class Bottles:
    """Download build inputs without installing Homebrew on the builder or student machine."""
    def __init__(self, work, lock, refresh):
        self.work, self.lock, self.refresh = work, lock, refresh
        self.kegs = {}
        self.metadata = {}

    def fetch(self, name, required_version=None):
        if name in self.kegs: return self.kegs[name]
        key = f'brew-{name}-arm64-sequoia'
        stored = self.lock.get(key)
        if stored and (not self.refresh or RESUME_LOCK):
            version, url, checksum = stored['version'], stored['url'], stored['sha256']
        else:
            data = read_json(f'https://formulae.brew.sh/api/formula/{urllib.parse.quote(name, safe="")}.json')
            self.metadata[name] = data
            version = data['versions']['stable']
            bottle = data['bottle']['stable']['files'].get('arm64_sequoia')
            if not bottle:
                raise RuntimeError(f'No macOS 15 ARM bottle for {name}; a source build is required')
            url, checksum = bottle['url'], bottle['sha256']
        if required_version and version != required_version:
            raise RuntimeError(f'{name}: manifest pins {required_version}, but input is {version}; review versions before refreshing')
        headers = None
        cached = CACHE / hashlib.sha256(url.encode()).hexdigest()
        if not cached.exists():
            repo = urllib.parse.urlsplit(url).path.split('/blobs/')[0].removeprefix('/v2/')
            token = read_json(f'https://ghcr.io/token?service=ghcr.io&scope=repository:{repo}:pull')['token']
            headers = {'Authorization': f'Bearer {token}'}
        archive = lock_download(key, url, checksum, refresh=self.refresh, lock=self.lock,
                                headers=headers)
        self.lock[key]['version'] = version
        safe_extract(archive, self.work, 'tar', skip_external_links=True)
        folder = self.work / name
        if not folder.exists():
            # Bottle formula directory occasionally differs from the API alias.
            folder = self.work / name.split('@')[0]
        kegs = [path for path in folder.iterdir() if path.is_dir()]
        if len(kegs) != 1: raise RuntimeError(f'Cannot identify extracted keg for {name}')
        self.kegs[name] = kegs[0]
        return kegs[0]

    def dependency(self, original):
        for prefix in ('/opt/homebrew/opt/', '/opt/homebrew/Cellar/'):
            if original.startswith(prefix):
                rest = original.removeprefix(prefix).split('/')
                formula = rest[0]
                keg = self.fetch(formula)
                suffix = rest[1:] if prefix.endswith('/opt/') else rest[2:]
                candidate = keg.joinpath(*suffix)
                if not candidate.exists(): raise RuntimeError(f'Missing dependency {original}')
                return candidate.resolve()
        raise RuntimeError(f'Unresolved non-system dependency {original}')

    def relative_dependency(self, dependency, origin=None):
        suffix = dependency.split('/', 1)[-1]
        if origin and (origin.parent / suffix).exists():
            return (origin.parent / suffix).resolve()
        name = Path(dependency).name
        matches = {file.resolve() for keg in self.kegs.values() for file in keg.rglob(name) if file.is_file()}
        if len(matches) != 1:
            raise RuntimeError(f'Cannot resolve relative library {dependency}: {len(matches)} candidates')
        return matches.pop()


def macho(path):
    if not path.is_file(): return False
    with path.open('rb') as file: magic = file.read(4)
    return magic in (b'\xcf\xfa\xed\xfe', b'\xce\xfa\xed\xfe', b'\xca\xfe\xba\xbe', b'\xbe\xba\xfe\xca')


def relocate(target, bottles):
    libraries = target / 'lib'
    libraries.mkdir(exist_ok=True)
    queue = [path for path in target.rglob('*') if macho(path)]
    seen = set()
    sources = {}
    while queue:
        binary = queue.pop()
        if binary in seen: continue
        seen.add(binary)
        # Bottle signatures become invalid when load commands change.
        subprocess.run(['codesign', '--remove-signature', str(binary)], capture_output=True)
        output = subprocess.check_output(['otool', '-L', str(binary)], text=True)
        for line in output.splitlines()[1:]:
            original = line.strip().split(' (')[0]
            dependency = original.replace('@@HOMEBREW_PREFIX@@', '/opt/homebrew').replace('@@HOMEBREW_CELLAR@@', '/opt/homebrew/Cellar')
            if dependency.startswith(('/usr/lib/', '/System/Library/')): continue
            if dependency.startswith('@loader_path/') and (binary.parent / dependency.removeprefix('@loader_path/')).exists():
                continue
            if dependency.startswith(('@loader_path/', '@rpath/')):
                source = bottles.relative_dependency(dependency, sources.get(binary))
            elif dependency.startswith('/opt/homebrew/'):
                source = bottles.dependency(dependency)
            else:
                raise RuntimeError(f'Unexpected library dependency: {dependency}')
            destination = libraries / source.name
            if destination in sources and sources[destination] != source:
                raise RuntimeError(f'Library name collision: {source.name}')
            if destination not in sources:
                copy(source, destination); sources[destination] = source; queue.append(destination)
            relative = os.path.relpath(destination, binary.parent)
            subprocess.run(['install_name_tool', '-change', original, '@loader_path/' + relative, str(binary)], check=True)
        if binary.suffix in ('.dylib', '.so'):
            subprocess.run(['install_name_tool', '-id', '@loader_path/' + binary.name, str(binary)], check=True)
        subprocess.run(['codesign', '--force', '--sign', '-', str(binary)], check=True, capture_output=True)
    # All load commands must now resolve without the builder's Homebrew tree.
    for binary in seen:
        output = subprocess.check_output(['otool', '-L', str(binary)], text=True)
        if '/opt/homebrew/' in output or '@@HOMEBREW_' in output:
            raise RuntimeError(f'Non-relocatable binary: {binary}')


def macos(target, lock, refresh):
    if platform.machine() != 'arm64': raise RuntimeError('Only Apple Silicon builders are supported')
    with tempfile.TemporaryDirectory(prefix='fairway-bottles-') as folder:
        bottles = Bottles(Path(folder), lock, refresh)
        php = bottles.fetch('php@8.4', MANIFEST['php'])
        mysql = bottles.fetch('mysql@8.4', MANIFEST['mysql'])
        nginx = bottles.fetch('nginx', MANIFEST['nginx'])
        for executable in ('php', 'php-cgi'):
            copy(php / 'bin' / executable, target / 'php/bin' / executable)
        copy(php / 'sbin/php-fpm', target / 'php/bin/php-fpm')
        extensions = list(php.glob('lib/php/*'))
        (target / 'php/ext').mkdir(parents=True, exist_ok=True)
        for extension in php.rglob('*.so'):
            if 'httpd' not in str(extension): copy(extension, target / 'php/ext' / extension.name)
        for directory in ('bin', 'lib', 'share'): copy(mysql / directory, target / 'mysql' / directory)
        copy(nginx / 'bin/nginx', target / 'nginx/bin/nginx')
        mime = next(nginx.rglob('mime.types'), None)
        if mime is None: raise RuntimeError('Nginx MIME configuration missing from bottle')
        copy(mime, target / 'nginx/conf/mime.types')
        relocate(target, bottles)
        for name, keg in bottles.kegs.items():
            for file in keg.rglob('*'):
                if file.is_file() and any(word in file.name.upper() for word in ('LICENSE', 'COPYING', 'NOTICE')):
                    copy(file, target / 'licenses' / name.replace('@', '-') / file.relative_to(keg))


def verify(target):
    suffix = '.exe' if platform.system() == 'Windows' else ''
    php = target / 'php/bin' / ('php' + suffix)
    ini = target / 'verify.ini'
    if suffix:
        ext = (target / 'php/ext').as_posix()
        ini.write_text(f'extension_dir="{ext}"\n' + '\n'.join(f'extension={name}' for name in
            ('curl', 'fileinfo', 'gd', 'intl', 'mbstring', 'mysqli', 'openssl', 'pdo_mysql', 'pdo_sqlite', 'sodium', 'sqlite3', 'zip')))
    else: ini.write_text('')
    env = {**os.environ, 'PHP_INI_SCAN_DIR': '', 'MIBS': ''}
    code = 'echo json_encode(["version"=>PHP_VERSION,"extensions"=>get_loaded_extensions()]);'
    output = subprocess.check_output([str(php), '-c', str(ini), '-r', code], env=env, text=True)
    info = json.loads(output)
    missing = set(MANIFEST['extensions']) - {item.lower() for item in info['extensions']}
    if info['version'] != MANIFEST['php'] or missing:
        raise RuntimeError(f'PHP validation failed: version={info["version"]}, missing={sorted(missing)}')
    subprocess.run([str(php), '-c', str(ini), str(target / 'composer.phar'), '--version'], env=env, check=True)
    subprocess.run([str(target / 'nginx/bin' / ('nginx' + suffix)), '-v'], check=True)
    subprocess.run([str(target / 'mysql/bin' / ('mysqld' + suffix)), '--no-defaults', '--version'], check=True)
    ini.unlink()


def main():
    global RESUME_LOCK
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--refresh-lock', action='store_true', help='Trust upstream HTTPS once; review and commit resulting SHA256 lock before release')
    parser.add_argument('--resume-lock', action='store_true', help='Resume interrupted preparation, retaining existing pins and recording missing inputs')
    parser.add_argument('--verify-only', action='store_true')
    args = parser.parse_args()
    RESUME_LOCK = args.resume_lock
    refresh = args.refresh_lock or args.resume_lock
    if args.verify_only: verify(DEST); return
    lock = json.loads(LOCK_PATH.read_text()) if LOCK_PATH.exists() else {}
    work = ROOT / '.runtime-work/staged'
    if work.exists(): shutil.rmtree(work)
    work.mkdir(parents=True)
    try:
        common(work, lock, refresh)
        if platform.system() == 'Darwin': macos(work, lock, refresh)
        elif platform.system() == 'Windows': windows(work, lock, refresh)
        else: raise RuntimeError('Runtime packages are supported only on Windows and Apple Silicon macOS')
        (work / 'manifest.json').write_text(json.dumps(MANIFEST, indent=2) + '\n')
        copy(ROOT / 'THIRD_PARTY_NOTICES.md', work / 'THIRD_PARTY_NOTICES.md')
        verify(work)
        if DEST.exists(): shutil.rmtree(DEST)
        shutil.move(work, DEST)
        if refresh: LOCK_PATH.write_text(json.dumps(lock, indent=2, sort_keys=True) + '\n')
        print(f'Runtime prepared at {DEST}', flush=True)
    except Exception:
        # Retain verified download records so interrupted staging can resume.
        if refresh: LOCK_PATH.write_text(json.dumps(lock, indent=2, sort_keys=True) + '\n')
        raise


if __name__ == '__main__': main()
