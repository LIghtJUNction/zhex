# Performance evidence

**The 2× performance target was not met.** On the fixed 96 primary cases, zhex achieved a geometric mean of **0.744×** the fastest eligible baseline in the generic build and **0.723×** in the native build. No primary case reached 2×. zhex lost 89/96 generic cases and 92/96 native cases; all losing cases and raw samples are retained. A ratio below 1 means the competitor was faster. These are exploratory measurements on a shared cloud CPU, not a claim of leadership across machines.

The measured source commit is [`60d913d2501a858825cf774ac454efe25e509234`](https://github.com/LIghtJUNction/zhex/commit/60d913d2501a858825cf774ac454efe25e509234). The five final runs contain **201,264 measured samples**, plus **984 explicitly unsupported rows** for hex’s absent uppercase slice API. The source was clean during every final capture. Source archives, compiler settings and executable hashes accompany each run.

## Fixed protocol and strongest baselines

The [protocol](PLAN.md) was written before the first timing run. Primary payloads are 16, 32, 64, 256, 4,096, 65,536, 1,048,576 and 8,388,608 bytes. Each has lower encoding, upper encoding and mixed-case decoding at four input/output offsets modulo 64: `(0,0)`, `(1,3)`, `(7,15)`, `(31,1)`. The full sweep covers 82 distinct lengths, including every length 0–64, vector boundaries, and the large inputs. Invalid bytes at first/middle/last positions are reported separately.

All implementations use checked APIs and the same initialized, preallocated output slices. Allocation, seeded input generation, poisoning, guard checks and independent output verification occur outside timing. All 65,536 possible two-byte decoder inputs are checked for every adapter before measurement. Each valid output byte is poisoned to the complement of its expected value before per-codec validation. `black_box` surrounds function pointers, inputs, outputs and results; the timer includes the common function-pointer call overhead.

Each measured combination has seven unfiltered samples after at least 1 ms warmup, calibrated toward 2 ms per sample. Implementations rotate order between samples. Results use sample medians, with min/max samples retained in CSV. Throughput is **payload GiB/s**: input bytes for encoding and decoded output bytes for decoding, divided by 2^30. A successful conversion reads and writes a total of three times the payload length. Invalid-input rows report latency without a throughput value.

The adapters enforce the same length/capacity/error-index contract without allocating. `const-hex`’s `0x`/`0X` acceptance is rejected. Libraries lacking an invalid-byte index get a position scan only after their checked decoder returns an error. No unchecked decoder or allocating String API is used. Transactional decoders keep their stronger write behavior; no validation is removed for speed.

Versions were checked against the official crates.io sparse index on 2026-11-09. Each archive checksum, license and published VCS commit is recorded in [registry.json](results/registry.json); dependency resolution is fixed by [Cargo.lock](Cargo.lock). The three additional baselines extend the required four with current reproducible implementations.

| Baseline | Version | Published license |
| --- | --- | --- |
| hex | 0.4.3 | MIT OR Apache-2.0 |
| faster-hex | 1.0.0 | MIT |
| const-hex | 1.19.3 | MIT OR Apache-2.0 |
| hex-simd | 0.8.0 | MIT |
| better-hex | 1.0.1 | MIT OR Apache-2.0 |
| fashex | 0.1.1 | MIT OR Apache-2.0 |
| hex-turbo | 0.4.0 | 0BSD OR Apache-2.0 |

## Hardware, toolchain and builds

The host reports **Intel Xeon Platinum 8573C**, x86-64, AVX2 and AVX-512 features. Measurements were pinned to logical CPU 0, but the physical host and neighboring workloads were shared. The OS reports Linux 6.18.57 with glibc 2.41. The compiler was **rustc 1.99.1 (`58cf8f9c3`, LLVM 23.1.1)**. Final captures started between 23:48 and 23:59 on 2026-11-09 in Asia/Taipei (UTC+8); exact UTC timestamps are stored in metadata.

Every dependency uses the comparison workspace’s release profile: `opt-level=3`, `lto="fat"`, `codegen-units=1`, `panic="abort"`. Generic means empty `RUSTFLAGS`; native means `-C target-cpu=native`. The scalar run keeps generic flags and enables only zhex’s `force-scalar` feature. Competitors keep their default dispatch. Compiler auto-vectorization remains enabled in the scalar build, so “force-scalar” means no hand-written zhex SIMD, not an absence of SIMD machine instructions.

## Primary scorecard

The score is the geometric mean of `fastest competitor median ns / zhex median ns`, with equal weight for every primary case. Each case picks its fastest eligible competitor. Kernel/candidate experiments are excluded. The preregistered broad-success criterion also requires no primary case below parity.

| Build | All 96 cases | Lower encode | Upper encode | Mixed decode | Cases ≥2× | Cases below parity |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| generic | 0.744× | 0.777× | 0.830× | 0.638× | 0/96 | 89/96 |
| native | 0.723× | 0.762× | 0.786× | 0.631× | 0/96 | 92/96 |
| scalar | 0.283× | 0.417× | 0.438× | 0.124× | 0/96 | 94/96 |

The scalar score is an ablation against competitors that still use their available SIMD. It does not estimate comparative performance on a CPU lacking SIMD. Full per-case scorecards: [generic](results/final-generic/fastest.csv), [native](results/final-native/fastest.csv), [scalar](results/final-scalar/fastest.csv). The generic/native difference is small relative to observed host variation; it is not a fine performance-regression finding.

### Short-input latency

Values are **ns/op**, shown as the minimum–maximum across the four alignment-specific sample medians. They span separately timed alignments and include host variation; they are not confidence intervals or an isolated alignment penalty. Every length 0–64 remains in the full [generic](results/final-generic/summary.csv) and [native](results/final-native/summary.csv) CSVs.

| Build | Payload bytes | Lower encode ns/op | Upper encode ns/op | Mixed decode ns/op |
| --- | ---: | ---: | ---: | ---: |
| generic | 0 | 8.2–9.4 | 8.0–8.8 | 8.1–9.4 |
| generic | 1 | 8.7–9.7 | 8.5–12.6 | 8.3–9.5 |
| generic | 8 | 8.4–10.6 | 8.2–11.6 | 11.4–13.9 |
| generic | 16 | 9.4–15.8 | 9.7–11.3 | 10.5–11.8 |
| generic | 32 | 9.0–11.0 | 9.1–10.3 | 10.5–13.8 |
| generic | 64 | 11.7–22.5 | 13.5–17.1 | 15.4–24.0 |
| native | 0 | 8.1–11.0 | 8.4–12.1 | 8.2–9.1 |
| native | 1 | 8.4–9.6 | 7.9–10.8 | 8.6–9.2 |
| native | 8 | 14.4–24.1 | 7.7–27.3 | 10.1–20.4 |
| native | 16 | 10.3–26.9 | 9.0–22.5 | 10.0–14.2 |
| native | 32 | 8.8–9.8 | 8.0–10.0 | 9.7–12.6 |
| native | 64 | 10.6–20.3 | 8.7–20.6 | 19.2–21.3 |

### Large-input throughput

Values are **payload GiB/s**, again minimum–maximum across the four alignment-specific sample medians. Encoding and decoding use the same payload-byte denominator.

| Build | Payload | Lower encode GiB/s | Upper encode GiB/s | Mixed decode GiB/s |
| --- | --- | ---: | ---: | ---: |
| generic | 256 B | 7.00–10.64 | 8.40–11.49 | 5.10–5.82 |
| generic | 4 KiB | 18.53–21.60 | 16.68–24.42 | 7.50–11.49 |
| generic | 64 KiB | 8.78–18.86 | 13.18–15.28 | 7.39–8.83 |
| generic | 1 MiB | 5.47–7.09 | 5.48–7.59 | 5.20–7.47 |
| generic | 8 MiB | 5.31–7.59 | 6.57–9.03 | 4.79–7.54 |
| native | 256 B | 12.59–18.42 | 12.22–16.83 | 7.15–11.14 |
| native | 4 KiB | 19.80–22.34 | 18.56–22.35 | 9.22–11.43 |
| native | 64 KiB | 14.07–18.59 | 15.55–17.72 | 6.23–10.18 |
| native | 1 MiB | 3.99–7.55 | 5.29–9.02 | 6.48–7.69 |
| native | 8 MiB | 5.42–7.76 | 4.31–6.79 | 4.39–6.70 |

For a direct comparison at the fixed `(0,0)` alignment, the following 4 KiB cases show why beating the scalar hex crate is insufficient. Each competitor value comes from its matching build and checked API.

| Build | Operation | zhex GiB/s | Fastest baseline | Baseline GiB/s | zhex / baseline |
| --- | --- | ---: | --- | ---: | ---: |
| generic | encode_lower | 19.37 | faster-hex | 24.80 | 0.781× |
| generic | encode_upper | 18.71 | hex-simd | 26.42 | 0.708× |
| generic | decode_mixed | 7.50 | hex-turbo | 17.10 | 0.438× |
| native | encode_lower | 20.47 | hex-turbo | 25.79 | 0.794× |
| native | encode_upper | 20.91 | better-hex | 28.61 | 0.731× |
| native | decode_mixed | 11.25 | hex-turbo | 22.21 | 0.507× |

## Measured implementation choices

### Arithmetic, tables and SWAR

The safe candidate sweep retained 5,400 combinations × seven samples in [candidates-full](results/candidates-full/summary.csv), with its [source snapshot and compiler metadata](results/candidates-full/metadata.json). Arithmetic encoding allows ordinary compiler vectorization; the alternatives use a 16-byte nibble table, 256 byte-pair entries per case, or four input bytes expanded through a u64 SWAR expression. The same checked adapter/preallocated output protocol applies.

For payloads 16–64 bytes, the pair-table candidate’s geometric mean speed relative to the selected arithmetic production path was 0.873× lower / 0.964× upper; SWAR was 0.871× / 0.940×. Across measured payloads 256 B–8 MiB, pair-table ratios were 0.541× / 0.691× and SWAR ratios 0.528× / 0.631×. Arithmetic remains the production choice. Small-table gains below 16 bytes were mixed, so no additional short-table branch was added. These are local engineering experiments, not competitor rankings.

Decoding uses a 256-entry nibble table with an invalid sentinel, preserving first-error order with a single combined validity test per pair. Range-match and LUT decoder prototypes are both retained in the candidate source and raw data. Earlier exploratory scalar results and their capture limitations are explicitly documented in [EXPLORATORY.md](results/EXPLORATORY.md).

### SIMD and the scalar fallback

The final kernel ablations include the exact production sources with checked wrappers. The table reports geometric mean `direct scalar latency / public zhex latency` over the same 32 primary cases for each operation, across all offsets. Generic scalar still benefits from compiler auto-vectorization; native gives the compiler additional target instructions.

| Build | Lower encode | Upper encode | Mixed decode |
| --- | ---: | ---: | ---: |
| generic | 1.886× | 1.784× | 4.793× |
| native | 1.100× | 1.063× | 5.146× |

Evidence: [generic full kernel sweep](results/final-kernels/summary.csv), [native primary kernel sweep](results/final-native-kernels/summary.csv). Native reduces the incremental encoding benefit of hand-written SIMD, while decode keeps a substantial advantage over the LUT loop. These gains are against zhex’s own scalar path and do not satisfy the strongest-competitor 2× target.

### Short-input cutoff and warm dispatch

On x86 the public path avoids AVX2 entry below 32 input bytes: encoding payloads below 32 bytes and decoding payloads below 16 bytes. The no-short wrapper performs the same safe preflight and CPU/OS check, then enters AVX2 even for tiny inputs. It therefore measures the combined effect of the cutoff, cached detection and kernel entry within one compilation.

| Operation | Payload range | Cases | No-short latency / public latency, geometric mean | Cases where public was slower |
| --- | --- | ---: | ---: | ---: |
| encode_lower | 0–31 B | 128 | 1.056× | 38 |
| encode_upper | 0–31 B | 128 | 1.084× | 21 |
| decode_mixed | 0–15 B | 64 | 1.139× | 7 |

The cutoff had a modest aggregate benefit with individual regressions; the table explicitly retains them. Native compilation folds feature detection away, but also changes ordinary code generation. No universal or isolated nanosecond saving is attributed to dispatch alone. Warmup performs initial detection before timing, so these results exclude cold CPUID/XGETBV initialization.

### AVX2 decode variants

The kept 32-character decoder computes validated nibbles as `(text & 15) + (is_letter & 9)`. The preceding implementation used separate digit/letter value selection. The controlled primary kernel runs measured a 1.381× geometric mean public decode improvement across all 32 size/offset cases: [original](results/kernels-baseline/summary.csv) and [revised32](results/kernels-revised32/summary.csv), with exact source archives beside each.

A 64-character packing variant was also implemented and tested, then rejected. The ratio `revised32 latency / revised64 latency` was 0.992× across the same primary decode cases; at offset zero, 32 B increased from 9.75 to 13.18 ns and 64 B from 11.13 to 16.90 ns. Some large cases improved and others regressed. All [revised64 results](results/kernels-revised64/summary.csv), including its source archive, remain available. The simpler revised32 kernel is the delivered implementation. These sequential variant timings are exploratory shared-host evidence, not a precise hardware-wide guarantee.

## Fallbacks, rejected input and scope limits

- Production falls back to the arithmetic/LUT implementation when SIMD is disabled, on unsupported architectures, under Miri, on excluded x86 SGX targets, or when x86 CPU/OS state does not support AVX2. The x86 check verifies CPUID, XSAVE/OSXSAVE/AVX, XCR0 XMM/YMM state and AVX2; the cached successful state is used only after those checks.
- AArch64 NEON is selected only when the compilation target guarantees it. Its public cutoff is 16 input bytes for encode and 32 input bytes for decode. ARM correctness was exercised by CI; this report contains **no ARM performance claim** from benchmark smoke.
- Partial vector tails use the scalar path with bounded slices. The full 0–64 and boundary sweep retains their costs instead of truncating inputs to favorable vector lengths.
- Invalid-input first/middle/last timings remain in all full raw/summary files under group `invalid`. Different native library error policies, including transactional writes and full-input checks, affect those timings. They are not included in the valid-input performance target.
- Buffers are repeatedly warmed and reused. No cold-cache, multi-thread scaling, memory-controller counter or independent memory-copy ceiling experiment was performed. Therefore no failure to reach 2× is attributed to a proven memory-bandwidth limit.
- This is one shared x86 cloud environment. Scheduling variation and code layout affect small differences. Per-sample ranges are retained and no performance threshold is enforced on shared CI runners.

## Reproduction and full evidence

The [runner instructions](README.md) explain generic/native/scalar and candidate/kernel invocations. Each final directory contains `raw.csv.gz`, `summary.csv`, `metadata.json`, `source.tar.gz`, the Cargo build report and a run log. The runner refuses to overwrite evidence, checks that sources did not change during compilation, records the executable SHA-256 and executes that exact binary. The source snapshots reproduce the measured code even after documentation changes.

| Run | Scope | Measured samples | Raw data | Provenance |
| --- | --- | ---: | --- | --- |
| final-generic | full competition | 58,184 | [CSV.gz](results/final-generic/raw.csv.gz) | [metadata](results/final-generic/metadata.json) / [source](results/final-generic/source.tar.gz) |
| final-native | full competition | 58,184 | [CSV.gz](results/final-native/raw.csv.gz) | [metadata](results/final-native/metadata.json) / [source](results/final-native/source.tar.gz) |
| final-scalar | full scalar ablation | 58,184 | [CSV.gz](results/final-scalar/raw.csv.gz) | [metadata](results/final-scalar/metadata.json) / [source](results/final-scalar/source.tar.gz) |
| final-kernels | full kernel/cutoff ablation | 22,680 | [CSV.gz](results/final-kernels/raw.csv.gz) | [metadata](results/final-kernels/metadata.json) / [source](results/final-kernels/source.tar.gz) |
| final-native-kernels | native primary kernel ablation | 4,032 | [CSV.gz](results/final-native-kernels/raw.csv.gz) | [metadata](results/final-native-kernels/metadata.json) / [source](results/final-native-kernels/source.tar.gz) |

An independent recomputation checked sample counts, coverage, median/rate arithmetic, primary scores, source archives and binary hashes: [benchmark audit](../validation/benchmark-audit.json). No final raw timing samples or losing cases were removed.

## Evidence check

| Dimension | Finding | Evidence / status |
| --- | --- | --- |
| Contribution | The implemented codec offers the requested constrained API; the 2× leadership goal remains unmet. | Public source/tests and the complete 96-case scorecards; supported constraint claim, rejected speed claim. |
| Clarity | Short latency and large throughput use explicit units; payload bytes mean the same thing for both directions. | Protocol, table captions and raw counts; supported. |
| Experimental strength | Seven current checked baselines were included, with the fastest eligible baseline selected per case. | Locked versions, registry checksums and full scorecards; no broad win claim. |
| Evaluation completeness | Five final builds/ablations cover offsets, boundaries, large input and rejection paths; ARM timing and cold dispatch are outside the measured scope. | Full artifacts and stated limitations; supported within scope. |
| Method soundness | Preallocation and independent poisoned-output checks preserve comparable work; SIMD choices have retained ablations, including the rejected 64-character variant. | Harness, source snapshots and independent audit; supported with shared-host timing limitations. |
