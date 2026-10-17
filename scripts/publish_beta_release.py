#!/usr/bin/env python3
"""Compatibility entry point for the generalized signed release publisher."""
from publish_unsigned_release import main

if __name__ == '__main__': main(signed=True)
