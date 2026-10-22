#!/usr/bin/env python3
"""Staple and validate notarization tickets, retrying transient Apple failures."""
import subprocess
import sys
import time


def _ticket_action(path, action, run, sleep):
    command = ['xcrun', 'stapler', action, str(path)]
    for delay in (0, 5, 15, 30):
        if delay:
            print(f'Retrying Apple ticket {action} in {delay} seconds…', flush=True)
            sleep(delay)
        result = run(command, check=False)
        if result.returncode == 0:
            return
        # Validation also contacts CloudKit and can lose its connection
        # after stapling succeeded. Retry only known ticket/service errors;
        # never let a failed validation produce a release artifact.
        if result.returncode not in (65, 68):
            raise subprocess.CalledProcessError(result.returncode, command)
    raise subprocess.CalledProcessError(result.returncode, command)


def validate(path, run=subprocess.run, sleep=time.sleep):
    """Validate the delivered ticket without modifying the installed bundle."""
    _ticket_action(path, 'validate', run, sleep)


def staple(path, run=subprocess.run, sleep=time.sleep):
    _ticket_action(path, 'staple', run, sleep)
    validate(path, run, sleep)


if __name__ == '__main__': staple(sys.argv[1])
