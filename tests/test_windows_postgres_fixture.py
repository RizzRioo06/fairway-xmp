import hashlib
import importlib.util
from pathlib import Path
import tempfile
import unittest
import zipfile

spec = importlib.util.spec_from_file_location('pg_fixture', Path(__file__).resolve().parents[1] / 'scripts/prepare_windows_postgres_fixture.py')
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class WindowsPostgresFixtureTests(unittest.TestCase):
    def archive(self, root, extra=()):
        archive = root / 'fixture.zip'
        with zipfile.ZipFile(archive, 'w') as bundle:
            for tool in ('postgres', 'initdb', 'pg_ctl'):
                bundle.writestr(f'pgsql/bin/{tool}.exe', b'test-only')
            bundle.writestr('pgsql/share/postgres.bki', b'catalog')
            bundle.writestr('pgsql/pgAdmin 4/unused', b'not staged')
            for name in extra:
                bundle.writestr(name, b'bad')
        return archive, hashlib.sha256(archive.read_bytes()).hexdigest()

    def test_verified_tools_only_and_no_overwrite(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder).resolve()
            archive, digest = self.archive(root)
            destination = root / 'tools'
            module.extract_verified(archive, destination, digest)
            self.assertEqual((destination / 'pgsql/share/postgres.bki').read_bytes(), b'catalog')
            self.assertFalse((destination / 'pgsql/pgAdmin 4').exists())
            with self.assertRaises(FileExistsError):
                module.extract_verified(archive, destination, digest)
            self.assertEqual((destination / 'pgsql/bin/postgres.exe').read_bytes(), b'test-only')

    def test_checksum_before_creation(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder).resolve()
            archive, _ = self.archive(root)
            with self.assertRaises(ValueError):
                module.extract_verified(archive, root / 'tools', '0' * 64)
            self.assertFalse((root / 'tools').exists())

    def test_unsafe_and_duplicate_names_before_creation(self):
        for name in ('../escape', '/absolute', 'pgsql/bin/C:bad', 'pgsql/bin/CON.dll',
                     'pgsql/bin/POSTGRES.exe', 'pgsql/bin/back\\slash', 'pgsql/bin/trailing.'):
            with self.subTest(name=name), tempfile.TemporaryDirectory() as folder:
                root = Path(folder).resolve()
                archive, digest = self.archive(root, (name,))
                with self.assertRaises(ValueError):
                    module.extract_verified(archive, root / 'tools', digest)
                self.assertFalse((root / 'tools').exists())

    def test_archive_symlink_rejected(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder).resolve()
            archive, _ = self.archive(root)
            with zipfile.ZipFile(archive, 'a') as bundle:
                link = zipfile.ZipInfo('pgsql/bin/link')
                link.create_system = 3
                link.external_attr = 0o120777 << 16
                bundle.writestr(link, '../outside')
            with self.assertRaises(ValueError):
                module.extract_verified(archive, root / 'tools', hashlib.sha256(archive.read_bytes()).hexdigest())
            self.assertFalse((root / 'tools').exists())
