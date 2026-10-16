from io import StringIO
from pathlib import Path
import sys
import unittest
from unittest.mock import patch
from urllib.error import HTTPError

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'scripts'))
import prepare_runtime


class MetadataRetryTests(unittest.TestCase):
    def test_connection_failure_retries_and_releases_the_response(self):
        response = StringIO('{"version":"reviewed"}')
        with patch.object(prepare_runtime, 'request', side_effect=[ConnectionResetError(), response]) as request, patch.object(prepare_runtime.time, 'sleep') as sleep:
            self.assertEqual(prepare_runtime.read_json('https://example.test/metadata'), {'version': 'reviewed'})
            self.assertEqual(request.call_count, 2)
            sleep.assert_called_once_with(2)
            self.assertTrue(response.closed)

    def test_retries_are_bounded_and_permanent_errors_fail_immediately(self):
        with patch.object(prepare_runtime, 'request', side_effect=ConnectionResetError()) as request, patch.object(prepare_runtime.time, 'sleep') as sleep:
            with self.assertRaises(ConnectionResetError): prepare_runtime.read_json('https://example.test/metadata')
            self.assertEqual(request.call_count, 3)
            self.assertEqual([call.args[0] for call in sleep.call_args_list], [2, 4])
        with patch.object(prepare_runtime, 'request', side_effect=HTTPError('https://example.test/metadata', 404, 'Not found', {}, None)) as request, patch.object(prepare_runtime.time, 'sleep') as sleep:
            with self.assertRaises(HTTPError): prepare_runtime.read_json('https://example.test/metadata')
            request.assert_called_once(); sleep.assert_not_called()
