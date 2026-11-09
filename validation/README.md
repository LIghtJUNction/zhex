# Verification evidence

The production source at
[`60d913d2501a858825cf774ac454efe25e509234`](https://github.com/LIghtJUNction/zhex/commit/60d913d2501a858825cf774ac454efe25e509234)
passed all **11 jobs** in [CI run 42648275776](https://github.com/LIghtJUNction/zhex/actions/runs/42648275776).
The [structured record](ci-42648275776/results.json) retains every job and step,
the exact commit, timestamps and original GitHub URLs. Adjacent `.log` files are
labeled excerpts of verification output; setup and cleanup noise is omitted.

| Check | Executed result |
| --- | --- |
| Stable Rust 1.99.1, x86-64 | Debug/release tests, doctest, forced scalar, native and forced AVX2 passed |
| Rust 1.70.0 MSRV, x86-64 | Same tests passed |
| Stable Rust 1.99.1, AArch64 | Tests ran on an actual ARM runner, including forced NEON, and passed |
| Rust 1.70.0 MSRV, AArch64 | Same actual ARM tests passed |
| `thumbv7em-none-eabi` | Stable and MSRV compiled the library, including `force-scalar` |
| Miri | Scalar integration tests and doctest passed; interpreter-specific bounds reduce exhaustive loop counts |
| Quality | Formatting and Clippy with denied warnings passed |
| Package boundary | One crate, no dependencies of any kind, no `build.rs`, unconditional `no_std`, and no `alloc` |
| Publish preparation | Package listing, package build, archive-policy check and `cargo publish --dry-run --locked` passed |
| Benchmark smoke | Both x86-64 and AArch64 ran checked competitor adapters and direct scalar/SIMD kernels |

The core tests use independent arithmetic/range references. They cover every
single byte, all 65,536 binary byte pairs, all 65,536 textual byte pairs,
mixed case, invalid characters and first-error positions, output sentinels,
capacity and overflow preflight, vector boundaries, independent slice offsets,
and inputs through 4 MiB + 1 byte. A separate integration binary synchronizes
16 threads for cold dispatch initialization. x86 unit tests check the actual
detector against `std`, and inject missing CPUID/XCR0 capabilities to verify
that unsupported instructions are never probed before their prerequisites.

## Raw runner artifacts

[Artifact provenance](ci-42648275776/artifacts.json) records the downloaded
GitHub artifact IDs and SHA-256 digests. Downloaded ZIP digests were verified;
the exact CSV contents are retained in
[`arm/`](ci-42648275776/arm/) and [`x86/`](ci-42648275776/x86/).
Both platforms exercised the production, scalar, and supported SIMD paths.
These short runs establish executable coverage; their timings are not used
for performance claims or regression thresholds.

Earlier [CI run 42646730969](https://github.com/LIghtJUNction/zhex/actions/runs/42646730969)
also passed all 11 jobs before the final AVX2 arithmetic improvement; its
[record](ci-42646730969/results.json) remains available separately.

## Release status

**No real crates.io publication or GitHub Release was performed.** The successful
dry-run explicitly stopped before uploading. The release workflow validates an
existing `vX.Y.Z` tag and runs the full check suite on its exact commit before
making `CARGO_REGISTRY_TOKEN` available only to the real publishing step. Missing
credentials, a changed tag, version mismatch or duplicate publication fail the
workflow. The GitHub Release job depends on successful crates.io publication.

The workflow files passed actionlint 1.7.12. The tag resolver was also exercised
against temporary Git fixtures: matching version, mismatched version, missing
tag, malformed version, injection-like input and changed expected commit. Those
fixtures did not create tags in this repository.
