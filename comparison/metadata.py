#!/usr/bin/env python3
"""Capture provenance without reading credentials or environment values unrelated to the build."""
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import sys
import tarfile
from datetime import datetime, timezone


def command(args):
    result = subprocess.run(args, capture_output=True, text=True, check=False)
    return result.stdout.strip() if result.returncode == 0 else {"exit": result.returncode, "stderr": result.stderr.strip()}


destination = Path(sys.argv[1])
destination.mkdir(parents=True, exist_ok=True)
metadata_command = ["cargo", "metadata", "--locked", "--format-version", "1", "--manifest-path", "comparison/Cargo.toml"]
features = os.environ.get("ZHEX_BENCH_FEATURES", "")
if features:
    metadata_command.extend(["--features", features])
metadata = json.loads(subprocess.check_output(metadata_command, text=True))
sources = [Path("Cargo.toml"), Path("Cargo.lock"), Path("README.md"), Path("LICENSE"), *Path("src").glob("*.rs"), Path("comparison/Cargo.toml"), Path("comparison/Cargo.lock"), Path("comparison/PLAN.md"), Path("comparison/run.sh"), *Path("comparison").glob("*.py"), *Path("comparison/src").rglob("*.rs"), *Path("comparison/src").rglob("ORIGIN.md")]
cpu = {}
try:
    for line in Path("/proc/cpuinfo").read_text().splitlines():
        if not line.strip():
            break
        key, _, value = line.partition(":")
        if key.strip() in {"vendor_id", "model name", "cpu family", "model", "stepping", "microcode", "cpu MHz", "flags", "Features", "CPU implementer", "CPU architecture", "CPU part"}:
            cpu[key.strip()] = value.strip()
except OSError:
    cpu["unavailable"] = True
result = {
    "captured_utc": datetime.now(timezone.utc).isoformat(),
    "build": os.environ.get("ZHEX_BENCH_BUILD"),
    "arguments": sys.argv[2:],
    "source_commit": command(["git", "rev-parse", "HEAD"]),
    "source_status": command(["git", "status", "--short", "--", "src", "Cargo.toml", "Cargo.lock", "comparison/src", "comparison/Cargo.toml", "comparison/Cargo.lock", "comparison/PLAN.md"]),
    "source_sha256": {str(p): hashlib.sha256(p.read_bytes()).hexdigest() for p in sources},
    "rustc": command(["rustc", "-vV"]),
    "cargo": command(["cargo", "-V"]),
    "rustflags": os.environ.get("RUSTFLAGS", ""),
    "features": features,
    "profile": {"opt-level": 3, "lto": "fat", "codegen-units": 1, "panic": "abort"},
    "cpu": cpu,
    "os": platform.platform(),
    "cpu_affinity": sorted(os.sched_getaffinity(0)) if hasattr(os, "sched_getaffinity") else None,
    "packages": [{key: p.get(key) for key in ("name", "version", "license", "source", "rust_version")} for p in metadata["packages"]],
    "resolved_features": {p["id"]: p["features"] for p in metadata["resolve"]["nodes"]},
    "seed": "0x4d595df4d0f33173",
    "timer": "std::time::Instant; seven unfiltered samples, median summary; exploratory shared cloud CPU",
    "throughput": "payload GiB/s: input bytes for encode, decoded bytes for decode",
}
(destination / "metadata.json").write_text(json.dumps(result, indent=2) + "\n")
with tarfile.open(destination / "source.tar.gz", "w:gz") as archive:
    for path in sources:
        archive.add(path, arcname=str(path), recursive=False)
