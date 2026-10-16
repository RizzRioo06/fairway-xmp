#!/usr/bin/env python3
"""Staple an accepted notarization ticket, retrying transient Apple failures."""
import subprocess
import sys
import time


def staple(path, run=subprocess.run, sleep=time.sleep):
    command = ['xcrun', 'stapler', 'staple', str(path)]
    for delay in (0, 5, 15, 30):
        if delay:
            print(f'Retrying Apple ticket delivery in {delay} seconds…', flush=True)
            sleep(delay)
        result = run(command, check=False)
        if result.returncode == 0:
            run(['xcrun', 'stapler', 'validate', str(path)], check=True)
            return
        # Missing ticket/CloudKit service failures can occur just after Apple
        # accepts a submission. Other failures must stop packaging immediately.
        if result.returncode not in (65, 68):
            raise subprocess.CalledProcessError(result.returncode, command)
    raise subprocess.CalledProcessError(result.returncode, command)


if __name__ == '__main__': staple(sys.argv[1])
