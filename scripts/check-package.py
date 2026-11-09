#!/usr/bin/env python3
"""Enforce the dependency-free publish boundary (Python 3.11+, CI only)."""

import argparse
import json
from pathlib import Path
import re
import subprocess
import tarfile
import tomllib


def check_manifest(manifest):
    def visit(table):
        for key, value in table.items():
            if key in ("dependencies", "dev-dependencies", "build-dependencies"):
                if value:
                    raise SystemExit(f"Forbidden dependency table: {key}: {value}")
            elif isinstance(value, dict):
                visit(value)

    visit(manifest)
    if manifest["package"].get("build", False) is not False:
        raise SystemExit("Build scripts are forbidden")


def allowed(name):
    return name in {
        ".cargo_vcs_info.json", "Cargo.toml", "Cargo.toml.orig", "Cargo.lock",
        "README.md", "LICENSE", "LICENSE-MIT", "LICENSE-APACHE",
    } or name.startswith(("src/", "tests/"))


parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--archive", action="store_true")
args = parser.parse_args()
manifest = tomllib.loads(Path("Cargo.toml").read_text())
check_manifest(manifest)
if Path("build.rs").exists():
    raise SystemExit("build.rs is forbidden")
if "#![no_std]" not in Path("src/lib.rs").read_text().splitlines():
    raise SystemExit("src/lib.rs must unconditionally declare #![no_std]")
for source in Path("src").rglob("*.rs"):
    if re.search(r"\b(?:extern\s+crate\s+alloc|alloc\s*::)", source.read_text()):
        raise SystemExit(f"alloc is forbidden: {source}")

metadata = json.loads(subprocess.check_output([
    "cargo", "metadata", "--locked", "--offline", "--format-version", "1", "--all-features",
], text=True))
if len(metadata["packages"]) != 1 or len(metadata["workspace_members"]) != 1:
    raise SystemExit("The published workspace must contain only zhex")
package = metadata["packages"][0]
if package["dependencies"] or any("custom-build" in t["kind"] for t in package["targets"]):
    raise SystemExit("Cargo metadata contains a dependency or build script")

if args.archive:
    prefix = f"{package['name']}-{package['version']}"
    archive = Path(metadata["target_directory"]) / "package" / f"{prefix}.crate"
    with tarfile.open(archive) as tar:
        names = []
        for member in tar.getmembers():
            if not member.isfile() or not member.name.startswith(prefix + "/"):
                raise SystemExit(f"Unexpected archive member: {member.name}")
            name = member.name[len(prefix) + 1:]
            if not allowed(name):
                raise SystemExit(f"Unexpected published file: {name}")
            names.append(name)
        for required in ("Cargo.lock", "Cargo.toml", "README.md", "src/lib.rs"):
            if required not in names:
                raise SystemExit(f"Missing published file: {required}")
        check_manifest(tomllib.loads(tar.extractfile(f"{prefix}/Cargo.toml").read().decode()))
        print(f"Verified {len(names)} package files: no comparison/, scripts/, or workflows")
print("Verified: one crate, zero dependencies of every kind, no build.rs, no_std, no alloc")

