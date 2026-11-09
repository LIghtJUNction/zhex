#!/usr/bin/env python3
"""Resolve an existing vX.Y.Z tag to a commit and verify its crate version."""

import argparse
import os
import re
import subprocess
import tomllib


parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("tag")
parser.add_argument("--expected-sha")
args = parser.parse_args()
if not re.fullmatch(r"v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)", args.tag):
    raise SystemExit("Release tag must be vX.Y.Z (no prerelease, suffix, or leading zero)")
sha = subprocess.check_output([
    "git", "rev-parse", "--verify", f"refs/tags/{args.tag}^{{commit}}",
], text=True).strip()
if args.expected_sha and sha != args.expected_sha:
    raise SystemExit("Release tag changed after validation; refusing to publish")
manifest = tomllib.loads(subprocess.check_output([
    "git", "show", f"{sha}:Cargo.toml",
], text=True))
version = manifest["package"]["version"]
if manifest["package"]["name"] != "zhex" or args.tag != f"v{version}":
    raise SystemExit("Release tag does not match zhex's Cargo.toml version")
print(f"Validated {args.tag} at {sha}")
if "GITHUB_OUTPUT" in os.environ:
    with open(os.environ["GITHUB_OUTPUT"], "a", encoding="utf-8") as output:
        output.write(f"tag={args.tag}\nsha={sha}\nversion={version}\n")

