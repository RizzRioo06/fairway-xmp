import copy
from pathlib import Path
import plistlib
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'scripts'))
import verify_macos_permissions as permissions


class MacPermissionPayloadTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.app = Path(self.temp.name) / 'Fairway XMP.app'
        self.helper = self.app / 'Contents/Library/HelperTools' / permissions.LABEL
        self.plist = self.app / 'Contents/Library/LaunchDaemons' / f'{permissions.LABEL}.plist'
        self.helper.parent.mkdir(parents=True)
        self.plist.parent.mkdir(parents=True)
        self.helper.write_bytes(b'fixture')
        self.helper.chmod(0o755)
        self.info = {
            'Label': permissions.LABEL,
            'BundleProgram': f'Contents/Library/HelperTools/{permissions.LABEL}',
            'ProgramArguments': [permissions.LABEL, '--server'],
            'MachServices': {permissions.LABEL: True},
            'ProcessType': 'Background',
            'AssociatedBundleIdentifiers': ['dev.fairway.environment'],
        }
        self.plist.write_bytes(plistlib.dumps(self.info))
        self.build = {'os': 'macos', 'features': {'nativeMacosHosts': True, 'nativeMacosTrust': True}}

    def test_features_and_payload_are_both_required(self):
        permissions.verify(self.app, self.build)
        for feature in self.build['features']:
            bad = copy.deepcopy(self.build)
            bad['features'][feature] = False
            with self.subTest(feature=feature), self.assertRaisesRegex(ValueError, 'both native'):
                permissions.verify(self.app, bad)
        with self.assertRaises(ValueError):
            permissions.verify(self.app, {'os': 'macos'})
        self.helper.unlink()
        with self.assertRaisesRegex(ValueError, 'missing'):
            permissions.verify(self.app, self.build)

    def test_changed_launch_capability_or_symlink_is_rejected(self):
        self.info['ProgramArguments'].append('/tmp/other-file')
        self.plist.write_bytes(plistlib.dumps(self.info))
        with self.assertRaisesRegex(ValueError, 'fixed capability'):
            permissions.verify(self.app, self.build)
        self.plist.unlink()
        try:
            self.plist.symlink_to(self.helper)
        except (OSError, NotImplementedError):
            self.skipTest('Creating symlinks requires platform permission')
        with self.assertRaisesRegex(ValueError, 'symlinks'):
            permissions.verify(self.app, self.build)

    def test_signed_payload_checks_the_exact_developer_id(self):
        with patch.object(permissions.subprocess, 'run') as run:
            permissions.verify(self.app, self.build, '4Q27NQ2UAF')
            args = run.call_args.args[0]
            self.assertIn('4Q27NQ2UAF', args[-2])
            self.assertIn(permissions.LABEL, args[-2])
            self.assertIn('100.6.1.13', args[-2])
            self.assertEqual(args[-1], str(self.helper))
            self.assertTrue(run.call_args.kwargs['check'])
