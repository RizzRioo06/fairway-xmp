import hashlib
import importlib.util
import io
import json
from pathlib import Path
import tempfile
import unittest
import zipfile

spec = importlib.util.spec_from_file_location('windows_postgres_client', Path(__file__).resolve().parents[1] / 'scripts/windows_postgres_client.py')
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)

class WindowsClientManifestTests(unittest.TestCase):
    def archive(self, entries):
        out = io.BytesIO()
        with zipfile.ZipFile(out, 'w') as z:
            for name, value in entries.items(): z.writestr(name, value)
        return out.getvalue()

    def test_manifest_records_only_root_dlls_from_verified_archive(self):
        data = self.archive({'libpq.dll': b'fixture-pq', 'libssl.dll': b'fixture-ssl', 'ext/php_pgsql.dll': b'extension', 'php.exe': b'exe'})
        source = {'sha256': hashlib.sha256(data).hexdigest()}
        with tempfile.TemporaryDirectory() as root:
            root = Path(root); archive = root / 'php.zip'; archive.write_bytes(data)
            module.write_client_manifest(archive, source, root)
            value = json.loads((root / 'windows-postgresql-client.json').read_text())
            self.assertEqual(set(value['files']), {'libpq.dll', 'libssl.dll'})
            self.assertEqual(value['sourceSha256'], source['sha256'])
            self.assertEqual(value['files']['libpq.dll']['sha256'], hashlib.sha256(b'fixture-pq').hexdigest())

    def test_checksum_failure_never_writes_a_pin(self):
        with tempfile.TemporaryDirectory() as root:
            root = Path(root); archive = root / 'php.zip'; archive.write_bytes(self.archive({'libpq.dll': b'fixture'}))
            with self.assertRaises(ValueError): module.write_client_manifest(archive, {'sha256': '0'*64}, root)
            self.assertFalse((root / 'windows-postgresql-client.json').exists())

    def test_missing_client_duplicate_names_and_device_paths_are_rejected(self):
        for entries in [{'other.dll': b'x'}, {'libpq.dll': b'x', 'LIBPQ.DLL': b'y'}, {'libpq.dll': b'x', 'C:bad.dll': b'y'}]:
            with self.subTest(entries=list(entries)), zipfile.ZipFile(io.BytesIO(self.archive(entries))) as z:
                with self.assertRaises(ValueError): module.client_manifest(z, {'sha256': 'a'*64})
