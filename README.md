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

On x86/x86-64, encoding from 16 bytes and decoding from 32 text bytes use
AVX2 after a cached `core::arch` CPUID/XGETBV
check verifies CPU support and OS vector-state handling. A build explicitly
targeting AVX2 omits that probe and requires that CPU/OS feature set to run.
When the target guarantees SSE2, decoding from 16 text bytes uses SSE2 if the
AVX2 path is not selected. AArch64 uses NEON when the target guarantees it.
Smaller inputs, other targets, SGX, and unsupported CPUs use scalar code.
`--features force-scalar` disables handwritten SIMD;
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

On an Intel Xeon Platinum 8573C cloud host, Linux 6.18.57 and Rust 1.99.1,
the second optimization pass measured the following geometric mean speed ratios.
A value above 1 means zhex is faster; each operation uses preallocated buffers
and checked decoding. Generic and `target-cpu=native` builds are separate.

| Comparison | Generic | Native |
| --- | ---: | ---: |
| Previous zhex / current, 0–64-byte payloads, lowercase encoding | 1.278× | 1.386× |
| Previous zhex / current, 0–64-byte payloads, uppercase encoding | 1.301× | 1.360× |
| Previous zhex / current, 0–64-byte payloads, decoding | 1.305× | 1.386× |
| Previous zhex / current, fixed 96 primary cases | 1.012× | 1.055× |
| Fastest external baseline / current, same 96 cases | 0.783× | 0.794× |

The **2× target was not met**; no primary case reached 2× the fastest competitor.
Primary cases span 16 bytes–8 MiB and four alignments. Compared with previous
zhex, 39/96 generic and 40/96 native primary cases regressed, including some
large encodes. These shared-host, hot-buffer timings are exploratory.
[The acceleration report](https://github.com/LIghtJUNction/zhex/blob/main/comparison/ACCELERATION.md)
records latency, payload throughput, scalar/SIMD and dispatch ablations,
rejected experiments, every regression, raw samples and reproduction metadata.

## Publishing

Run **Bump and release** on the default branch with `patch`, `minor`, `major`,
or `custom` plus an explicit `X.Y.Z`. `current` publishes the manifest's version
only if it has no release tag yet. For example:

```sh
gh workflow run bump.yml -R LIghtJUNction/zhex -f bump=patch
```

The workflow updates both manifests and lockfiles, commits the change, creates
an immutable `vX.Y.Z` tag, and runs the complete release checks on that exact
commit. Pushing a version tag directly or selecting an existing tag in the
**Release** workflow is also supported. The tag must match `Cargo.toml`.

Only the actual publish step receives the repository's `CARGO_REGISTRY_TOKEN`
secret; a missing secret fails explicitly. Publication follows package and
dry-run checks. A GitHub Release is created only after crates.io publication
succeeds. Published versions cannot be overwritten, and active releases are
serialized without cancellation.

MIT licensed.
