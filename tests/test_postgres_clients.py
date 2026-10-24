import importlib.util
from pathlib import Path
import subprocess
import unittest
from unittest.mock import patch

SCRIPT = Path(__file__).resolve().parents[1] / 'scripts/check_postgres_clients.py'
SPEC = importlib.util.spec_from_file_location('postgres_clients', SCRIPT)
clients = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(clients)


class WindowsPostgresFixtureTests(unittest.TestCase):
    def test_restricted_token_gets_inherited_access_through_user_sid(self):
        root = Path('private fixture')
        sid = 'S-1-5-21-123-456-789-1001'
        with patch.object(clients.subprocess, 'check_output', return_value=f'"runner\\user, name","{sid}"\r\n') as identity, \
                patch.object(clients.subprocess, 'run') as grant:
            clients.grant_windows_fixture_access(root)
        identity.assert_called_once_with(['whoami', '/user', '/fo', 'csv', '/nh'],
                                         text=True, encoding='utf-8', errors='replace', timeout=15)
        grant.assert_called_once_with(['icacls', str(root), '/inheritance:r', '/grant:r', f'*{sid}:(OI)(CI)F'],
                                      check=True, stdout=subprocess.DEVNULL, timeout=30)

    def test_unknown_identity_fails_before_changing_permissions(self):
        for identity in ('', '"runner\\user","Administrators"\n', '"runner\\user"\n',
                         '"user","S-1-5-21-1001"\n"other","S-1-5-21-1002"\n'):
            with self.subTest(identity=identity), \
                    patch.object(clients.subprocess, 'check_output', return_value=identity), \
                    patch.object(clients.subprocess, 'run') as grant:
                with self.assertRaisesRegex(RuntimeError, 'Windows user SID'):
                    clients.grant_windows_fixture_access(Path('private fixture'))
                grant.assert_not_called()

    def test_permission_failure_stops_the_check(self):
        with patch.object(clients.subprocess, 'check_output', return_value='"runner\\user","S-1-5-21-1001"\n'), \
                patch.object(clients.subprocess, 'run', side_effect=subprocess.CalledProcessError(1, 'icacls')):
            with self.assertRaises(subprocess.CalledProcessError):
                clients.grant_windows_fixture_access(Path('private fixture'))


if __name__ == '__main__':
    unittest.main()
