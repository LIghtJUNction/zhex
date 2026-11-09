# zhex

Hexadecimal encoding and decoding into caller-owned buffers. The crate always
uses `#![no_std]` and only `core`: no allocation, dependencies, build script, or
external runtime. Stable Rust 1.70 or newer.

```rust
let mut text = [0; 8];
assert_eq!(zhex::encode_lower(&[0, 18, 171, 255], &mut text), Ok(8));
assert_eq!(&text, b"0012abff");

let mut bytes = [0; 4];
assert_eq!(zhex::decode(b"0012aBfF", &mut bytes), Ok(4));
assert_eq!(bytes, [0, 18, 171, 255]);
```

`encode_upper` produces uppercase text. `encoded_len` checks multiplication
overflow; `decoded_len` rejects odd lengths. All functions return `Result`.
An oversized output is accepted and its unused suffix stays unchanged.

Decoding accepts only ASCII `0–9`, `a–f`, and `A–F`. Empty input succeeds.
Error precedence is odd length, insufficient capacity, then the first invalid
byte. `Error::InvalidByte { index, byte }` uses the original input byte offset.
Encoding checks overflow before capacity. Length and capacity errors leave all
output unchanged. An invalid byte may leave an unspecified output prefix
modified; decoding checks characters as it goes, without a full prescan.

## Backends and verification

On x86/x86-64, long inputs use AVX2 after a cached `core::arch` CPUID/XGETBV
check verifies CPU support and OS vector-state handling. A build explicitly
targeting AVX2 omits that probe and requires that CPU/OS feature set to run.
AArch64 uses NEON when the compilation target
guarantees it. Short inputs, other targets, SGX, and unsupported CPUs use the
scalar implementation. `--features force-scalar` disables handwritten SIMD;
the compiler may still vectorize ordinary Rust. No slice padding is required.

```sh
cargo test --locked
cargo test --locked --release --features force-scalar
```

CI runs stable and MSRV tests on x86-64 and AArch64 hosts, forced supported
SIMD, Miri on scalar code, bare-metal compilation, and package dependency checks.
Shared-runner benchmark timings are smoke tests without performance thresholds.

The standalone [comparison project](https://github.com/LIghtJUNction/zhex/tree/main/comparison) is excluded from
the crate's workspace and published package. It pins checked, nonallocating
competitor APIs and records every size, offset, failed goal, and raw sample.
Its third-party dependencies never enter zhex's dependency tree.

## Measured performance

The **2× target was not met**. On an Intel Xeon Platinum 8573C cloud host with
Rust 1.99.1, the fixed 96 primary cases gave geometric mean speed ratios of
**0.744× generic / 0.723× native** against each case's fastest checked baseline;
no primary case reached 2×. Cases span 16 bytes–8 MiB and four alignments.
These shared-host timings are exploratory. [The full report](https://github.com/LIghtJUNction/zhex/blob/main/comparison/PERFORMANCE.md)
includes short-input latency, large-input payload throughput, scalar/SIMD
ablations, all regressions, raw samples and exact reproduction metadata.

## Publishing

Push an existing version's `vX.Y.Z` tag, or select that existing tag in the
Release workflow. The tag must match `Cargo.toml`. The workflow validates the
exact commit, packages and dry-runs it, then publishes using the repository's
`CARGO_REGISTRY_TOKEN` secret. A missing secret fails explicitly. A GitHub Release
is created only after crates.io publication succeeds; published versions cannot
be overwritten.

MIT licensed.
