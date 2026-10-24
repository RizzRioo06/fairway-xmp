import hashlib
import importlib.util
import io
import json
from pathlib import Path
import tempfile
import tarfile
import unittest
import zipfile
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location('runtime', Path(__file__).resolve().parents[1] / 'scripts/prepare_runtime.py')
runtime = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(runtime)


class ArchiveTests(unittest.TestCase):
    def test_verification_rejects_missing_client_or_worker_extension_on_each_php_version(self):
        for folder in ('php', 'php-8.2', 'php-8.3', 'php-8.4'):
            for extension in ('pdo_pgsql', 'pgsql', 'sockets', 'soap', 'ldap', 'pcntl', 'posix'):
                def output(command, **kwargs):
                    php_folder = Path(command[0]).parts[-3]
                    version = runtime.MANIFEST['php'] if php_folder == 'php' else runtime.MANIFEST['phpVersions'][php_folder[4:]]
                    missing = extension if php_folder == folder else None
                    return json.dumps({'version': version, 'extensions': [name for name in runtime.required_php_extensions(runtime.MANIFEST, 'Darwin') if name != missing]})
                with self.subTest(folder=folder, extension=extension), tempfile.TemporaryDirectory() as temporary, \
                        patch.object(runtime.platform, 'system', return_value='Darwin'), \
                        patch.object(runtime.subprocess, 'check_output', side_effect=output):
                    target = Path(temporary)
                    (target / 'manifest.json').write_text(json.dumps(runtime.MANIFEST))
                    with self.assertRaisesRegex(RuntimeError, extension):
                        runtime.verify(target)

    def test_worker_extensions_are_required_only_on_unix(self):
        for system in ('Darwin', 'Linux', 'Windows'):
            with self.subTest(system=system):
                required = runtime.required_php_extensions(runtime.MANIFEST, system)
                self.assertTrue({'sockets', 'soap', 'ldap'}.issubset(required))
                self.assertEqual({'pcntl', 'posix'} & required,
                                 set() if system == 'Windows' else {'pcntl', 'posix'})

    def test_windows_verifier_loads_shipped_clients_without_unix_extensions(self):
        def output(command, **kwargs):
            php_folder = Path(command[0]).parts[-3]
            if '-r' in command:
                ini = Path(command[command.index('-c') + 1]).read_text()
                for extension in ('sockets', 'soap', 'ldap'):
                    self.assertIn(f'extension={extension}', ini)
                self.assertNotIn('extension=pcntl', ini)
                self.assertNotIn('extension=posix', ini)
                version = runtime.MANIFEST['php'] if php_folder == 'php' else runtime.MANIFEST['phpVersions'][php_folder[4:]]
                return json.dumps({'version': version, 'extensions': runtime.MANIFEST['extensions']})
            if any('composer.phar' in str(part) for part in command):
                return f'Composer version {runtime.MANIFEST["composer"]} bundled'
            if '-v' in command: return f'nginx/{runtime.MANIFEST["nginx"]}'
            if 'version' in command: return f'Mailpit v{runtime.MANIFEST["mailpit"]} bundled'
            return f'mysqld Ver {runtime.MANIFEST["mysql"]} bundled'

        with tempfile.TemporaryDirectory() as temporary, \
                patch.object(runtime.platform, 'system', return_value='Windows'), \
                patch.object(runtime.subprocess, 'check_output', side_effect=output), \
                patch('sys.stdout', new=io.StringIO()):
            target = Path(temporary)
            (target / 'manifest.json').write_text(json.dumps(runtime.MANIFEST))
            for folder in ('php', 'php-8.2', 'php-8.3', 'php-8.4'):
                ext = target / folder / 'ext'
                ext.mkdir(parents=True)
                for name in ('sockets', 'soap', 'ldap'):
                    (ext / f'php_{name}.dll').touch()
            runtime.verify(target)

    def test_runtime_verification_is_offline_and_still_rejects_wrong_mailpit_version(self):
        manifest = {'php': '8.5.11', 'extensions': [], 'composer': '2.10.3',
                    'nginx': '1.30.5', 'mysql': '9.7.2', 'mailpit': '1.31.6'}
        for mailpit_version in ('1.31.6', '0.0.0'):
            def offline_output(command, **kwargs):
                if '-r' in command:
                    return json.dumps({'version': manifest['php'], 'extensions': []})
                if 'version' in command:
                    if '--no-release-check' not in command:
                        raise OSError('Release lookup is unavailable offline')
                    return f'mailpit v{mailpit_version} compiled with Go'
                if any('composer.phar' in str(part) for part in command):
                    return f'Composer version {manifest["composer"]} built'
                if '-v' in command:
                    return f'nginx/{manifest["nginx"]}'
                return f'mysqld Ver {manifest["mysql"]} bundled'

            with self.subTest(version=mailpit_version), tempfile.TemporaryDirectory() as folder, \
                    patch.object(runtime, 'MANIFEST', manifest), \
                    patch.object(runtime.platform, 'system', return_value='Darwin'), \
                    patch.object(runtime.subprocess, 'check_output', side_effect=offline_output), \
                    patch('sys.stdout', new=io.StringIO()):
                (Path(folder) / 'manifest.json').write_text(json.dumps(manifest))
                if mailpit_version == manifest['mailpit']:
                    runtime.verify(Path(folder))
                else:
                    with self.assertRaisesRegex(RuntimeError, 'Runtime version mismatch'):
                        runtime.verify(Path(folder))

    def test_verification_rejects_missing_or_stale_staged_manifest_before_running_binaries(self):
        with tempfile.TemporaryDirectory() as folder, patch.object(runtime.subprocess, 'check_output') as execute:
            target = Path(folder)
            with self.assertRaisesRegex(RuntimeError, 'manifest is missing'):
                runtime.verify(target)
            stale = {**runtime.MANIFEST, 'phpVersions': {**runtime.MANIFEST['phpVersions'], '8.0': '8.0.30'}}
            (target / 'manifest.json').write_text(json.dumps(stale))
            with self.assertRaisesRegex(RuntimeError, 'manifest is out of date'):
                runtime.verify(target)
            execute.assert_not_called()

    def test_verification_rejects_removed_php_binaries_even_with_a_current_manifest(self):
        with tempfile.TemporaryDirectory() as folder, patch.object(runtime.subprocess, 'check_output') as execute:
            target = Path(folder)
            (target / 'manifest.json').write_text(json.dumps(runtime.MANIFEST))
            (target / 'php-8.0').mkdir()
            (target / 'php-8.1').mkdir()
            with self.assertRaisesRegex(RuntimeError, 'unexpected PHP folders: php-8.0, php-8.1'):
                runtime.verify(target)
            execute.assert_not_called()

    def test_php_8_0_and_8_1_are_not_bundled(self):
        root = Path(__file__).resolve().parents[1]
        manifest = json.loads((root / 'runtime-manifest.json').read_text())
        self.assertEqual(list(manifest['phpVersions']), ['8.2', '8.3', '8.4', '8.5'])
        self.assertFalse((root / 'scripts/windows_legacy_sources.json').exists())
        downloads = json.loads((root / 'runtime-downloads.lock.json').read_text())
        linux = json.loads((root / 'runtime-linux-downloads.lock.json').read_text())
        sources = json.loads((root / 'runtime-sources.lock.json').read_text())
        for name in (*downloads, *linux, *(item['name'] for item in sources)):
            self.assertNotRegex(name, r'8\.0|8\.1')
        self.assertIn('brew-icu4c@78-arm64-sequoia', downloads)
        for item in sources:
            versions = set(item.get('phpVersions') or [])
            self.assertFalse(versions & {'8.0', '8.1'})

    def test_windows_archive_cannot_escape_runtime_folder(self):
        for malicious in ('../outside.txt', '/outside.txt', 'C:/outside.txt', '..\\outside.txt'):
            with self.subTest(path=malicious), tempfile.TemporaryDirectory() as folder:
                root = Path(folder)
                archive = root / 'archive.zip'
                with zipfile.ZipFile(archive, 'w') as bundle: bundle.writestr(malicious, 'unsafe')
                with self.assertRaises(ValueError): runtime.safe_extract(archive, root / 'out', 'zip')
                self.assertFalse((root / 'outside.txt').exists())

    def test_valid_archive_extracts(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            archive = root / 'archive.zip'
            with zipfile.ZipFile(archive, 'w') as bundle: bundle.writestr('php/bin/php.exe', 'binary')
            runtime.safe_extract(archive, root / 'out', 'zip')
            self.assertEqual((root / 'out/php/bin/php.exe').read_text(), 'binary')

    def test_bottle_external_link_is_skipped_without_writing_outside(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            archive = root / 'bottle.tar'
            with tarfile.open(archive, 'w') as bundle:
                link = tarfile.TarInfo('nginx/html')
                link.type = tarfile.SYMTYPE
                link.linkname = '../../outside'
                bundle.addfile(link)
            with self.assertRaises(tarfile.LinkOutsideDestinationError):
                runtime.safe_extract(archive, root / 'strict', 'tar')
            runtime.safe_extract(archive, root / 'bottle', 'tar', skip_external_links=True)
            self.assertFalse((root / 'bottle/nginx/html').exists())
            self.assertFalse((root / 'outside').exists())

    def test_checksum_mismatch_refuses_cached_download(self):
        with tempfile.TemporaryDirectory() as folder:
            previous = runtime.CACHE
            runtime.CACHE = Path(folder)
            try:
                url = 'https://example.invalid/runtime.zip'
                cached = runtime.CACHE / hashlib.sha256(url.encode()).hexdigest()
                cached.write_bytes(b'tampered')
                with self.assertRaises(RuntimeError): runtime.download(url, '0' * 64)
                self.assertFalse(cached.exists())
            finally: runtime.CACHE = previous


if __name__ == '__main__': unittest.main()
