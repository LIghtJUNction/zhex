#!/usr/bin/env python3
"""Refuse source drift during compilation and record the executable being timed."""
import hashlib
import json
from pathlib import Path
import sys

destination = Path(sys.argv[1])
path = destination / "metadata.json"
metadata = json.loads(path.read_text())
for source, expected in metadata["source_sha256"].items():
    actual = hashlib.sha256(Path(source).read_bytes()).hexdigest()
    if actual != expected:
        raise SystemExit(f"Source changed during build: {source}; rerun after edits finish")
executable = None
for line in (destination / "build.jsonl").read_text().splitlines():
    item = json.loads(line)
    if item.get("reason") == "compiler-artifact" and item.get("target", {}).get("name") == "zhex-comparison" and item.get("executable"):
        executable = Path(item["executable"])
if executable is None:
    raise SystemExit("Cargo did not report the comparison executable")
metadata["executable_sha256"] = hashlib.sha256(executable.read_bytes()).hexdigest()
metadata["execution"] = "Previously built executable, no rebuild between metadata and measurement"
path.write_text(json.dumps(metadata, indent=2) + "\n")
print(executable)
