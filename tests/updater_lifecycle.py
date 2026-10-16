"""Exercise the real Tauri updater with exact signed artifacts on hosted runners."""
from functools import partial
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import subprocess
import threading
import time


def run_probe(command, output):
    """Fail a stalled native installer with its stack, preserving the cause."""
    timeout = 900 if os.name == 'nt' else 120
    with subprocess.Popen(command, env={**os.environ, 'FAIRWAY_INSTALLER_TEST': '1'}) as process:
        try:
            code = process.wait(timeout=timeout)
        except subprocess.TimeoutExpired:
            if os.name != 'nt':
                try:
                    subprocess.run(['sample', str(process.pid), '3', '-file', str(output / 'updater-process-sample.txt')], check=False, timeout=15)
                except (OSError, subprocess.SubprocessError) as error:
                    print(f'Could not capture updater stack: {error}', flush=True)
            process.kill(); process.wait()
            raise
        if code:
            raise subprocess.CalledProcessError(code, command)


def exercise(probe, executable, artifact, output):
    artifact = artifact.resolve()
    signature = artifact.with_name(artifact.name + '.sig').read_text().strip()
    root = output / 'updater-feed'
    root.mkdir()
    os.link(artifact, root / artifact.name)
    (root / 'tampered.bin').write_bytes(b'Tampered update must not stop services')
    server = ThreadingHTTPServer(('127.0.0.1', 0), partial(SimpleHTTPRequestHandler, directory=str(root)))
    base = f'http://127.0.0.1:{server.server_port}'
    version = json.loads((Path(__file__).resolve().parents[1] / 'package.json').read_text())['version']
    target = 'windows-x86_64' if os.name == 'nt' else 'darwin-aarch64'
    feed = {'version': version, 'notes': 'Exact signed candidate reinstall drill', 'platforms': {target: {'signature': signature, 'url': f'{base}/{artifact.name}'}}}
    (root / 'latest.json').write_text(json.dumps(feed))
    bad = json.loads(json.dumps(feed)); bad['platforms'][target]['url'] = f'{base}/tampered.bin'
    (root / 'bad.json').write_text(json.dumps(bad))
    wrong_version = json.loads(json.dumps(feed)); wrong_version['version'] = '99.0.0'
    (root / 'wrong-version.json').write_text(json.dumps(wrong_version))
    report = output / 'updater.json'
    thread = threading.Thread(target=server.serve_forever, daemon=True); thread.start()
    try:
        run_probe([str(probe.resolve()), str(executable), base, str(report), str(Path('packaging/updater.pub').resolve())], output)
        result = json.loads(report.read_text())
        if result['status'] != 'prepared': raise RuntimeError('Updater preparation did not complete')
        # NSIS is launched asynchronously by the plugin; it exits the probe.
        # Its --install-cli hook marks completion after all files are replaced.
        if os.name == 'nt':
            marker = Path(os.environ['LOCALAPPDATA']) / 'dev.fairway.environment/cli-installed'
            deadline = time.monotonic() + 180
            while time.monotonic() < deadline:
                try:
                    if marker.is_file() and marker.stat().st_mtime_ns > (root / 'latest.json').stat().st_mtime_ns:
                        subprocess.run([str(executable), '--build-info'], check=True, capture_output=True, timeout=10)
                        break
                except (OSError, subprocess.SubprocessError): pass
                time.sleep(.5)
            else: raise RuntimeError('NSIS update did not finish its post-install hook')
        else:
            subprocess.run(['codesign', '--verify', '--deep', '--strict', str(executable.parents[2])], check=True)
            subprocess.run(['xcrun', 'stapler', 'validate', str(executable.parents[2])], check=True)
        result['status'] = 'passed'
        result['checks'].append('real-tauri-install-of-exact-signed-candidate')
        result['note'] = 'Same-version native reinstall drill; production version comparison remains enabled. Human restart and subsequent-version acceptance are separate.'
        report.write_text(json.dumps(result, indent=2) + '\n')
        return result
    finally:
        server.shutdown(); server.server_close(); thread.join()
