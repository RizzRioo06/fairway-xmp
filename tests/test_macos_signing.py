import importlib.util
from pathlib import Path
import plistlib
import sys
import unittest

SCRIPTS = Path(__file__).resolve().parents[1] / 'scripts'
sys.path.insert(0, str(SCRIPTS))
import sign_macos


class SigningTests(unittest.TestCase):
    def test_only_php_interpreters_receive_the_jit_entitlement(self):
        app = Path('/build/Fairway XMP.app')
        runtime = app / 'Contents/Resources/runtime'
        for version in ('php', 'php-8.0', 'php-8.5'):
            for binary in ('php', 'php-cgi', 'php-fpm'):
                command = sign_macos.signing_command(runtime / version / 'bin' / binary, 'Developer ID', app)
                self.assertIn('--entitlements', command)
        for path in (app, app / 'Contents/MacOS/fairway', runtime / 'mysql/bin/mysqld', runtime / 'nginx/bin/nginx', runtime / 'lib/libpcre2.dylib', runtime / 'php-8.1/ext/opcache.so'):
            self.assertNotIn('--entitlements', sign_macos.signing_command(path, 'Developer ID', app))
        entitlements = plistlib.loads(sign_macos.ENTITLEMENTS.read_bytes())
        self.assertEqual(entitlements, {'com.apple.security.cs.allow-jit': True})
