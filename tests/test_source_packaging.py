import io
from pathlib import Path
import sys
import tarfile
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'scripts'))
from package_sources import archive_git_tree


class SourceTreeTests(unittest.TestCase):
    def archive(self, root, filename, content=b'original source', mode=0o644, mtime=0, names=None):
        path = root / filename
        with tarfile.open(path, 'w:gz') as bundle:
            for name in names or ['src/code.c']:
                member = tarfile.TarInfo(name)
                member.size, member.mode, member.mtime = len(content), mode, mtime
                bundle.addfile(member, io.BytesIO(content))
        return path

    def test_git_tree_ignores_archive_metadata_but_detects_source_and_mode_changes(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            original = archive_git_tree(self.archive(root, 'original.tar.gz'))
            self.assertEqual(original, archive_git_tree(self.archive(root, 'repacked.tar.gz', mtime=123456)))
            self.assertNotEqual(original, archive_git_tree(self.archive(root, 'changed.tar.gz', content=b'changed source')))
            self.assertNotEqual(original, archive_git_tree(self.archive(root, 'executable.tar.gz', mode=0o755)))

    def test_source_tree_refuses_duplicate_and_escaping_paths(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            for names in [['../outside'], ['/outside'], ['source.c', 'source.c']]:
                with self.subTest(names=names):
                    with self.assertRaises(RuntimeError):
                        archive_git_tree(self.archive(root, 'unsafe.tar.gz', names=names))


if __name__ == '__main__': unittest.main()
