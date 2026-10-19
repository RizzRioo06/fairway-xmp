#!/usr/bin/env python3
"""Publish exact candidate files without rebuilding, retaining the historical unsigned entry point."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]


def run_json(*args):
    return json.loads(subprocess.check_output(args, text=True))


def sha256(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def verify_tag(tag, sha, root=ROOT):
    commit = subprocess.check_output(
        ['git', 'rev-parse', '--verify', f'refs/tags/{tag}^{{commit}}'], cwd=root, text=True,
    ).strip()
    require(commit == sha, f'{tag} does not identify the tested candidate commit')


def published_name(target, name):
    marker = '-build-source.tar.gz'
    if name.endswith(marker):
        platform = {'macos-arm64': 'macos', 'windows-x64': 'windows'}[target]
        return name[: -len(marker)] + f'-{platform}-build-source.tar.gz'
    return name


def find_draft_release(repo, tag):
    # GitHub's by-tag endpoint can return 404 for an unpublished draft.
    releases = run_json('gh', 'api', f'repos/{repo}/releases?per_page=100')
    matches = [release for release in releases if release['tag_name'] == tag]
    require(len(matches) == 1, f'Expected exactly one uploaded draft for {tag}')
    require(matches[0].get('draft') is True, f'{tag} is no longer a draft')
    return matches[0]


def verify_uploaded_assets(release, stage):
    paths = [path for path in stage.iterdir() if path.is_file()]
    assets = release.get('assets', [])
    actual = {asset['name']: asset for asset in assets}
    require(len(actual) == len(assets) and set(actual) == {path.name for path in paths},
            'Uploaded release asset set does not match the tested files')
    for path in paths:
        asset = actual[path.name]
        require(asset.get('digest') == 'sha256:' + sha256(path) and asset.get('size') == path.stat().st_size,
                f'Uploaded release asset does not match the tested file: {path.name}')


def human_acceptance(manifest, evidence_path):
    from release_versions import channel
    from release_candidate import acceptance
    require(bool(evidence_path) or channel(manifest['version']) == 'beta',
            'RC and stable publication require pre-publication exact-installer human acceptance evidence')
    if not evidence_path:
        return None
    evidence = json.loads(Path(evidence_path).read_text())
    acceptance(manifest, evidence, phase='pre-publication')
    return evidence


def main(signed=False):
    repo = os.environ['GH_REPO']
    run_id = os.environ['FAIRWAY_RUN_ID']
    tag = os.environ['FAIRWAY_TAG']
    sha = os.environ['FAIRWAY_SHA']
    require(not signed or bool(os.environ.get('FAIRWAY_NOTES')), 'Select release notes explicitly for signed releases')
    notes = Path(os.environ.get('FAIRWAY_NOTES', ROOT / 'releases/beta1-notes.md'))
    version = tag[1:] if tag.startswith('v') else tag
    prerelease = True
    if signed:
        from release_versions import channel
        prerelease = channel(version) != 'stable'
        require(tag == f'v{version}', 'Signed release tags must start with v')
    latest = 'false' if prerelease else 'true'
    require(notes.is_file(), 'Release notes are missing')
    run = run_json('gh', 'run', 'view', run_id, '--repo', repo, '--json', 'conclusion,headSha,url')
    require(run['conclusion'] == 'success', 'Candidate run did not succeed')
    require(run['headSha'] == sha, 'Candidate run is not the requested commit')
    releases = run_json('gh', 'api', f'repos/{repo}/releases?per_page=100')
    require(all(item['tag_name'] != tag for item in releases), f'{tag} already exists')
    # The release owner creates the tag before dispatching this workflow.
    # Explicit target_commitish on a historical commit can require workflow
    # permissions unavailable to GITHUB_TOKEN, even when the tag exists.
    verify_tag(tag, sha)

    with tempfile.TemporaryDirectory() as temporary:
        download = Path(temporary) / 'artifacts'
        names = [
            f'candidate-metadata-{target}' for target in ('windows-x64', 'macos-arm64', 'linux-x86_64')
        ] + [
            f'{"candidate" if signed else "unsigned-candidate"}-acceptance-{sha}',
            f'fairway-xmp-windows-x64-{version}',
            f'fairway-xmp-macos-arm64-{version}',
            f'fairway-xmp-linux-x86_64-{version}',
            f'fairway-xmp-linux-x86_64-{version}-sources',
        ]
        command = ['gh', 'run', 'download', run_id, '--repo', repo, '--dir', str(download)]
        for name in names:
            command.extend(['--name', name])
        subprocess.check_call(command)
        linux = download / f'fairway-xmp-linux-x86_64-{version}'
        for path in (download / f'fairway-xmp-linux-x86_64-{version}-sources').glob('*.tar.gz'):
            shutil.copy2(path, linux / path.name)
        directories = {
            'windows-x64': download / f'fairway-xmp-windows-x64-{version}',
            'macos-arm64': download / f'fairway-xmp-macos-arm64-{version}',
            'linux-x86_64': linux,
        }
        for target, directory in directories.items():
            subprocess.check_call([
                'python', 'scripts/release_candidate.py', 'verify-files',
                '--candidate', str(download / f'candidate-metadata-{target}' / f'candidate-{target}.json'),
                '--directory', str(directory),
            ], cwd=ROOT)
        manifest_path = download / f'{"candidate" if signed else "unsigned-candidate"}-acceptance-{sha}' / 'candidate-manifest.json'
        manifest = json.loads(manifest_path.read_text())
        require(manifest['commit'] == sha and manifest['version'] == version, 'Candidate manifest does not match this release')
        evidence = human_acceptance(manifest, os.environ.get('FAIRWAY_ACCEPTANCE')) if signed else None
        human_status = None
        if evidence is not None:
            from release_candidate import acceptance
            human_status = acceptance(manifest, evidence, phase='pre-publication')
        if signed:
            from update_feed import generate
            feed = generate(manifest, directories, repo, notes.read_text())
            for target in ('macos-arm64', 'windows-x64'):
                candidate = manifest['candidates'][target]
                subprocess.check_call([
                    'cargo', 'run', '--manifest-path', str(ROOT / 'src-tauri/Cargo.toml'),
                    '--no-default-features', '--example', 'verify_update', '--locked', '--',
                    str(ROOT / 'packaging/updater.pub'),
                    str(directories[target] / candidate['updater']['artifact']),
                    str(directories[target] / candidate['updater']['signature']), version,
                ], cwd=ROOT)
        stage = Path(temporary) / 'release'
        stage.mkdir()
        published = {}
        for target, directory in directories.items():
            candidate = json.loads((download / f'candidate-metadata-{target}' / f'candidate-{target}.json').read_text())
            for name, info in candidate['files'].items():
                source = directory / name
                destination_name = published_name(target, name)
                require(destination_name not in published, f'Duplicate release asset: {destination_name}')
                destination = stage / destination_name
                os.link(source, destination)
                published[destination_name] = {
                    'sha256': info['sha256'], 'bytes': info['bytes'], 'candidateName': name, 'target': target,
                }
                require(sha256(destination) == info['sha256'], f'Published file changed: {destination_name}')
        if signed:
            (stage / 'latest.json').write_text(json.dumps(feed, indent=2) + '\n')
        macos = directories['macos-arm64']
        for name in ('LICENSE', 'THIRD_PARTY_NOTICES.md', 'runtime-manifest.json', 'runtime-downloads.lock.json'):
            os.link(macos / name, stage / name)
        for name in ('runtime-linux-downloads.lock.json', 'runtime-sources.lock.json'):
            content = subprocess.check_output(['git', 'show', f'{sha}:{name}'], cwd=ROOT)
            (stage / name).write_bytes(content)
        shutil.copy2(manifest_path, stage / 'candidate-manifest.json')
        if evidence is not None:
            (stage / 'acceptance.json').write_text(json.dumps(evidence, indent=2) + '\n')
        files = {
            path.name: {'sha256': sha256(path), 'bytes': path.stat().st_size, **published.get(path.name, {})}
            for path in stage.iterdir() if path.is_file()
        }
        build_info = {
            'schema': 1, 'version': version, 'tag': tag, 'commit': sha, 'run': run['url'],
            'signed': manifest['signing'] if signed else False, 'prerelease': prerelease, 'latest': not prerelease,
            'prePublicationHumanAccepted': evidence is not None,
            'humanAccepted': human_status is not None and human_status['status'] == 'complete',
            'humanAcceptance': human_status, 'rebuilt': False, 'files': files,
        }
        (stage / 'BUILD-INFO.json').write_text(json.dumps(build_info, indent=2) + '\n')
        files['BUILD-INFO.json'] = {'sha256': sha256(stage / 'BUILD-INFO.json'), 'bytes': (stage / 'BUILD-INFO.json').stat().st_size}
        sums = ''.join(f"{files[name]['sha256']}  {name}\n" for name in sorted(files))
        (stage / 'SHA256SUMS.txt').write_text(sums)
        subprocess.check_call([
            'gh', 'release', 'create', tag, '--repo', repo, '--verify-tag', *(['--draft'] if signed else []),
            *(['--prerelease'] if prerelease else []), f'--latest={latest}',
            '--title', f'Fairway XMP {version}', '--notes-file', str(notes),
            *sorted(stage.iterdir(), key=lambda path: path.name),
        ])
        if signed:
            uploaded = find_draft_release(repo, tag)
            verify_uploaded_assets(uploaded, stage)
            subprocess.check_call([
                'gh', 'api', '--method', 'PATCH', f'repos/{repo}/releases/{uploaded["id"]}',
                '--silent', '-F', 'draft=false', '-F', f'prerelease={str(prerelease).lower()}', '-f', f'make_latest={latest}',
            ])
            print('Versioned release published. Promote updates separately using publish_release_feed.py.')
        print(f'Published {tag} from {run["url"]}')


if __name__ == '__main__':
    main()
