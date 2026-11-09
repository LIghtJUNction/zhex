#!/usr/bin/env python3
"""Resolve an existing release tag through shared validation rules."""

import sys

from release_tools import main

main(["resolve", *sys.argv[1:]])

