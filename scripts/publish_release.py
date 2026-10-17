#!/usr/bin/env python3
"""Publish exact tested beta, RC or stable artifacts; feed promotion is separate."""
from publish_unsigned_release import main

if __name__ == '__main__': main(signed=True)
