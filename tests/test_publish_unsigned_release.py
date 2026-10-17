import importlib.util
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest import mock


spec = importlib.util.spec_from_file_location(
    'publish_unsigned_release', Path(__file__).resolve().parents[1] / 'scripts/publish_unsigned_release.py',
)
publisher = importlib.util.module_from_spec(spec)
spec.loader.exec_module(publisher)


class ReleaseTagTests(unittest.TestCase):
    def test_uploaded_draft_is_found_without_the_published_tag_endpoint(self):
        release = {'id': 42, 'tag_name': 'v0.1.0-beta.3', 'draft': True}
        with mock.patch.object(publisher, 'run_json', return_value=[release]) as api:
            self.assertEqual(publisher.find_draft_release('eimg/fairway-xmp', release['tag_name']), release)
            api.assert_called_once_with('gh', 'api', 'repos/eimg/fairway-xmp/releases?per_page=100')
        for releases in ([], [release, release], [{**release, 'draft': False}]):
            with self.subTest(releases=releases), mock.patch.object(publisher, 'run_json', return_value=releases):
                with self.assertRaises(RuntimeError):
                    publisher.find_draft_release('eimg/fairway-xmp', release['tag_name'])

    def test_uploaded_assets_require_the_exact_set_and_content(self):
        with tempfile.TemporaryDirectory() as temporary:
            stage = Path(temporary)
            artifact = stage / 'installer.pkg'
            artifact.write_bytes(b'tested installer')
            asset = {'name': artifact.name, 'size': artifact.stat().st_size, 'digest': 'sha256:' + publisher.sha256(artifact)}
            publisher.verify_uploaded_assets({'assets': [asset]}, stage)
            for assets in ([], [asset, asset], [{**asset, 'digest': 'sha256:' + '0' * 64}],
                           [{**asset, 'size': 0}], [asset, {**asset, 'name': 'unexpected.exe'}]):
                with self.subTest(assets=assets), self.assertRaises(RuntimeError):
                    publisher.verify_uploaded_assets({'assets': assets}, stage)

    def test_release_tag_must_resolve_to_the_tested_commit(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)

            def git(*args):
                return subprocess.check_output([
                    'git', '-c', 'core.hooksPath=/dev/null', '-c', 'commit.gpgsign=false',
                    '-c', 'user.name=Release test', '-c', 'user.email=release-test@example.invalid',
                    *args,
                ], cwd=root, text=True).strip()

            git('init', '--quiet')
            git('commit', '--quiet', '--allow-empty', '-m', 'Tested candidate')
            candidate = git('rev-parse', 'HEAD')
            git('tag', 'v0.1.0-beta.2')
            publisher.verify_tag('v0.1.0-beta.2', candidate, root)
            git('commit', '--quiet', '--allow-empty', '-m', 'Later documentation')
            later = git('rev-parse', 'HEAD')
            with self.assertRaisesRegex(RuntimeError, 'does not identify the tested candidate'):
                publisher.verify_tag('v0.1.0-beta.2', later, root)
            git('tag', '-a', 'annotated-beta', '-m', 'Candidate', candidate)
            publisher.verify_tag('annotated-beta', candidate, root)
