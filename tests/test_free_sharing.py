"""Compile-time pin isolation and sharing identity checks without personal data."""
import json
import hashlib
import io
import os
from pathlib import Path
import plistlib
import shutil
import subprocess
import sys
import tempfile
import tarfile
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'scripts'))
from package_macos_sharing import verify_identity
import package_macos_sharing as sharing


@unittest.skipIf(os.name == 'nt', 'Bundle symlink fixtures require Unix')
class SharingCopyTests(unittest.TestCase):
    def test_internal_file_and_directory_aliases_become_independent_portable_files(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder).resolve()
            source = root / 'source'
            (source / 'data').mkdir(parents=True)
            original = source / 'data/tool'
            original.write_bytes(b'unchanged runtime bytes')
            original.chmod(0o755)
            (source / 'relative').symlink_to('data/tool')
            (source / 'absolute').symlink_to(original)
            (source / 'directory').symlink_to('data', target_is_directory=True)
            destination = root / 'copy'
            sharing.copy_bundle_tree(source, destination)
            for name in ('data/tool', 'relative', 'absolute', 'directory/tool'):
                copy = destination / name
                self.assertEqual(copy.read_bytes(), original.read_bytes())
                self.assertEqual(copy.stat().st_mode & 0o777, 0o755)
                self.assertNotEqual(copy.stat().st_ino, original.stat().st_ino)
            self.assertFalse(any(path.is_symlink() for path in destination.rglob('*')))
            self.assertTrue((source / 'absolute').is_symlink())
            with self.assertRaises(FileExistsError):
                sharing.copy_bundle_tree(source, destination)

    def test_external_broken_and_cyclic_links_fail_before_copying_any_files(self):
        for target in ('../outside', 'missing', '.', 'bad'):
            with self.subTest(target=target), tempfile.TemporaryDirectory() as folder:
                root = Path(folder).resolve()
                (root / 'outside').write_bytes(b'private unrelated data')
                source = root / 'source'
                source.mkdir()
                (source / 'good').write_bytes(b'valid prefix')
                (source / 'bad').symlink_to(target)
                destination = root / 'copy'
                with self.assertRaises(ValueError):
                    sharing.copy_bundle_tree(source, destination)
                self.assertFalse(destination.exists())
                self.assertEqual((root / 'outside').read_bytes(), b'private unrelated data')

    def test_source_aliases_and_special_files_are_rejected(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder).resolve()
            source = root / 'source'
            source.mkdir()
            alias = root / 'alias'
            alias.symlink_to(source, target_is_directory=True)
            with self.assertRaises(ValueError):
                sharing.copy_bundle_tree(alias, root / 'copy')
            os.mkfifo(source / 'pipe')
            with self.assertRaises(ValueError):
                sharing.copy_bundle_tree(source, root / 'copy')
            self.assertFalse((root / 'copy').exists())

    @unittest.skipUnless(sys.platform == 'darwin', 'Native codesign fixture requires macOS')
    def test_native_strict_signature_survives_moving_the_materialized_bundle(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder).resolve()
            app = root / 'Source.app'
            binary = app / 'Contents/MacOS/probe'
            binary.parent.mkdir(parents=True)
            # Copy a system executable as inert signing input; never execute it.
            shutil.copyfile('/usr/bin/true', binary)
            binary.chmod(0o755)
            (app / 'Contents/Info.plist').write_bytes(plistlib.dumps({
                'CFBundleExecutable': 'probe', 'CFBundleIdentifier': 'dev.fairway.sharing-fixture',
                'CFBundlePackageType': 'APPL', 'CFBundleVersion': '1'}))
            resources = app / 'Contents/Resources'
            resources.mkdir()
            (resources / 'notice').write_text('fixture notice')
            (resources / 'alias').symlink_to(resources / 'notice')
            def sign(path):
                subprocess.run(['/usr/bin/codesign', '--force', '--sign', '-', str(path)],
                               check=True, capture_output=True)
            def verify(path):
                return subprocess.run(['/usr/bin/codesign', '--verify', '--deep', '--strict',
                                       '--verbose=4', str(path)], capture_output=True, text=True)
            sign(app)
            before = verify(app)
            self.assertNotEqual(before.returncode, 0)
            self.assertIn('invalid destination for symbolic link', before.stderr)
            copied = root / 'Copied.app'
            sharing.copy_bundle_tree(app, copied)
            sign(copied)
            moved = root / 'Moved.app'
            copied.rename(moved)
            after = verify(moved)
            self.assertEqual(after.returncode, 0, after.stderr)
            self.assertTrue((resources / 'alias').is_symlink())


