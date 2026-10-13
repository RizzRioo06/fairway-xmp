#!/usr/bin/env python3
from pathlib import Path
import subprocess
import sys
from prepare_runtime import macho

app, identity = Path(sys.argv[1]), sys.argv[2]
for file in sorted(app.rglob('*'), key=lambda p: len(p.parts), reverse=True):
    if macho(file):
        subprocess.run(['codesign', '--force', '--options', 'runtime', '--timestamp', '--sign', identity, str(file)], check=True)
subprocess.run(['codesign', '--force', '--options', 'runtime', '--timestamp', '--sign', identity, str(app)], check=True)
subprocess.run(['codesign', '--verify', '--deep', '--strict', '--verbose=2', str(app)], check=True)
