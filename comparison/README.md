# Reproduce the measurements

This directory is a separate, unpublished Cargo workspace. Its dependencies,
lockfile and benchmark tools are excluded from zhex's released package. Use
Rust 1.99.1 to reproduce the saved measurements; the library itself supports its
declared older MSRV. Every competitor is pinned in `Cargo.toml` and `Cargo.lock`.

```sh
# From the repository root. Each output directory must be new.
comparison/run.sh generic comparison/results/my-generic
comparison/run.sh native comparison/results/my-native
comparison/run.sh scalar comparison/results/my-scalar
comparison/run.sh candidates comparison/results/my-candidates
comparison/run.sh generic comparison/results/my-kernels --kernels
comparison/run.sh native comparison/results/my-native-kernels --kernels

# Same-process previous implementation, and a short-input ablation.
comparison/run.sh generic comparison/results/my-tuning --tune --primary
comparison/run.sh generic comparison/results/my-short-kernels --kernels --short

# Short functional smoke; timings are not performance evidence.
cargo run --release --locked --manifest-path comparison/Cargo.toml -- --smoke

# Exhaustive adapter checks without timing.
cargo run --release --locked --manifest-path comparison/Cargo.toml -- --check-only
```

On Linux, prefix the script with `taskset -c <available_cpu>` to keep the process
on one logical CPU. `metadata.json` records the resulting affinity. Stop other
CPU-intensive work during timing. The saved cloud measurements remain
exploratory because the host's other workloads are outside this process's control.

Read [PLAN.md](PLAN.md) for the workload fixed before timing, semantic adapters,
sample protocol, throughput denominator and success criterion. Input generation,
allocation and result verification happen outside timing. Seven unfiltered raw
samples per combination are retained in `raw.csv.gz`; `summary.csv` contains
medians and ranges; `fastest.csv` contains every primary case against its fastest
eligible competitor. `previous.csv` reports the same-process historical zhex
comparison, which is excluded from the competitive score. Invalid-input timings
are separate. Initial tuning runs
are labeled exploratory; later committed-source sweeps are the delivery evidence.

The runner snapshots its source into `source.tar.gz`, checks for source changes
during compilation, records the executable SHA-256, and executes that exact
binary. It refuses to overwrite prior evidence. Reproduce a snapshot by
extracting it into a fresh directory and running the same command and toolchain.
`results/registry.json` records the official crates.io sparse-index version and
checksum check, licenses and each published source commit.

The checked baseline APIs are `hex::encode_to_slice/decode_to_slice`,
`faster_hex::hex_encode/hex_encode_upper/hex_decode`,
`const_hex::encode_to_slice/encode_to_slice_upper/decode_to_slice`,
`hex_simd::encode/decode`, `better_hex::encode_to_slice/encode_to_slice_upper/decode_to_slice`,
`fashex::encode/decode`, and `hex_turbo::Engine::encode_slice/decode_slice`.
`hex`'s missing uppercase slice API is explicitly recorded as unsupported.

Competitors are linked only here under their published licenses: `hex-simd`
and `faster-hex` use MIT; `hex-turbo` uses 0BSD OR Apache-2.0; the remaining
baselines use MIT OR Apache-2.0. The published decoder attributes the original
public-domain Lemire table algorithm; it does not depend on these crates.
Arithmetic, LUT and SWAR prototypes in `src/candidates.rs`
are local experiments; `src/kernels.rs` includes the exact production kernels
to compare scalar, SIMD and the short-input cutoff without expanding public API.
