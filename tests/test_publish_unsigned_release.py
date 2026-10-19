import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'scripts'))
import release_candidate


spec = importlib.util.spec_from_file_location(
    'publish_unsigned_release', Path(__file__).resolve().parents[1] / 'scripts/publish_unsigned_release.py',
)
publisher = importlib.util.module_from_spec(spec)
spec.loader.exec_module(publisher)


class ReleaseTagTests(unittest.TestCase):
    def test_rc_and_stable_require_completed_evidence_for_exact_installer_hashes(self):
        for version in ['0.1.0-rc.1', '0.1.0']:
            manifest = {'version': version, 'commit': 'a' * 40, 'candidates': {target: {
                'installer': 'installer', 'files': {'installer': {'sha256': 'b' * 64}}}
                for target in release_candidate.TARGETS}}
            with self.subTest(version=version), tempfile.TemporaryDirectory() as temporary:
                with self.assertRaisesRegex(RuntimeError, 'human acceptance'):
                    publisher.human_acceptance(manifest, None)
                evidence = {'version': version, 'commit': manifest['commit'], 'platforms': {target: {
                    'installerSha256': 'b' * 64, 'tester': 'Test fixture', 'os': 'Test OS', 'testedAt': '2026-10-17',
                    'checks': {check: True for check in release_candidate.MANUAL_CHECKS[target]}}
                    for target in release_candidate.TARGETS}}
                path = Path(temporary) / 'acceptance.json'
                path.write_text(json.dumps(evidence))
                self.assertEqual(publisher.human_acceptance(manifest, path), evidence)
                for target in ('macos-arm64', 'windows-x64'):
                    evidence['platforms'][target]['checks']['in-app-update-and-restart'] = False
                path.write_text(json.dumps(evidence))
                self.assertEqual(publisher.human_acceptance(manifest, path), evidence)
                with self.assertRaises(ValueError): release_candidate.acceptance(manifest, evidence)
                evidence['platforms']['windows-x64']['checks']['mail-catcher-and-inbox'] = False
                path.write_text(json.dumps(evidence))
                with self.assertRaisesRegex(ValueError, 'Unfinished human checks'):
                    publisher.human_acceptance(manifest, path)
                evidence['platforms']['windows-x64']['installerSha256'] = 'c' * 64
                path.write_text(json.dumps(evidence))
                with self.assertRaisesRegex(ValueError, 'exact-installer'):
                    publisher.human_acceptance(manifest, path)

    def test_signed_publisher_uses_prerelease_flags_for_rc_and_latest_for_stable(self):
        for version, prerelease in [('0.1.0-rc.1', True), ('0.1.0', False)]:
            with self.subTest(version=version), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary); fixture = root / 'fixture'; fixture.mkdir()
                sha = 'a' * 40
                manifest = {'version': version, 'commit': sha, 'signing': {'macos-arm64': 'apple-notarized', 'windows-x64': 'updater-only', 'linux-x86_64': 'unsigned'}, 'candidates': {}}
                for target, suffix in [('macos-arm64', 'macos-arm64.app.tar.gz'), ('windows-x64', 'windows-x64-setup.exe'), ('linux-x86_64', 'linux-x86_64.run')]:
                    directory = fixture / f'fairway-xmp-{target}-{version}'; directory.mkdir()
                    installer = f'fairway-xmp-{version}-{suffix}'
                    (directory / installer).write_bytes(b'tested bytes')
                    updater = {'artifact': installer, 'signature': installer + '.sig'}
                    if target != 'linux-x86_64': (directory / updater['signature']).write_text('fixture signature')
                    files = {p.name: {'sha256': publisher.sha256(p), 'bytes': p.stat().st_size} for p in directory.iterdir()}
                    data = {'version': version, 'commit': sha, 'installer': installer, 'files': files, 'updater': updater, 'lifecycle': {'updater': {'status': 'passed'}}}
                    manifest['candidates'][target] = data
                    metadata = fixture / f'candidate-metadata-{target}'; metadata.mkdir()
                    (metadata / f'candidate-{target}.json').write_text(json.dumps(data))
                (fixture / f'fairway-xmp-linux-x86_64-{version}-sources').mkdir()
                macos = fixture / f'fairway-xmp-macos-arm64-{version}'
                for name in ['LICENSE', 'THIRD_PARTY_NOTICES.md', 'runtime-manifest.json', 'runtime-downloads.lock.json']:
                    (macos / name).write_text('{}')
                metadata = fixture / f'candidate-acceptance-{sha}'; metadata.mkdir()
                (metadata / 'candidate-manifest.json').write_text(json.dumps(manifest))
                evidence = {'version': version, 'commit': sha, 'platforms': {target: {
                    'installerSha256': data['files'][data['installer']]['sha256'], 'tester': 'Test fixture', 'os': 'Test OS', 'testedAt': '2026-10-17',
                    'checks': {check: True for check in release_candidate.MANUAL_CHECKS[target]}}
                    for target, data in manifest['candidates'].items()}}
                for target in ('macos-arm64', 'windows-x64'):
                    evidence['platforms'][target]['checks']['in-app-update-and-restart'] = False
                accepted = root / 'acceptance.json'; accepted.write_text(json.dumps(evidence))
                notes = root / 'notes.md'; notes.write_text('Reviewed release notes')
                uploaded = {}; commands = []; build = {}; fail_signature = False
                def run_json(*args):
                    if args[:3] == ('gh', 'run', 'view'): return {'conclusion': 'success', 'headSha': sha, 'url': 'https://example.invalid/run'}
                    return [uploaded] if uploaded else []
                def execute(args, **kwargs):
                    commands.append(args)
                    if args[0] == 'cargo' and fail_signature:
                        raise subprocess.CalledProcessError(1, args)
                    if args[:3] == ['gh', 'run', 'download']:
                        shutil.copytree(fixture, Path(args[args.index('--dir') + 1]), dirs_exist_ok=True)
                    if args[:3] == ['gh', 'release', 'create']:
                        paths = [arg for arg in args if isinstance(arg, Path)]
                        build.update(json.loads(next(p for p in paths if p.name == 'BUILD-INFO.json').read_text()))
                        self.assertTrue(any(p.name == 'acceptance.json' for p in paths))
                        uploaded.update({'id': 42, 'tag_name': 'v' + version, 'draft': True, 'assets': [
                            {'name': p.name, 'digest': 'sha256:' + publisher.sha256(p), 'size': p.stat().st_size} for p in paths]})
                with mock.patch.dict(os.environ, {'GH_REPO': 'eimg/fairway-xmp', 'FAIRWAY_RUN_ID': '123', 'FAIRWAY_TAG': 'v' + version, 'FAIRWAY_SHA': sha, 'FAIRWAY_NOTES': str(notes), 'FAIRWAY_ACCEPTANCE': str(accepted)}), \
                     mock.patch.object(publisher, 'run_json', side_effect=run_json), mock.patch.object(publisher, 'verify_tag'), \
                     mock.patch.object(publisher.subprocess, 'check_call', side_effect=execute), mock.patch.object(publisher.subprocess, 'check_output', return_value=b'{}'), mock.patch('builtins.print'):
                    publisher.main(signed=True)
                create = next(args for args in commands if args[:3] == ['gh', 'release', 'create'])
                self.assertEqual('--prerelease' in create, prerelease)
                self.assertIn('--latest=' + str(not prerelease).lower(), create)
                patch = next(args for args in commands if args[:3] == ['gh', 'api', '--method'])
                self.assertIn('prerelease=' + str(prerelease).lower(), patch)
                self.assertIn('make_latest=' + str(not prerelease).lower(), patch)
                self.assertEqual(build['prerelease'], prerelease)
                self.assertTrue(build['prePublicationHumanAccepted'])
                self.assertFalse(build['humanAccepted'])
                self.assertEqual(build['humanAcceptance']['status'], 'awaiting-post-publication')
                self.assertEqual(set(build['humanAcceptance']['pendingChecks']), {'macos-arm64', 'windows-x64'})
                self.assertEqual(len([args for args in commands if args[0] == 'cargo']), 2)
                # Deferring the human check must not bypass the automated drill or signature verifier.
                for failure in ('native-drill', 'signature'):
                    uploaded.clear(); commands.clear()
                    fail_signature = failure == 'signature'
                    if failure == 'native-drill':
                        manifest['candidates']['windows-x64']['lifecycle']['updater']['status'] = 'failed'
                    else:
                        manifest['candidates']['windows-x64']['lifecycle']['updater']['status'] = 'passed'
                    (metadata / 'candidate-manifest.json').write_text(json.dumps(manifest))
                    with self.subTest(failure=failure), mock.patch.dict(os.environ, {'GH_REPO': 'eimg/fairway-xmp', 'FAIRWAY_RUN_ID': '123', 'FAIRWAY_TAG': 'v' + version, 'FAIRWAY_SHA': sha, 'FAIRWAY_NOTES': str(notes), 'FAIRWAY_ACCEPTANCE': str(accepted)}), \
                         mock.patch.object(publisher, 'run_json', side_effect=run_json), mock.patch.object(publisher, 'verify_tag'), \
                         mock.patch.object(publisher.subprocess, 'check_call', side_effect=execute), mock.patch.object(publisher.subprocess, 'check_output', return_value=b'{}'), \
                         self.assertRaises((ValueError, subprocess.CalledProcessError)):
                        publisher.main(signed=True)
                    self.assertFalse(any(args[:3] == ['gh', 'release', 'create'] for args in commands))

    def test_uploaded_draft_is_found_without_the_published_tag_endpoint(self):
        release = {'id': 42, 'tag_name': 'v0.1.0-beta.3', 'draft': True}
        with mock.patch.object(publisher, 'run_json', return_value=[release]) as api:
            self.assertEqual(publisher.find_draft_release('eimg/fairway-xmp', release['tag_name']), release)
            api.assert_called_once_with('gh', 'api', 'repos/eimg/fairway-xmp/releases?per_page=100')
        for releases in ([], [release, release], [{**release, 'draft': False}]):
            with self.subTest(releases=releases), mock.patch.object(publisher, 'run_json', return_value=releases):
                with self.assertRaises(RuntimeError):
                    publisher.find_draft_release('eimg/fairway-xmp', release['tag_name'])

    def test_uploaded_assets_require_the_exact_set_and_content(self):
        with tempfile.TemporaryDirectory() as temporary:
            stage = Path(temporary)
            artifact = stage / 'installer.pkg'
            artifact.write_bytes(b'tested installer')
            asset = {'name': artifact.name, 'size': artifact.stat().st_size, 'digest': 'sha256:' + publisher.sha256(artifact)}
            publisher.verify_uploaded_assets({'assets': [asset]}, stage)
            for assets in ([], [asset, asset], [{**asset, 'digest': 'sha256:' + '0' * 64}],
                           [{**asset, 'size': 0}], [asset, {**asset, 'name': 'unexpected.exe'}]):
                with self.subTest(assets=assets), self.assertRaises(RuntimeError):
                    publisher.verify_uploaded_assets({'assets': assets}, stage)

    def test_release_tag_must_resolve_to_the_tested_commit(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)

            def git(*args):
                return subprocess.check_output([
                    'git', '-c', 'core.hooksPath=/dev/null', '-c', 'commit.gpgsign=false',
                    '-c', 'user.name=Release test', '-c', 'user.email=release-test@example.invalid',
                    *args,
                ], cwd=root, text=True).strip()

            git('init', '--quiet')
            git('commit', '--quiet', '--allow-empty', '-m', 'Tested candidate')
            candidate = git('rev-parse', 'HEAD')
            git('tag', 'v0.1.0-beta.2')
            publisher.verify_tag('v0.1.0-beta.2', candidate, root)
            git('commit', '--quiet', '--allow-empty', '-m', 'Later documentation')
            later = git('rev-parse', 'HEAD')
            with self.assertRaisesRegex(RuntimeError, 'does not identify the tested candidate'):
                publisher.verify_tag('v0.1.0-beta.2', later, root)
            git('tag', '-a', 'annotated-beta', '-m', 'Candidate', candidate)
            publisher.verify_tag('annotated-beta', candidate, root)
