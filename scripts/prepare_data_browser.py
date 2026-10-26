"""Build the pinned standalone browser for the Linux terminal package."""
from __future__ import annotations
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
from prepare_runtime import ROOT, MANIFEST, copy, lock_download, safe_extract


def replace_once(text, before, after):
    if text.count(before) != 1:
        raise RuntimeError(f'Data browser production overlay no longer matches: {before[:80]}')
    return text.replace(before, after, 1)


def production_overlay(project):
    """A small, checked overlay keeps upstream development files unmodified in sources."""
    index = project / 'server/index.ts'
    text = index.read_text()
    text = "import { allowedRequest, browserUrl, serveStatic } from './fairway-http.mjs';\n" + text
    text = replace_once(text, 'const port = 3001;', 'const port = Number(process.env.FAIRWAY_DATA_BROWSER_PORT);')
    text = replace_once(text, "const cookieName = 'data_browser_session';", "const cookieName = `data_browser_session_${port}`;")
    text = replace_once(text, '    const url = new URL(req.url', '    if (!allowedRequest(req, res, port)) return;\n    const url = new URL(req.url')
    text = replace_once(text, "    send(res, 404, { error: 'Not found' });", "    if (await serveStatic(req, res, url)) return;\n    send(res, 404, { error: 'Not found' });")
    text = replace_once(text, 'server.listen(port, host, () => {', "server.on('error', error => {\n  console.error(`Data browser could not listen on ${host}:${port}: ${error.message}`);\n  process.exit(1);\n});\nserver.listen(port, host, () => {")
    text = replace_once(text, 'API listening on http://${host}:${port}', 'Data browser: ${browserUrl(port)} (Ctrl+C to stop)')
    index.write_text(text)
    # A headless machine may have no file manager. Reveal must not crash the server.
    operations = project / 'server/operations.ts'
    operations.write_text(replace_once(operations.read_text(),
        "spawn(command, [dir], { detached: true, stdio: 'ignore' }).unref();",
        "spawn(command, [dir], { detached: true, stdio: 'ignore' }).on('error', () => {}).unref();"))
    api = project / 'src/api.ts'
    api_text = replace_once(api.read_text(), r'  if (/127\.0\.0\.1:3001/.test(detail)) return new Error(detail);' + '\n', '')
    api.write_text(api_text.replace('127.0.0.1:3001', '${window.location.host}').replace("'fetch failed: network error to ${window.location.host}'", '`fetch failed: network error to ${window.location.host}`'))
    copy(ROOT / 'packaging/linux/data-browser-http.mjs', project / 'server/fairway-http.mjs')


def prepare(target, sources, lock, refresh=False):
    revision = MANIFEST['dataBrowser']
    archive = lock_download('data-browser-source', f'https://codeload.github.com/eimg/data-browser/tar.gz/{revision}', refresh=refresh, lock=lock)
    work = ROOT / '.runtime-work/data-browser-build'
    if work.exists(): shutil.rmtree(work)
    safe_extract(archive, work, 'tar')
    project, = work.iterdir()
    copy(archive, sources / 'data-browser-source.tar.gz')
    production_overlay(project)
    subprocess.run(['npm', 'ci', '--ignore-scripts', '--no-audit', '--no-fund'], cwd=project, check=True)
    # Vite builds static files only; the shipped server never starts a development server.
    subprocess.run(['node', 'node_modules/vite/bin/vite.js', 'build'], cwd=project, check=True)
    output = target / 'data-browser'
    if output.exists(): shutil.rmtree(output)
    output.mkdir(parents=True, exist_ok=True)
    copy(project / 'dist', output / 'dist')
    subprocess.run(['node', 'node_modules/esbuild/bin/esbuild', 'server/index.ts', '--bundle', '--platform=node',
                    '--format=esm', '--packages=external', '--target=node22', f'--outfile={output.resolve() / "server.mjs"}'], cwd=project, check=True)
    copy(project / 'server/sqlite-query.mjs', output / 'sqlite-query.mjs')
    package = json.loads((project / 'package.json').read_text())
    # No upstream dev/watch scripts run during end-user setup.
    package.pop('scripts', None)
    (output / 'package.json').write_text(json.dumps(package, indent=2) + '\n')
    copy(project / 'package-lock.json', output / 'package-lock.json')
    copy(ROOT / 'packaging/linux/data-browser-launch.mjs', output / 'launch.mjs')
    identity = hashlib.sha256()
    for path in sorted(output.rglob('*')):
        if path.is_file():
            identity.update(path.relative_to(output).as_posix().encode() + b'\0' + path.read_bytes())
    (output / 'build.json').write_text(json.dumps({'revision': revision, 'id': identity.hexdigest(), 'minimumNodeMajor': 22}, indent=2) + '\n')
    licenses = target / 'licenses/data-browser'
    copy(project / 'README.md', licenses / 'UPSTREAM-README.md')
    records = []
    modules = project / 'node_modules'
    for path in sorted(modules.rglob('package.json')):
        if path.parent == modules: continue
        try: info = json.loads(path.read_text())
        except ValueError: continue
        if not info.get('name') or not info.get('version'): continue
        records.append({key: info.get(key) for key in ('name', 'version', 'license', 'repository')})
        for notice in path.parent.iterdir():
            if notice.is_file() and notice.name.lower().startswith(('license', 'licence', 'copying', 'notice')):
                copy(notice, licenses / path.parent.relative_to(modules) / notice.name)
    (licenses / 'packages.json').write_text(json.dumps(records, indent=2) + '\n')
    copy(project / 'package-lock.json', sources / 'data-browser-package-lock.json')
    for name in ('prepare_data_browser.py',): copy(ROOT / 'scripts' / name, sources / name)
    for name in ('data-browser-http.mjs', 'data-browser-launch.mjs'):
        copy(ROOT / 'packaging/linux' / name, sources / name)


if __name__ == '__main__':
    import argparse
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--destination', type=Path, required=True)
    parser.add_argument('--sources', type=Path, required=True)
    args = parser.parse_args()
    lock = json.loads((ROOT / 'runtime-linux-downloads.lock.json').read_text())
    prepare(args.destination.resolve(), args.sources.resolve(), lock)
