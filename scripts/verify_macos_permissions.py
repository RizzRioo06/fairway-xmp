#!/usr/bin/env python3
"""Verify the actual Mac app's native permission features and fixed helper payload."""
import argparse
import json
import os
from pathlib import Path
import plistlib
import re
import subprocess

LABEL = 'dev.fairway.environment.hosts-helper'


def verify(app, build, team_id=None):
    if build.get('os') != 'macos' or any(
            build.get('features', {}).get(feature) is not True
            for feature in ('nativeMacosHosts', 'nativeMacosTrust')):
        raise ValueError('The packaged app must enable both native Mac permission features')
    library = app / 'Contents/Library'
    helper = library / 'HelperTools' / LABEL
    plist = library / 'LaunchDaemons' / f'{LABEL}.plist'
    for path in (library, helper.parent, plist.parent, helper, plist):
        if path.is_symlink():
            raise ValueError('The address helper payload must not contain symlinks')
    if not helper.is_file() or not os.access(helper, os.X_OK) or not plist.is_file():
        raise ValueError('The packaged app is missing its executable address helper or plist')
    info = plistlib.loads(plist.read_bytes())
    expected = {
        'Label': LABEL,
        'BundleProgram': f'Contents/Library/HelperTools/{LABEL}',
        'ProgramArguments': [LABEL, '--server'],
        'MachServices': {LABEL: True},
        'ProcessType': 'Background',
        'AssociatedBundleIdentifiers': ['dev.fairway.environment'],
    }
    if info != expected:
        raise ValueError('The address helper launch configuration differs from its fixed capability')
    if team_id is not None:
        if not re.fullmatch(r'[A-Z0-9]{10}', team_id):
            raise ValueError('Expected a ten-character Apple Developer team ID')
        requirement = (f'anchor apple generic and identifier "{LABEL}" and '
                       f'certificate leaf[subject.OU] = "{team_id}" and '
                       'certificate 1[field.1.2.840.113635.100.6.2.6] exists and '
                       'certificate leaf[field.1.2.840.113635.100.6.1.13] exists')
        subprocess.run(['codesign', '--verify', '--strict', '-R=' + requirement, str(helper)], check=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('app', type=Path)
    parser.add_argument('--team-id', help='Also require the helper Developer ID signing identity')
    args = parser.parse_args()
    build = json.loads(subprocess.check_output(
        [str(args.app.resolve() / 'Contents/MacOS/fairway'), '--build-info'], text=True))
    verify(args.app, build, args.team_id)
    print('Verified packaged native Mac permission features and address helper payload.')


if __name__ == '__main__':
    main()
