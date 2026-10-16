#!/usr/bin/env python3
"""Publish a tested beta with verified updater signatures; feed promotion is separate."""
from publish_unsigned_release import main

if __name__ == '__main__': main(signed=True)