class SharingIdentityTests(unittest.TestCase):
    def test_source_archives_must_match_commit_runtime_pin_and_official_postgresql_source(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            pg_source = b'unchanged source fixture'
            lock = {'version': '18.6', 'sha256': hashlib.sha256(pg_source).hexdigest()}
            (root / 'postgresql-source.lock.json').write_text(json.dumps(lock))
            (root / 'runtime-manifest.json').write_text('{"php":"fixture"}')
            pin = {'manifestSha256': 'a' * 64}
            entries = {'README.md': b'Fairway XMP source commit: reviewed.', 'runtime-manifest.json': b'{"php":"fixture"}', 'postgresql/runtime-pin.json': json.dumps(pin).encode(), 'postgresql/postgresql-source.lock.json': json.dumps(lock).encode(), 'postgresql/postgresql-18.6-source.tar.bz2': pg_source, 'postgresql/PostgreSQL-COPYRIGHT': b'notice', 'postgresql/timezone-README': b'notice', 'fairway-xmp-source.tar.gz': b'app sources'}
            archive = root / 'sources.tar.gz'
            with tarfile.open(archive, 'w:gz') as bundle:
                for name, data in entries.items():
                    member = tarfile.TarInfo('fairway-xmp-0.2.0-rc.1-sources/' + name); member.size = len(data)
                    bundle.addfile(member, io.BytesIO(data))
            with mock.patch.object(sharing, 'ROOT', root):
                sharing.verify_sources(archive, pin, 'reviewed', '0.2.0-rc.1')
                with self.assertRaisesRegex(ValueError, 'another commit'):
                    sharing.verify_sources(archive, pin, 'other', '0.2.0-rc.1')
                with self.assertRaisesRegex(ValueError, 'another PostgreSQL payload'):
                    sharing.verify_sources(archive, {}, 'reviewed', '0.2.0-rc.1')
                (root / 'postgresql-source.lock.json').write_text(json.dumps({**lock, 'sha256': '0' * 64}))
                with self.assertRaises(ValueError):
                    sharing.verify_sources(archive, pin, 'reviewed', '0.2.0-rc.1')

    def test_free_workflow_uses_no_credentials_or_publication(self):
        text = (ROOT / '.github/workflows/free-sharing-builds.yml').read_text()
        for forbidden in ('${{ secrets.', 'check_release_setup.py', 'publish_release.py', 'configure_release.py', '--features native-macos-trust,native-macos-hosts'):
            self.assertNotIn(forbidden, text)
        self.assertIn('signed_release: false', text)
        self.assertIn('authorize_postgresql_source_build', text)
        self.assertIn('FAIRWAY_DISTRIBUTION: free-sharing', text)
        self.assertLess(text.index('configure_free_sharing.py'), text.index('npm run tauri -- build'))
        self.assertIn('scripts/package_sources.py --locked', text)

    def test_distribution_platform_features_and_exact_pin_must_match(self):
        pin = {'version': '18.6', 'platform': 'macos-arm64', 'manifestSha256': 'a' * 64}
        identity = {'distribution': 'free-sharing', 'commit': 'b' * 40, 'version': '0.2.0-rc.1', 'os': 'macos', 'architecture': 'aarch64', 'postgresqlRuntimePin': pin, 'features': {'nativeMacosHosts': False, 'nativeMacosTrust': True}}
        verify_identity(identity, pin, 'b' * 40, '0.2.0-rc.1')
        for key, value in [('distribution', 'standard'), ('commit', 'c' * 40), ('architecture', 'x86_64'), ('postgresqlRuntimePin', {**pin, 'manifestSha256': '0' * 64}), ('features', {'nativeMacosHosts': True, 'nativeMacosTrust': True})]:
            with self.subTest(key=key), self.assertRaises(ValueError):
                verify_identity({**identity, key: value}, pin, 'b' * 40, '0.2.0-rc.1')


class CompileTimePinTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        compiler = shutil.which('rustc') or str(Path.home() / '.cargo/bin/rustc')
        if not Path(compiler).is_file():
            raise unittest.SkipTest('Rust compiler unavailable for build-script fixtures')
        cls.scratch = tempfile.TemporaryDirectory()
        cls.binary = Path(cls.scratch.name) / ('build-script.exe' if os.name == 'nt' else 'build-script')
        subprocess.run([compiler, '--edition=2021', str(ROOT / 'src-tauri/build.rs'), '-o', str(cls.binary)], check=True, capture_output=True)

    @classmethod
    def tearDownClass(cls):
        cls.scratch.cleanup()

    def test_standard_and_windows_builds_ignore_free_pin_and_free_mac_fails_without_it(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder).resolve()
            default = root / 'src-tauri/src/postgresql/managed/runtime-pin.json'
            default.parent.mkdir(parents=True); default.write_text('committed pin fixture')
            free = root / 'artifacts/free-sharing/runtime-pin.json'
            free.parent.mkdir(parents=True); free.write_text('free pin fixture')
            output = root / 'out'; output.mkdir()
            env = {**os.environ, 'CARGO_MANIFEST_DIR': str(root / 'src-tauri'), 'OUT_DIR': str(output), 'CARGO_CFG_TARGET_ARCH': 'aarch64'}
            for mode, target, expected in [('standard', 'macos', 'committed pin fixture'), ('free-sharing', 'windows', 'committed pin fixture'), ('free-sharing', 'macos', 'free pin fixture')]:
                with self.subTest(mode=mode, target=target):
                    result = subprocess.run([str(self.binary)], env={**env, 'FAIRWAY_DISTRIBUTION': mode, 'CARGO_CFG_TARGET_OS': target}, capture_output=True, text=True)
                    self.assertEqual(result.returncode, 0, result.stderr)
                    self.assertEqual((output / 'postgresql-runtime-pin.json').read_text(), expected)
                    self.assertEqual(default.read_text(), 'committed pin fixture')
            free.unlink()
            result = subprocess.run([str(self.binary)], env={**env, 'FAIRWAY_DISTRIBUTION': 'free-sharing', 'CARGO_CFG_TARGET_OS': 'macos'}, capture_output=True)
            self.assertNotEqual(result.returncode, 0, 'A free build must never silently fall back to the release pin')
            if os.name != 'nt':
                free.symlink_to(default)
                result = subprocess.run([str(self.binary)], env={**env, 'FAIRWAY_DISTRIBUTION': 'free-sharing', 'CARGO_CFG_TARGET_OS': 'macos'}, capture_output=True)
                self.assertNotEqual(result.returncode, 0)


if __name__ == '__main__':
    unittest.main()
