#!/usr/bin/env python3
"""Collect version-matched runtime sources and packaging recipes for a release."""
from __future__ import annotations

import argparse
import concurrent.futures
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess
import tarfile
import urllib.parse

from prepare_runtime import CACHE, LOCK_PATH, MANIFEST, ROOT, download, read_json


def source_input(name, url, sha256=None):
    path = urllib.parse.urlsplit(url).path
    suffix = next((suffix for suffix in ('.tar.gz', '.tar.xz', '.tar.bz2', '.tgz', '.zip') if path.endswith(suffix)), '.tar.gz')
    return {'name': name, 'url': url, 'sha256': sha256, 'file': name.replace('@', '-') + suffix}


def collect_plan(stage):
    lock = json.loads(LOCK_PATH.read_text())
    plan = []
    recipes = stage / 'homebrew'
    recipes.mkdir(parents=True, exist_ok=True)
    for key, entry in sorted(lock.items()):
        if not key.startswith('brew-'):
            continue
        name = key.removeprefix('brew-').removesuffix('-arm64-sequoia')
        archive = CACHE / hashlib.sha256(entry['url'].encode()).hexdigest()
        if not archive.exists() or hashlib.file_digest(archive.open('rb'), 'sha256').hexdigest() != entry['sha256']:
            raise RuntimeError(f'Missing verified bottle for {name}; stage the locked macOS runtime first')
        with tarfile.open(archive) as bottle:
            formula, = [member for member in bottle.getmembers() if '/.brew/' in member.name and member.name.endswith('.rb')]
            text = bottle.extractfile(formula).read().decode()
            (recipes / f'{name}.rb').write_text(text)
            metadata = [member for member in bottle.getmembers() if member.name.endswith(('/INSTALL_RECEIPT.json', '/sbom.spdx.json'))]
            for member in metadata:
                (recipes / f'{name}.{Path(member.name).name}').write_bytes(bottle.extractfile(member).read())
        url = re.search(r'^  url "([^"]+)"', text, re.M)
        sha = re.search(r'^  sha256 "([0-9a-f]{64})"', text, re.M)
        revision = re.search(r'^\s+revision: "([0-9a-f]{40})"', text, re.M)
        if url and url.group(1).endswith('.git') and revision:
            item = source_input(name, url.group(1).removesuffix('.git') + '/+archive/' + revision.group(1) + '.tar.gz')
            item['revision'] = revision.group(1)
        elif url and sha:
            item = source_input(name, url.group(1), sha.group(1))
        else:
            raise RuntimeError(f'Review the source recipe for {name}: cannot identify stable source')
        item['binary'] = {'key': key, **entry}
        item['mirrors'] = re.findall(r'^  mirror "([^"]+)"', text.split('  license', 1)[0], re.M)
        plan.append(item)
        tap = None
        for index, block in enumerate(re.findall(r'^  (?:patch[^\n]*|resource "[^"]+") do\n(.*?)^  end', text, re.M | re.S)):
            patch_url = re.search(r'^\s+url "([^"]+)"', block, re.M)
            patch_sha = re.search(r'^\s+sha256 "([0-9a-f]{64})"', block, re.M)
            local_patch = re.search(r'^\s+file "([^"]+)"', block, re.M)
            if local_patch:
                if tap is None:
                    tap = read_json(f'https://formulae.brew.sh/api/formula/{urllib.parse.quote(name, safe="")}.json')
                    if tap['bottle']['stable']['files']['arm64_sequoia']['sha256'] != entry['sha256']:
                        raise RuntimeError(f'Homebrew recipe changed for {name}; locate its historical patch commit')
                extra = source_input(f'{name}-resource-{index}', f'https://raw.githubusercontent.com/Homebrew/homebrew-core/{tap["tap_git_head"]}/{local_patch.group(1)}')
                extra['file'] = f'{name.replace("@", "-")}-patch-{index}.patch'
                plan.append(extra)
                continue
            if not patch_url or not patch_sha:
                raise RuntimeError(f'Review external source/patch {index} for {name}')
            extra = source_input(f'{name}-resource-{index}', patch_url.group(1), patch_sha.group(1))
            if 'patch' in extra['url']:
                extra['file'] = f'{name.replace("@", "-")}-patch-{index}.patch'
            plan.append(extra)
    # The release distribution includes readable PHP/JavaScript and vendor files;
    # the Git tag also supplies the preferred development sources/build inputs.
    pma = lock['phpmyadmin']
    plan.append(source_input('phpmyadmin-distribution', pma['url'], pma['sha256']))
    plan.append(source_input('phpmyadmin-development', f'https://codeload.github.com/phpmyadmin/phpmyadmin/tar.gz/refs/tags/RELEASE_{MANIFEST["phpmyadmin"].replace(".", "_")}'))
    plan.append(source_input('composer', f'https://codeload.github.com/composer/composer/tar.gz/refs/tags/{MANIFEST["composer"]}'))
    certifi_version = re.search(r'certifi-([\d.]+)-', lock['ca-certificates']['url']).group(1)
    certifi = read_json(f'https://pypi.org/pypi/certifi/{certifi_version}/json')
    sdist, = [file for file in certifi['urls'] if file['packagetype'] == 'sdist']
    plan.append(source_input('ca-certificates', sdist['url'], sdist['digests']['sha256']))
    source_lock = ROOT / 'runtime-sources.lock.json'
    if source_lock.exists():
        pinned = {item['name']: item for item in json.loads(source_lock.read_text())}
        for item in plan:
            previous = pinned.get(item['name'])
            if previous:
                if item['url'] != previous['url']:
                    raise RuntimeError(f'Source URL changed for {item["name"]}; review the source lock')
                item['sha256'] = previous['sha256']
    return plan


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--plan-only', action='store_true', help='Write recipes and download plan without fetching source archives')
    args = parser.parse_args()
    version = json.loads((ROOT / 'package.json').read_text())['version']
    stage = ROOT / '.runtime-work' / f'sources-{version}'
    stage.mkdir(parents=True, exist_ok=True)
    plan = collect_plan(stage)
    plan_path = stage / 'source-inputs.json'
    plan_path.write_text(json.dumps(plan, indent=2) + '\n')
    if args.plan_only:
        print(f'{len(plan)} source inputs; plan at {plan_path}')
        return
    sources = stage / 'upstream'
    sources.mkdir(exist_ok=True)

    def fetch(item):
        failure = None
        for url in [item['url'], *item.get('mirrors', [])]:
            try:
                archive, sha = download(url, item['sha256'])
                break
            except OSError as error:
                failure = error
        else:
            raise RuntimeError(f'Cannot obtain source for {item["name"]}') from failure
        destination = sources / item['file']
        shutil.copyfile(archive, destination)
        print(f'Verified {item["name"]}: {destination.stat().st_size / 1024**2:.1f} MiB', flush=True)
        return {**item, 'sha256': sha}

    with concurrent.futures.ThreadPoolExecutor(max_workers=6) as pool:
        verified = list(pool.map(fetch, plan))
    plan_path.write_text(json.dumps(verified, indent=2) + '\n')
    (ROOT / 'runtime-sources.lock.json').write_text(json.dumps(verified, indent=2) + '\n')
    commit = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip()
    subprocess.run(['git', 'archive', '--format=tar.gz', f'--output={stage / "fairway-xmp-source.tar.gz"}', commit], cwd=ROOT, check=True)
    shutil.copyfile(LOCK_PATH, stage / LOCK_PATH.name)
    shutil.copyfile(ROOT / 'runtime-manifest.json', stage / 'runtime-manifest.json')
    (stage / 'README.md').write_text(f'''# Fairway XMP {version}: bundled-tool source

This archive accompanies the Windows x64 and macOS ARM64 installers.
Fairway XMP source commit: {commit}.

`upstream/` contains source distributions for every bundled Homebrew component,
MySQL (also used by the official Windows distribution), phpMyAdmin, Composer,
and the certificate bundle. `source-inputs.json` maps each archive to its URL,
SHA-256, and matching macOS bottle when applicable.

`homebrew/` contains the exact recipes and available build metadata from the pinned
bottles. Recipes include inline patches; externally downloaded patches and
resource sources are in `upstream/`. Use the recorded recipes to build native
components with Homebrew and their listed build prerequisites. These source
archives retain the components' own licenses.

`fairway-xmp-source.tar.gz` contains the application, configuration templates,
and scripts that stage, relocate, and package the runtimes. Extract it and see
CONTRIBUTING.md for platform build instructions. `scripts/prepare_runtime.py`
describes the only Fairway changes to the native binaries: staging and macOS
library load-path relocation, followed by ad-hoc signing. Windows uses the
unmodified upstream executables and DLLs. No MySQL or PHP source patches are
applied by Fairway; upstream Homebrew patches are recorded in the recipes.

macOS libraries are dynamically linked under the app's `Contents/Resources/runtime/lib`.
You may replace a library with a compatible build. Apply the same relative load
paths and ad-hoc signing described by the relocation script before running it.
''')
    assets = ROOT / 'artifacts' / f'alpha-{version}'
    assets.mkdir(parents=True, exist_ok=True)
    output = assets / f'Fairway-XMP-{version}-bundled-tool-sources.tar.gz'
    with tarfile.open(output, 'w:gz', compresslevel=1) as bundle:
        bundle.add(stage, arcname=f'Fairway-XMP-{version}-sources')
    print(f'Source archive ready: {output} ({output.stat().st_size / 1024**2:.1f} MiB)', flush=True)


if __name__ == '__main__':
    main()
