#!/usr/bin/env python3
"""Promote the latest.json of an already published versioned beta release."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
from update_feed import beta_version


def main():
    repo, tag = os.environ['GH_REPO'], os.environ['FAIRWAY_TAG']
    if repo != 'eimg/fairway-xmp' or tag != 'v' + tag.removeprefix('v'):
        raise ValueError('Unexpected repository or beta tag')
    version = tag.removeprefix('v'); beta_version(version)
    release = json.loads(subprocess.check_output(['gh', 'release', 'view', tag, '--repo', repo, '--json', 'isDraft,isPrerelease'], text=True))
    if release['isDraft'] or not release['isPrerelease']: raise ValueError('Publish the tested beta before promoting its feed')
    with tempfile.TemporaryDirectory() as temporary:
        root = Path(temporary)
        subprocess.check_call(['gh', 'release', 'download', tag, '--repo', repo, '--pattern', 'latest.json', '--dir', str(root)])
        feed = json.loads((root / 'latest.json').read_text())
        if feed['version'] != version or set(feed['platforms']) != {'darwin-aarch64', 'windows-x86_64'}:
            raise ValueError('Published update metadata does not match the requested beta')
        for platform in feed['platforms'].values():
            if not platform['url'].startswith(f'https://github.com/{repo}/releases/download/{tag}/') or not platform['signature']:
                raise ValueError('Unexpected updater asset URL or missing signature')
        existing = subprocess.run(['gh', 'release', 'view', 'beta-feed', '--repo', repo], capture_output=True)
        if existing.returncode == 0:
            previous = root / 'previous'; previous.mkdir()
            subprocess.check_call(['gh', 'release', 'download', 'beta-feed', '--repo', repo, '--pattern', 'latest.json', '--dir', str(previous)])
            old = json.loads((previous / 'latest.json').read_text())
            if beta_version(old['version']) > beta_version(version): raise ValueError('Refusing to move the beta feed backwards')
            subprocess.check_call(['gh', 'release', 'upload', 'beta-feed', str(root / 'latest.json'), '--repo', repo, '--clobber'])
        else:
            subprocess.check_call(['gh', 'release', 'create', 'beta-feed', '--repo', repo, '--prerelease', '--latest=false', '--title', 'Beta update feed', '--notes', 'Metadata for Fairway XMP in-app beta updates. Download installers from the numbered beta releases.', str(root / 'latest.json')])
    print(f'Beta update feed now points to {tag}.')


if __name__ == '__main__': main()
