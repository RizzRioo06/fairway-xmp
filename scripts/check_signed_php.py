#!/usr/bin/env python3
"""Exercise PCRE JIT and Composer in every signed bundled PHP interpreter."""
import os
from pathlib import Path
import subprocess
import sys


def main():
    runtime = Path(sys.argv[1]) / 'Contents/Resources/runtime'
    environment = {**os.environ, 'MIBS': '', 'PHP_INI_SCAN_DIR': ''}
    interpreters = sorted(runtime.glob('php*/bin/php'))
    if len(interpreters) != 6: raise RuntimeError('Expected all six signed PHP versions')
    for php in interpreters:
        subprocess.run([str(php), '-n', '-r', 'set_error_handler(function($severity, $message) { throw new Exception($message); }); if (preg_match("/hello/", "hello") !== 1) exit(1); echo "PCRE JIT OK: ", PHP_VERSION, PHP_EOL;'], env=environment, check=True)
        subprocess.run([str(php), '-n', str(runtime / 'composer.phar'), '--version'], env=environment, check=True)
    print('PASS: signed PHP regex JIT and Composer on all six versions')


if __name__ == '__main__': main()
