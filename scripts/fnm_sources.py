"""Version-matched fnm and dependency sources, plus an offline rebuild recipe."""
import hashlib
import json
from pathlib import Path
import tarfile
import tomllib


def source_plan(archive, entry, version):
    plan = [{'name': 'fnm', **entry, 'file': 'fnm.tar.gz'}]
    with tarfile.open(archive) as bundle:
        lock = tomllib.loads(bundle.extractfile(f'fnm-{version}/Cargo.lock').read().decode())
    for package in lock['package']:
        source = package.get('source')
        if not source:
            continue
        if source != 'registry+https://github.com/rust-lang/crates.io-index':
            raise RuntimeError(f'Unreviewed fnm dependency source: {source}')
        name, release = package['name'], package['version']
        plan.append({'name': f'fnm-crate-{name}-{release}',
                     'url': f'https://static.crates.io/crates/{name}/{name}-{release}.crate',
                     'sha256': package['checksum'], 'file': f'fnm-{name}-{release}.crate'})
    return plan


def rebuild(sources, destination):
    """Expand the accompanying source archive into an offline Cargo project."""
    destination.mkdir(parents=True, exist_ok=False)
    def extract(archive, target):
        with tarfile.open(archive) as bundle:
            bundle.extractall(target, filter='data')
    extract(sources / 'fnm.tar.gz', destination)
    project, = destination.glob('fnm-*')
    lock = tomllib.loads((project / 'Cargo.lock').read_text())
    vendor = project / 'vendor'
    for package in lock['package']:
        if not package.get('source'):
            continue
        name, release = package['name'], package['version']
        archive = sources / f'fnm-{name}-{release}.crate'
        with archive.open('rb') as stream:
            checksum = hashlib.file_digest(stream, 'sha256').hexdigest()
        if checksum != package['checksum']:
            raise RuntimeError(f'Source checksum mismatch: {name}')
        extract(archive, vendor)
        root = vendor / f'{name}-{release}'
        files = {path.relative_to(root).as_posix(): hashlib.sha256(path.read_bytes()).hexdigest()
                 for path in root.rglob('*') if path.is_file() and path.name != '.cargo-checksum.json'}
        (root / '.cargo-checksum.json').write_text(json.dumps({'package': package['checksum'], 'files': files}))
    config = project / '.cargo'
    config.mkdir(exist_ok=True)
    (config / 'config.toml').write_text('[source.crates-io]\nreplace-with = "vendored-sources"\n[source.vendored-sources]\ndirectory = "vendor"\n')
    print(f'Run in {project}: cargo build --release --locked --offline')


if __name__ == '__main__':
    import argparse
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('sources', type=Path)
    parser.add_argument('destination', type=Path)
    args = parser.parse_args()
    rebuild(args.sources, args.destination)
