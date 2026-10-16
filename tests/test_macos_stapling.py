from pathlib import Path
import subprocess
import sys
from types import SimpleNamespace
import unittest
from unittest.mock import Mock

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'scripts'))
from staple_macos import staple


class StaplingTests(unittest.TestCase):
    def test_network_failure_retries_then_requires_validation(self):
        run = Mock(side_effect=[SimpleNamespace(returncode=68), SimpleNamespace(returncode=0), SimpleNamespace(returncode=0)])
        sleep = Mock()
        staple('Fairway XMP.app', run, sleep)
        sleep.assert_called_once_with(5)
        self.assertEqual(run.call_args_list[-1].args[0], ['xcrun', 'stapler', 'validate', 'Fairway XMP.app'])
        self.assertTrue(run.call_args_list[-1].kwargs['check'])

    def test_retry_is_bounded_and_other_failures_are_immediate(self):
        run = Mock(return_value=SimpleNamespace(returncode=68)); sleep = Mock()
        with self.assertRaises(subprocess.CalledProcessError): staple('app', run, sleep)
        self.assertEqual(run.call_count, 4)
        self.assertEqual([call.args[0] for call in sleep.call_args_list], [5, 15, 30])
        run = Mock(return_value=SimpleNamespace(returncode=73)); sleep = Mock()
        with self.assertRaises(subprocess.CalledProcessError): staple('app', run, sleep)
        run.assert_called_once(); sleep.assert_not_called()
