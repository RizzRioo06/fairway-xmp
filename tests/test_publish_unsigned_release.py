import importlib.util
from pathlib import Path
import subprocess
import tempfile
import unittest


spec = importlib.util.spec_from_file_location(
    'publish_unsigned_release', Path(__file__).resolve().parents[1] / 'scripts/publish_unsigned_release.py',
)
publisher = importlib.util.module_from_spec(spec)
spec.loader.exec_module(publisher)


class ReleaseTagTests(unittest.TestCase):
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
