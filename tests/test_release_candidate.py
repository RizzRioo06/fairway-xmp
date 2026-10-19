import copy
import json
import os
from pathlib import Path
import sys
import tempfile
import subprocess
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'scripts'))
import release_candidate as candidate


class CandidateTests(unittest.TestCase):
    def manifest(self):
        result = {'version': '0.1.0-beta.1', 'commit': 'a' * 40, 'candidates': {}}
        for target in candidate.TARGETS:
            result['candidates'][target] = {'installer': 'installer', 'files': {'installer': {'sha256': 'b' * 64, 'bytes': 1}}}
        return result

    def evidence(self, manifest):
        return {'version': manifest['version'], 'commit': manifest['commit'], 'platforms': {target: {
            'installerSha256': 'b' * 64, 'tester': 'May', 'os': 'tested OS', 'testedAt': '2026-10-15T10:00:00+06:30',
            'checks': {name: True for name in candidate.MANUAL_CHECKS[target]},
        } for target in candidate.TARGETS}}

    def test_acceptance_is_bound_to_installer_and_commit(self):
        manifest = self.manifest()
        evidence = self.evidence(manifest)
        candidate.acceptance(manifest, evidence)
        for changed in ('commit', 'hash', 'missing-check', 'string-pass', 'tester'):
            bad = copy.deepcopy(evidence)
            record = bad['platforms']['windows-x64']
            if changed == 'commit': bad['commit'] = 'c' * 40
            if changed == 'hash': record['installerSha256'] = 'c' * 64
            if changed == 'missing-check': record['checks'].pop('tray-and-quit')
            if changed == 'string-pass': record['checks']['tray-and-quit'] = 'true'
            if changed == 'tester': record['tester'] = ''
            for phase in candidate.ACCEPTANCE_PHASES:
                with self.subTest(changed=changed, phase=phase), self.assertRaises(ValueError): candidate.acceptance(manifest, bad, phase)

    def test_only_explicitly_false_update_checks_can_be_deferred(self):
        manifest = self.manifest()
        evidence = self.evidence(manifest)
        for target in ('macos-arm64', 'windows-x64'):
            evidence['platforms'][target]['checks']['in-app-update-and-restart'] = False
        original = copy.deepcopy(evidence)
        status = candidate.acceptance(manifest, evidence, 'pre-publication')
        self.assertEqual(status['status'], 'awaiting-post-publication')
        self.assertEqual(status['pendingChecks'], {target: ['in-app-update-and-restart'] for target in ('windows-x64', 'macos-arm64')})
        self.assertEqual(evidence, original)
        with self.assertRaises(ValueError): candidate.acceptance(manifest, evidence)
        for target, record in evidence['platforms'].items():
            for check in record['checks']:
                if check in candidate.POST_PUBLICATION_CHECKS:
                    continue
                bad = copy.deepcopy(evidence)
                bad['platforms'][target]['checks'][check] = False
                with self.subTest(target=target, check=check), self.assertRaises(ValueError):
                    candidate.acceptance(manifest, bad, 'pre-publication')
        for value in (None, 'false', 0, 1):
            bad = copy.deepcopy(evidence)
            bad['platforms']['windows-x64']['checks']['in-app-update-and-restart'] = value
            with self.subTest(value=value), self.assertRaises(ValueError): candidate.acceptance(manifest, bad, 'pre-publication')
        bad = copy.deepcopy(evidence)
        bad['platforms']['windows-x64']['checks'].pop('in-app-update-and-restart')
        with self.assertRaises(ValueError): candidate.acceptance(manifest, bad, 'pre-publication')
        with self.assertRaises(ValueError): candidate.acceptance(manifest, evidence, 'skip')
        for target in ('macos-arm64', 'windows-x64'):
            evidence['platforms'][target]['checks']['in-app-update-and-restart'] = True
        self.assertEqual(candidate.acceptance(manifest, evidence)['status'], 'complete')

    def test_acceptance_cli_defaults_to_all_checks(self):
        manifest = self.manifest()
        evidence = self.evidence(manifest)
        evidence['platforms']['macos-arm64']['checks']['in-app-update-and-restart'] = False
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            candidate.write(root / 'manifest.json', manifest)
            candidate.write(root / 'evidence.json', evidence)
            command = [sys.executable, str(candidate.ROOT / 'scripts/release_candidate.py'), 'check-acceptance',
                       '--manifest', str(root / 'manifest.json'), '--evidence', str(root / 'evidence.json')]
            self.assertNotEqual(subprocess.run(command, capture_output=True).returncode, 0)
            result = subprocess.run([*command, '--phase', 'pre-publication'], capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertIn('awaiting-post-publication', result.stdout)
            self.assertIn('in-app-update-and-restart', result.stdout)

    def test_downloaded_artifact_tampering_and_traversal_are_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'installer.exe'
            path.write_bytes(b'exact tested file')
            data = {'files': {path.name: {'sha256': candidate.digest(path), 'bytes': path.stat().st_size}}}
            candidate.verify_files(data, directory)
            path.write_bytes(b'changed installer')
            with self.assertRaises(ValueError): candidate.verify_files(data, directory)
            data['files'] = {'../installer.exe': {'sha256': 'a' * 64, 'bytes': 1}}
            with self.assertRaises(ValueError): candidate.verify_files(data, directory)

    def test_assembly_requires_all_platforms_and_exact_identity(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for target in candidate.TARGETS:
                candidate.write(root / f'candidate-{target}.json', {'target': target, 'version': '0.1.0-beta.1', 'commit': 'a' * 40,
                    'runtime': {'php': '8.5.11'}, 'lifecycle': {'status': 'passed'}, 'installer': 'installer', 'files': {'installer': {'sha256': 'b' * 64}}})
            with patch.object(candidate, 'versions', return_value='0.1.0-beta.1'), patch.object(candidate.subprocess, 'check_output', side_effect=lambda args, **kwargs: '' if 'status' in args else 'a' * 40), patch.object(candidate, 'ROOT', root):
                candidate.write(root / 'runtime-manifest.json', {'php': '8.5.11'})
                result = candidate.assemble(root, root / 'output/manifest.json')
                self.assertEqual(result['status'], 'awaiting-human-acceptance')
                evidence = candidate.read(root / 'output/acceptance-template.json')
                self.assertEqual(evidence['postPublicationChecks'], ['in-app-update-and-restart'])
                self.assertTrue(all(value is False for record in evidence['platforms'].values() for value in record['checks'].values()))
                with self.assertRaises(ValueError): candidate.acceptance(result, evidence)
                data = candidate.read(root / 'candidate-windows-x64.json'); data['commit'] = 'c' * 40
                candidate.write(root / 'candidate-windows-x64.json', data)
                with self.assertRaises(ValueError): candidate.assemble(root, root / 'output/manifest.json')
                (root / 'candidate-windows-x64.json').unlink()
                with self.assertRaises(ValueError): candidate.assemble(root, root / 'output/manifest.json')

    def test_repository_versions_agree(self):
        self.assertRegex(candidate.versions(), r'^\d+\.\d+\.\d+')

    def test_installer_lifecycle_refuses_local_and_self_hosted_accounts(self):
        script = Path(__file__).resolve().parent / 'installer_lifecycle.py'
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / 'must-not-exist'
            for github, runner in [('false', 'github-hosted'), ('true', 'self-hosted')]:
                result = subprocess.run([sys.executable, str(script), '--installer', 'missing-installer', '--baseline', 'missing-baseline',
                    '--probe', 'missing-probe', '--output', str(output), '--allow-user-profile-changes'],
                    env={**os.environ, 'GITHUB_ACTIONS': github, 'RUNNER_ENVIRONMENT': runner}, text=True, capture_output=True)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn('disposable GitHub-hosted runner', result.stderr)
                self.assertFalse(output.exists())

    def test_record_requires_clean_installed_binary_and_matching_runtime(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            output = root / 'windows'; output.mkdir()
            installer = output / 'fairway-xmp-0.1.0-beta.1-windows-x64-setup.exe'
            installer.write_bytes(b'exact installed artifact')
            runtime = {'php': '8.5.11'}
            candidate.write(root / 'runtime-manifest.json', runtime)
            original = {'status': 'passed', 'runtime': runtime, 'installer': installer.name, 'installerSha256': candidate.digest(installer), 'build': {
                'version': '0.1.0-beta.1', 'commit': 'a' * 40, 'modifiedSources': False, 'os': 'windows', 'architecture': 'x86_64',
            }}
            lifecycle = root / 'lifecycle.json'
            with patch.object(candidate, 'versions', return_value='0.1.0-beta.1'), patch.object(candidate.subprocess, 'check_output', side_effect=lambda args, **kwargs: '' if 'status' in args else 'a' * 40), patch.object(candidate, 'ROOT', root):
                candidate.write(lifecycle, original)
                candidate.record('windows-x64', output, 'installer-artifact', lifecycle, root / 'metadata/candidate-windows-x64.json')
                data = candidate.read(root / 'metadata/candidate-windows-x64.json')
                self.assertEqual(data['files'][installer.name]['sha256'], candidate.digest(installer))
                for field, value in [('modifiedSources', True), ('commit', 'b' * 40), ('version', '0.1.0-alpha.2'), ('architecture', 'aarch64')]:
                    bad = copy.deepcopy(original); bad['build'][field] = value
                    candidate.write(lifecycle, bad)
                    with self.subTest(field=field), self.assertRaises(ValueError):
                        candidate.record('windows-x64', output, 'installer-artifact', lifecycle, root / 'metadata/candidate-windows-x64.json')
                bad = copy.deepcopy(original); bad['runtime'] = {'php': '8.0.30'}
                candidate.write(lifecycle, bad)
                with self.assertRaises(ValueError): candidate.record('windows-x64', output, 'installer-artifact', lifecycle, root / 'metadata/candidate-windows-x64.json')
                candidate.write(lifecycle, original)
                installer.write_bytes(b'installer replaced after the test')
                with self.assertRaises(ValueError): candidate.record('windows-x64', output, 'installer-artifact', lifecycle, root / 'metadata/candidate-windows-x64.json')
