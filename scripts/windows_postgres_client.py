"""Record the libpq dependency closure from the checksum-verified Windows PHP ZIP.

No binaries are executed here. The manifest is embedded into the application at
build time; it is not a user-editable runtime trust override.
"""
import hashlib
import json
from pathlib import Path


def client_manifest(archive, source):
    if len(source.get('sha256', '')) != 64:
        raise ValueError('A locked Windows PHP source checksum is required')
    files = {}
    for name in archive.namelist():
        if '/' in name or '\\' in name or not name.lower().endswith('.dll'):
            continue
        if name != Path(name).name or ':' in name or name.lower() in {n.lower() for n in files}:
            raise ValueError('Unsafe or duplicate Windows client DLL name')
        info = archive.getinfo(name)
        if info.file_size <= 0 or info.file_size > 128 * 1024 * 1024 or len(files) >= 128:
            raise ValueError('Oversized Windows client DLL set')
        with archive.open(name) as stream:
            sha = hashlib.file_digest(stream, 'sha256').hexdigest()
        files[name] = {'sha256': sha, 'bytes': info.file_size}
    if 'libpq.dll' not in files:
        raise ValueError('Pinned Windows PHP archive has no libpq.dll')
    return {'schemaVersion': 1, 'platform': 'windows-x64', 'sourceSha256': source['sha256'], 'files': files}


def write_client_manifest(archive_path, source, target):
    import zipfile
    with Path(archive_path).open('rb') as stream:
        if hashlib.file_digest(stream, 'sha256').hexdigest() != source['sha256']:
            raise ValueError('Windows PHP archive checksum mismatch')
    with zipfile.ZipFile(archive_path) as archive:
        manifest = client_manifest(archive, source)
    (Path(target) / 'windows-postgresql-client.json').write_text(json.dumps(manifest, indent=2, sort_keys=True) + '\n')
