#!/usr/bin/env bash
set -euo pipefail

# Use std only in this test executable to verify CPU *and OS* support before
# starting a binary compiled with the SIMD feature globally enabled.
probe_dir=$(mktemp -d)
trap 'rm -rf "$probe_dir"' EXIT
rustc --edition=2021 -o "$probe_dir/probe" - <<'RS'
fn main() {
    #[cfg(target_arch = "x86_64")]
    assert!(std::is_x86_feature_detected!("avx2"), "AVX2 unavailable on this runner");
    #[cfg(target_arch = "aarch64")]
    assert!(std::arch::is_aarch64_feature_detected!("neon"), "NEON unavailable on this runner");
}
RS
"$probe_dir/probe"
case "$(uname -m)" in
  x86_64) export RUSTFLAGS='-C target-feature=+avx2' ;;
  aarch64) export RUSTFLAGS='-C target-feature=+neon' ;;
  *) echo 'Unsupported SIMD test runner architecture' >&2; exit 1 ;;
esac
cargo test --locked --release

