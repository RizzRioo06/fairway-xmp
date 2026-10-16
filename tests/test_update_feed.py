import copy
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

SCRIPTS = Path(__file__).resolve().parents[1] / 'scripts'
sys.path.insert(0, str(SCRIPTS))
import release_candidate as candidate
import update_feed


class UpdateFeedTests(unittest.TestCase):
    def fixture(self, root):
        version = '0.1.0-beta.3'
        manifest = {'version': version, 'commit': 'a' * 40, 'signing': {'macos-arm64': 'apple-notarized', 'windows-x64': 'updater-only', 'linux-x86_64': 'unsigned'}, 'candidates': {}}
        directories = {}
        for target, name in [('macos-arm64', f'fairway-xmp-{version}-macos-arm64.app.tar.gz'), ('windows-x64', f'fairway-xmp-{version}-windows-x64-setup.exe')]:
            directory = root / target; directory.mkdir(); directories[target] = directory
            (directory / name).write_bytes(b'exact candidate bytes')
            (directory / (name + '.sig')).write_text('test signature fixture')
            manifest['candidates'][target] = {'version': version, 'commit': manifest['commit'], 'lifecycle': {'updater': {'status': 'passed'}}, 'updater': {'artifact': name, 'signature': name + '.sig'}, 'files': {p.name: {'sha256': candidate.digest(p), 'bytes': p.stat().st_size} for p in directory.iterdir()}}
        return manifest, directories

    def test_feed_uses_verified_versioned_assets_and_both_platforms(self):
        with tempfile.TemporaryDirectory() as temporary:
            manifest, directories = self.fixture(Path(temporary))
            result = update_feed.generate(manifest, directories, 'eimg/fairway-xmp', 'Beta notes', '2026-10-16T00:00:00Z')
            self.assertEqual(set(result['platforms']), {'darwin-aarch64', 'windows-x86_64'})
            self.assertEqual(result['version'], '0.1.0-beta.3')
            for platform in result['platforms'].values():
                self.assertIn('/releases/download/v0.1.0-beta.3/', platform['url'])
                self.assertNotIn('/latest/', platform['url'])
            file = directories['windows-x64'] / manifest['candidates']['windows-x64']['updater']['artifact']
            file.write_bytes(b'tampered')
            with self.assertRaisesRegex(ValueError, 'changed or is missing'):
                update_feed.generate(manifest, directories, 'eimg/fairway-xmp', '')

    def test_feed_rejects_missing_signing_or_native_update_evidence(self):
        with tempfile.TemporaryDirectory() as temporary:
            manifest, directories = self.fixture(Path(temporary))
            for mutate in [lambda m: m['signing'].update({'macos-arm64': 'unsigned'}), lambda m: m['candidates']['windows-x64']['lifecycle']['updater'].update({'status': 'failed'}), lambda m: m['candidates']['macos-arm64'].update({'commit': 'b' * 40})]:
                bad = copy.deepcopy(manifest); mutate(bad)
                with self.assertRaises(ValueError): update_feed.generate(bad, directories, 'eimg/fairway-xmp', '')

    def test_beta_order_and_release_overlay(self):
        self.assertGreater(update_feed.beta_version('0.1.0-beta.10'), update_feed.beta_version('0.1.0-beta.9'))
        with self.assertRaises(ValueError): update_feed.beta_version('0.1.0')
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary) / 'overlay.json'
            subprocess.run([sys.executable, str(SCRIPTS / 'configure_release.py'), '--public-key', str(SCRIPTS.parent / 'packaging/updater.pub'), '--output', str(output)], check=True, capture_output=True)
            config = json.loads(output.read_text())
            self.assertTrue(config['bundle']['createUpdaterArtifacts'])
            self.assertTrue(config['plugins']['updater']['requireSignedVersion'])
            self.assertEqual(config['plugins']['updater']['endpoints'], ['https://github.com/eimg/fairway-xmp/releases/download/beta-feed/latest.json'])
            result = subprocess.run([sys.executable, str(SCRIPTS / 'configure_release.py'), '--public-key', str(SCRIPTS.parent / 'packaging/updater.pub'), '--endpoint', 'http://insecure.invalid'], capture_output=True)
            self.assertNotEqual(result.returncode, 0)
