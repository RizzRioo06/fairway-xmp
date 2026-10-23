#!/usr/bin/env python3
"""Exercise PCRE JIT, PostgreSQL drivers and Composer in every signed bundled PHP."""
import os
from pathlib import Path
import subprocess
import sys


def main():
    runtime = Path(sys.argv[1]) / 'Contents/Resources/runtime'
    environment = {**os.environ, 'MIBS': '', 'PHP_INI_SCAN_DIR': ''}
    interpreters = sorted(runtime.glob('php*/bin/php'))
    if len(interpreters) != 4: raise RuntimeError('Expected all four signed PHP versions')
    for php in interpreters:
        subprocess.run([str(php), '-n', '-r', 'set_error_handler(function($severity, $message) { throw new Exception($message); }); if (preg_match("/hello/", "hello") !== 1) exit(1); echo "PCRE JIT OK: ", PHP_VERSION, PHP_EOL;'], env=environment, check=True)
        subprocess.run([str(php), '-n', '-r', 'if (!extension_loaded("pdo_pgsql") || !extension_loaded("pgsql") || !in_array("pgsql", PDO::getAvailableDrivers(), true)) exit(1); echo "PostgreSQL clients OK: ", PHP_VERSION, PHP_EOL;'], env=environment, check=True)
        subprocess.run([str(php), '-n', str(runtime / 'composer.phar'), '--version'], env=environment, check=True)
    print('PASS: signed PHP regex JIT, PostgreSQL clients and Composer on all four versions')


if __name__ == '__main__': main()
