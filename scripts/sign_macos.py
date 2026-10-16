#!/usr/bin/env python3
"""Sign independent Mach-O files first, then seal and verify the app bundle."""
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path
import subprocess
import sys
from prepare_runtime import macho

app, identity = Path(sys.argv[1]), sys.argv[2]
# Finder can run the bundled .command uninstaller only when it is executable.
uninstaller = app / 'Contents/Resources/Uninstall Fairway XMP.command'
if uninstaller.is_file():
    uninstaller.chmod(uninstaller.stat().st_mode | 0o111)


def sign(file):
    subprocess.run(['codesign', '--force', '--options', 'runtime', '--timestamp', '--sign', identity, str(file)], check=True)


# Each file is independent (symlink aliases are skipped). Seal the containing
# app only after every executable/library succeeds. Bounded concurrency avoids
# hundreds of sequential timestamp requests for the bundled PHP/MySQL tools.
files = [file for file in app.rglob('*') if not file.is_symlink() and macho(file)]
with ThreadPoolExecutor(max_workers=4) as pool:
    list(pool.map(sign, files))
sign(app)
subprocess.run(['codesign', '--verify', '--deep', '--strict', '--verbose=2', str(app)], check=True)
