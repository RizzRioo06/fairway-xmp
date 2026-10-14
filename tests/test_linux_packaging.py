import importlib.util
from pathlib import Path
import sys
import unittest

SCRIPTS = Path(__file__).resolve().parents[1] / 'scripts'
sys.path.insert(0, str(SCRIPTS))
SPEC = importlib.util.spec_from_file_location('linux_runtime', SCRIPTS / 'prepare_linux_runtime.py')
runtime = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(runtime)


class LinuxCompatibilityTests(unittest.TestCase):
    def test_newer_glibc_requirement_is_rejected(self):
        runtime.validate_glibc('Name: GLIBC_2.28 Name: GLIBC_2.35')
        with self.assertRaises(RuntimeError):
            runtime.validate_glibc('Name: GLIBC_2.28 Name: GLIBC_2.36')
        with self.assertRaises(RuntimeError):
            runtime.validate_glibc('Name: GLIBC_2.9 Name: GLIBC_2.40')


if __name__ == '__main__': unittest.main()
