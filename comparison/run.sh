#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
mode=${1:-generic}
destination=${2:-comparison/results/$mode}
if [ "$#" -ge 2 ]; then shift 2; else shift "$#"; fi
features=()
export ZHEX_BENCH_FEATURES=""
case "$mode" in
  generic) export RUSTFLAGS="" ;;
  native) export RUSTFLAGS="-C target-cpu=native" ;;
  scalar) export RUSTFLAGS=""; export ZHEX_BENCH_FEATURES="force-scalar"; features=(--features force-scalar) ;;
  candidates) export RUSTFLAGS=""; export ZHEX_BENCH_FEATURES="force-scalar"; features=(--features force-scalar); set -- --candidates "$@" ;;
  *) printf 'Unknown build mode: %s\n' "$mode" >&2; exit 2 ;;
esac
if [ -e "$destination/metadata.json" ] || [ -e "$destination/raw.csv" ] || [ -e "$destination/raw.csv.gz" ]; then
  printf 'Refusing to overwrite prior evidence: %s\n' "$destination" >&2
  exit 2
fi
mkdir -p "$destination"
export ZHEX_BENCH_BUILD="$mode"
python3 comparison/metadata.py "$destination" "$@"
cargo build --release --locked --manifest-path comparison/Cargo.toml "${features[@]}" --message-format=json > "$destination/build.jsonl"
binary=$(python3 comparison/built_binary.py "$destination")
"$binary" "$@" > "$destination/raw.csv" 2> "$destination/run.log"
gzip -f "$destination/raw.csv"
python3 comparison/summarize.py "$destination/raw.csv.gz" "$destination"
