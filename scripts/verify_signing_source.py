#!/usr/bin/env python3
"""Allow signing only exact commits in the fetched main-branch history."""
import argparse
from pathlib import Path
import re
import subprocess


def verify(root, expected=None, source='HEAD'):
    def git(*args):
        return subprocess.check_output(['git', *args], cwd=root, text=True).strip()

    commit = git('rev-parse', '--verify', '--end-of-options', source + '^{commit}')
    if expected is not None and (not re.fullmatch(r'[0-9a-f]{40}', expected) or commit != expected):
        raise ValueError('Signing checkout differs from the exact build commit')
    # origin/main is populated by checkout with fetch-depth: 0; never accept a
    # caller-provided replacement for the trusted branch.
    git('rev-parse', '--verify', 'refs/remotes/origin/main^{commit}')
    if subprocess.run(['git', 'merge-base', '--is-ancestor', commit, 'refs/remotes/origin/main'], cwd=root).returncode != 0:
        raise ValueError('Signing requires a commit already merged into origin/main')
    return commit


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--expected', help='Exact commit reported by the build job')
    parser.add_argument('--source', default='HEAD', help='Resolve and validate a comparison ref before checking it out')
    args = parser.parse_args()
    print(verify(Path(__file__).resolve().parents[1], args.expected, args.source))


if __name__ == '__main__':
    main()
