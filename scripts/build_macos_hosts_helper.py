#!/usr/bin/env python3
"""Build the optional signed-hosts prototype payload; never registers a service.

Run before the desktop build. Copy the resulting Library/ into Contents/ before
sign_macos.py, which seals the helper and its containing app with the same team.
"""
import argparse
import os
from pathlib import Path
import plistlib
import re
import shlex
import subprocess

LABEL = 'dev.fairway.environment.hosts-helper'
ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--team-id', required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if not re.fullmatch(r'[A-Z0-9]{10}', args.team_id):
        parser.error('Expected a ten-character Apple Developer team ID')
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    env = {**os.environ, 'MACOSX_DEPLOYMENT_TARGET': '15.0'}
    command = ['cargo', 'rustc', '--manifest-path', str(ROOT / 'src-tauri/Cargo.toml'),
               '--release', '--lib', '--no-default-features', '--features', 'native-hosts-writer',
               '--locked', '--offline', '--', '--print', 'native-static-libs']
    result = subprocess.run(command, cwd=ROOT, env=env, text=True, capture_output=True)
    print(result.stderr, end='', flush=True)
    result.check_returncode()
    flags = re.findall(r'native-static-libs: (.+)', result.stderr)
    if not flags:
        raise RuntimeError('Rust did not report its native link libraries')
    constants = output / 'HelperIdentity.swift'
    # Native XPC checks audit identities against Developer ID and the exact helper
    # identifier in both directions. Never trust a PID-only or path-only check.
    requirement = (f'anchor apple generic and identifier "{LABEL}" and '
                   f'certificate leaf[subject.OU] = "{args.team_id}" and '
                   'certificate 1[field.1.2.840.113635.100.6.2.6] exists and '
                   'certificate leaf[field.1.2.840.113635.100.6.1.13] exists')
    import json
    constants.write_text(f'let helperLabel = {json.dumps(LABEL)}\n'
                         f'let helperRequirement = {json.dumps(requirement)}\n')
    tools = output / 'Library/HelperTools'
    tools.mkdir(parents=True, exist_ok=True)
    # Swift permits top-level executable statements in main.swift.
    source = output / 'main.swift'
    source.write_bytes((ROOT / 'packaging/macos/HostsHelper.swift').read_bytes())
    subprocess.run(['xcrun', 'swiftc', '-O', '-module-cache-path', str(output / 'module-cache'),
                    '-target', 'arm64-apple-macos15.0',
                    str(source), str(constants), str(ROOT / 'src-tauri/target/release/libfairway_core.a'),
                    *shlex.split(flags[-1]), '-o', str(tools / LABEL)], check=True, env=env)
    # Negative smoke checks stop before any service connection or hosts write.
    for argument in (['--client', '--server'] if os.geteuid() != 0 else ['--client']):
        probe = subprocess.run([str(tools / LABEL), argument], input=b'', capture_output=True, timeout=5)
        if probe.returncode != 1 or not probe.stderr:
            raise RuntimeError(f'Helper guard failed for {argument}')
    plists = output / 'Library/LaunchDaemons'
    plists.mkdir(parents=True, exist_ok=True)
    (plists / f'{LABEL}.plist').write_bytes(plistlib.dumps({
        'Label': LABEL,
        'BundleProgram': f'Contents/Library/HelperTools/{LABEL}',
        'ProgramArguments': [LABEL, '--server'],
        'MachServices': {LABEL: True},
        'ProcessType': 'Background',
        'AssociatedBundleIdentifiers': ['dev.fairway.environment'],
    }))
    print(f'Payload prepared in {output / "Library"}; sign the complete app before testing.')


if __name__ == '__main__':
    main()
