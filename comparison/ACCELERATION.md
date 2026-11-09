# Second optimization pass: measured gains and limits

On the measured Intel Xeon Platinum 8573C, this pass improves the geometric mean
of all 0–64-byte cases against the preceding implementation by **1.278–1.305× in
the generic build** and **1.360–1.386× in the native build**, depending on the
operation. The unchanged 96-case primary target remains **unmet**: against the
fastest eligible external library for each case, the scores are **0.783× generic**
and **0.794× native**, with **zero primary cases reaching 2×**. Short-tail gains
do not establish leadership over the strongest competitors.

These conclusions use the four committed-source runs at
`f69459e8e58db7d761099a00bb161da34ef638ee`. The historical self-comparison is frozen
at `86f569aa72b945073187877323f4a20a6b4f12f6` and runs in the same executable.
Only module import paths and its crate-level attribute were adapted; its codec
logic and error semantics were preserved. Every slower case remains in the raw
results. The [independent audit](../validation/acceleration-audit.json) checks
coverage, timing arithmetic, medians, both scorecards, archived source and binary
provenance. The [first-pass report](PERFORMANCE.md) remains available separately.

## What changed and why

**Bounded overlapping tails.** Encoding and decoding used to finish incomplete
vector chunks with scalar work. The new path processes a final full vector
ending exactly at the end of the slice. It rereads an already processed prefix,
then handles the remaining bytes. Every read and write stays within the caller's
slices; there is no padding assumption. Encoding writes identical values over
its overlap. Decoding first validates the whole vector, and the overlapped
prefix was already valid, so the first invalid index remains the original input
position. No output from a failing vector is stored.

**A shorter encoding entry.** On supported x86 CPUs, AVX2 encoding begins at
16 input bytes. A widening operation maps 16 input bytes into 32 nibble positions
and one table shuffle emits their text. The earlier 32-byte main loop is retained
for bulk input. An experiment using the widening path throughout the bulk loop
did not produce a consistent gain and was rejected.

**SSE2 short decoding and two-table AVX2 decoding.** Targets that guarantee SSE2
use its checked 16-character decoder for short input. AVX2 handles 64-character
groups using a subtract-one hash and two delta tables. Subtracting one places
`0`, `1`–`9`, `A`–`F` and `a`–`f` in four useful high-nibble buckets. One table
marks invalid bytes; the other yields nibble values. The two vector check masks
are combined before stores. On failure, the first vector mask is examined first,
then the second, preserving source order without rescanning the whole input.

The tables follow Daniel Lemire's [public-domain base16 example](https://github.com/lemire/Code-used-on-Daniel-Lemire-s-blog/blob/23fa12e7a1f705d3e6fa9589b6d573b078c30618/2023/07/27/src/base16.c)
and [article](https://lemire.me/blog/2023/07/27/decoding-base16-sequences-quickly/).
Its provenance is recorded in the source and [LICENSE](../LICENSE). The Rust
slice boundaries, dispatch, group error handling and tails are implemented here.

The scalar algorithms, public API, validation rules, MSRV and dependency policy
are unchanged. Rust 1.70.0 release tests, stable generic/native release tests,
rustfmt and Clippy passed after the final code change. The local logs are
[MSRV](results/round2-final-msrv.log), [stable](results/round2-final-stable.log),
[native](results/round2-final-native.log) and [Clippy](results/round2-final-clippy.log).
These runs cover 14 reference-based integration tests, concurrent dispatch,
unit checks and the documentation example; target-specific unit-test counts vary
when native features make runtime detection unnecessary.

## Fixed workload and environment

The [protocol](PLAN.md) retains the original eight primary payload sizes
(16, 32, 64, 256, 4,096, 65,536, 1,048,576 and 8,388,608 bytes), three successful
operations and four input/output offsets: `(0,0)`, `(1,3)`, `(7,15)`, `(31,1)`.
That is 96 primary cases. The full sweep still includes every size 0–64, all
specified vector-boundary neighbors, large inputs and first/middle/last invalid
bytes. No workload or validity requirement was removed to improve the score.

Hardware and software are recorded in each `metadata.json`: Intel Xeon Platinum
8573C; Linux 6.18.57, glibc 2.41; x86-64; Rust 1.99.1 (LLVM 23.1.1); CPU affinity
`[0]`. Both builds use optimization level 3, fat LTO, one codegen unit and abort
on panic. Generic has empty `RUSTFLAGS`; native has `-C target-cpu=native` for
all linked libraries. The published crate still supports Rust 1.70.0; comparison
dependencies are built only in their separate workspace with the recorded newer
toolchain.

All adapters use checked, preallocated slice APIs. Input generation, allocation,
output poisoning and reference checks happen outside timing. A fixed seed,
at least 1 ms of warmup, calibrated 2 ms samples, seven unfiltered samples,
rotating implementation order and `black_box` are used. CPU feature caches are
warm; the first CPUID discovery is not timed. Buffers are reused and warm.
`const-hex`'s prefix extension is rejected by its adapter; libraries without an
error position are scanned only after their checked decoder returns an error.
The adapter suite independently checks all 65,536 two-character byte pairs.

The [official registry recheck](results/round2-registry-check.json) at
2026-11-09 16:37:40 UTC confirms the same latest stable versions: hex 0.4.3,
faster-hex 1.0.0, const-hex 1.19.3, hex-simd 0.8.0, better-hex 1.0.1,
fashex 0.1.1 and hex-turbo 0.4.0. [Registry provenance](results/registry.json)
records their checksums, licenses and published VCS commits. The final comparison
retains each library's normal dispatch on this CPU. `hex` lacks a nonallocating
uppercase slice API; its 328 unsupported combinations are explicitly retained
per full run.

## The unchanged primary score

A speedup is `baseline median ns / zhex median ns`. A ratio below 1 is slower
for zhex. The external score chooses the fastest eligible competitor separately
for every case. The previous implementation and internal kernels are excluded.

| Build | Fastest external GM | External losses | External cases ≥2× | Previous-zhex GM | Previous losses |
| --- | ---: | ---: | ---: | ---: | ---: |
| generic | 0.783× | 86/96 | 0/96 | 1.012× | 39/96 |
| native | 0.794× | 80/96 | 0/96 | 1.055× | 40/96 |

The generic competitive range is 0.251–1.080×; native is 0.326–1.804×. Full
case-by-case results are [generic fastest](results/round2-final-generic/fastest.csv),
[native fastest](results/round2-final-native/fastest.csv),
[generic previous](results/round2-final-generic/previous.csv) and
[native previous](results/round2-final-native/previous.csv). The primary results
by operation are:

| Operation | Generic external | Generic previous | Native external | Native previous |
| --- | ---: | ---: | ---: | ---: |
| Lower encode | 0.794× | 0.934× | 0.862× | 1.068× |
| Upper encode | 0.844× | 0.987× | 0.801× | 0.997× |
| Mixed decode | 0.716× | 1.123× | 0.724× | 1.103× |

This is a modest aggregate primary improvement over the prior implementation,
with regressions in both builds. Generic lower encoding is 0.934× against its
predecessor, despite using the same bulk vector operations. Its 4 KiB cases are
0.717× in aggregate; the corresponding native result is 0.948×. Upper encoding
is 0.828× at generic 4 KiB and 1.083× at native 4 KiB. These losses remain in the
score. Inspection found no register spills or extra bulk vector operations, but
LLVM selected indexed stores where the older generic loop used incremented
pointers. The separate pointer experiment below did not justify a production
change. Neither timing variation nor that compiler difference is proven to
explain every regression.

The fastest baseline varies by case. In generic, hex-turbo wins 24 cases,
fashex 23, hex-simd 22, better-hex 12, faster-hex 10 and const-hex 5. In native,
hex-turbo wins 25, better-hex 21, fashex 18, faster-hex 14, hex-simd 10 and
const-hex 8. The reported target does not use the slower `hex` result as a proxy
for the strongest implementation.

## Short inputs: gains, latency and retained regressions

The following uses **all 260 valid cases per operation** at payload lengths
0–64 and all four offsets, compared in the same process with previous-zhex.

| Build | Operation | Previous/new GM | Cases below parity |
| --- | ---: | ---: | ---: |
| generic | Lower encode | 1.278× | 53/260 |
| generic | Upper encode | 1.301× | 47/260 |
| generic | Mixed decode | 1.305× | 34/260 |
| native | Lower encode | 1.386× | 58/260 |
| native | Upper encode | 1.360× | 58/260 |
| native | Mixed decode | 1.386× | 34/260 |

The tail mechanism's effect is visible when lengths are grouped around the
vector boundaries. These are additional coverage results, not replacements for
the fixed primary score:

| Payload bytes | Generic lower | Generic upper | Generic decode | Native lower | Native upper | Native decode |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 0–7 | 0.989× | 1.028× | 1.069× | 1.083× | 1.095× | 1.093× |
| 8–15 | 0.983× | 1.062× | 1.227× | 1.038× | 1.042× | 1.403× |
| 16–31 | 1.292× | 1.349× | 1.372× | 1.453× | 1.460× | 1.507× |
| 32–47 | 1.364× | 1.377× | 1.314× | 1.571× | 1.404× | 1.488× |
| 48–63 | 1.552× | 1.482× | 1.417× | 1.558× | 1.591× | 1.346× |

For absolute latency, the following cells show **ns/op ranges of the four
per-offset medians** for current zhex. They are not confidence intervals; every
raw sample and the per-case minimum/maximum is preserved in `summary.csv`.

| Payload bytes | Generic lower | Generic upper | Generic decode | Native lower | Native upper | Native decode |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 0 | 7.52–8.90 | 7.78–8.59 | 7.75–8.84 | 7.39–8.23 | 7.18–7.88 | 6.89–8.04 |
| 8 | 8.73–11.85 | 8.52–14.19 | 12.78–15.83 | 8.76–10.58 | 8.48–9.84 | 8.93–9.88 |
| 15 | 12.57–13.85 | 13.18–13.86 | 11.08–14.00 | 17.94–28.28 | 10.81–24.36 | 9.41–18.68 |
| 16 | 8.54–10.14 | 8.36–9.68 | 9.42–18.63 | 7.58–9.50 | 7.04–16.70 | 8.39–21.39 |
| 31 | 9.06–9.44 | 9.05–10.55 | 10.32–11.99 | 7.24–20.58 | 7.70–17.37 | 8.08–10.57 |
| 32 | 9.55–15.08 | 9.42–11.57 | 9.32–12.08 | 7.11–9.56 | 8.71–16.52 | 7.53–13.15 |
| 63 | 10.68–11.31 | 10.62–17.26 | 12.04–17.25 | 9.14–9.53 | 8.77–10.83 | 10.62–11.95 |
| 64 | 10.70–12.73 | 10.13–13.31 | 11.45–15.88 | 8.98–9.74 | 8.71–10.48 | 9.61–11.24 |

The unchanged code on some tiny paths also moves in these measurements. That,
along with wide sample ranges, limits interpretation of small differences on
this shared cloud host. The consistent broad short-tail gains are the supported
result; a universal improvement at every length or alignment is not established.

## Large-input throughput

All throughput uses **payload GiB/s**: original byte input for encode, decoded
byte output for decode. A successful operation transfers three times that many
bytes when its reads and writes are added. The following cells show ranges of
per-offset median rates for current zhex, with the same denominator throughout.

| Payload bytes | Generic lower | Generic upper | Generic decode | Native lower | Native upper | Native decode |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 4 KiB | 8.67–22.94 | 10.30–25.54 | 8.06–13.18 | 18.81–21.95 | 11.68–23.81 | 6.34–12.89 |
| 64 KiB | 14.53–19.80 | 14.78–17.65 | 2.82–11.42 | 15.53–18.15 | 15.57–18.54 | 9.45–11.17 |
| 1 MiB | 7.07–9.14 | 4.16–9.15 | 8.06–9.35 | 8.13–9.59 | 7.40–8.46 | 6.61–7.98 |
| 8 MiB | 7.03–8.25 | 6.36–7.95 | 6.54–8.03 | 6.00–8.09 | 6.26–7.54 | 6.12–7.67 |

Across the 16 primary decode cases at 4 KiB and above, generic old/new GM is
1.138× with one case below parity; native is 1.030× with five below parity.
No independent bandwidth-ceiling experiment was performed, so these results do
not establish memory-bandwidth saturation. They also provide no ARM throughput
claim; AArch64 runner timing is only a functional smoke test.

## Invalid input and error cost

Length and capacity errors still return before writing. Each vector group's
validity is checked before its output stores. The first invalid byte is reported
in original input coordinates; earlier valid groups may already have written
output. Invalid-input cases remain outside the successful-throughput score.
The following old/new GMs cover all eight primary sizes and four offsets.

| Invalid position | Generic ratio | Generic losses | Native ratio | Native losses |
| --- | ---: | ---: | ---: | ---: |
| First invalid | 0.863× | 28/32 | 0.969× | 11/32 |
| Middle invalid | 0.997× | 16/32 | 1.096× | 9/32 |
| Last invalid | 1.067× | 12/32 | 1.199× | 6/32 |

Early invalid input is a recorded tradeoff: generic first-invalid GM is 0.863×,
with 28 of 32 cases slower. The larger validated group can do more work before
rejecting its first byte. Invalid timing uses ns/op only, since the number of
successfully decoded bytes is not the nominal payload length. All absolute
invalid latencies are in the linked full `summary.csv` files.

## Scalar, SIMD, dispatch and fallback evidence

The kernel ablation uses the exact production source through checked wrappers.
It preserves capacity, CPU/OS and invalid-byte checks. The following ratio is
`internal scalar median / public median` on the 32 primary cases per operation;
it measures benefit over this library's own scalar implementation.

| Build | Operation | Scalar/public GM | Public losses |
| --- | ---: | ---: | ---: |
| generic | Lower encode | 1.944× | 1/32 |
| generic | Upper encode | 1.899× | 0/32 |
| generic | Mixed decode | 5.131× | 0/32 |
| native | Lower encode | 1.118× | 6/32 |
| native | Upper encode | 1.079× | 9/32 |
| native | Mixed decode | 5.377× | 0/32 |

On generic payloads 0–15, bypassing the unconditional AVX2 entry gives public
speedups of 1.368× lower encode, 1.354× upper encode and 1.064× decode. Across
all 192 such cases the GM is 1.254×, with 20 cases below parity. The source's
short cutoff is therefore retained. Native folds feature detection away and
also changes compiler optimization; its differences are not attributed solely
to dispatch.

The scalar implementation did not change, so the previous full force-scalar
measurement is preserved in [final-scalar](results/final-scalar/), with its older
source commit clearly identified. The new kernel runs measure the current
scalar against the current public path. Fallback routing remains explicit:
on x86 without runtime AVX2, encoding uses scalar; decoding can use compile-target
SSE2, otherwise scalar. AArch64 uses NEON only when the target guarantees it;
other targets, `force-scalar`, Miri and SGX exclusion take scalar paths. CI tests
and compile checks verify those routes separately from host timing. No physical
non-AVX2 or ARM performance result is inferred from this AVX2 host.

## Rejected experiments remain available

- [Initial SSE2 short experiment](results/round2-sse2-short/): decoding at 8–15
  payload bytes improved by 1.253× over the previous implementation. Encoding
  had no consistent gain, so its proposed SSE2 encoder was removed.
- [Expanded encoder and arithmetic 128-character decode group](results/round2-expanded128-primary/):
  valid primary ratios were 0.978× lower, 0.949× upper and 1.001× decode. Scalar
  recovery on a failed 128-character group made late errors particularly slow.
  This design was rejected. Emitted code also showed register pressure and
  repeated constant loads in that arithmetic batch.
- [Two-table 64-character exploration](results/round2-table64-primary/) and
  [short sweep](results/round2-table64-short/) supported the simpler group and
  bounded tails. The final encoder restores the original 32-byte bulk loop.
  [Short kernel ablation](results/round2-overlap-kernels-short/) supported moving
  its AVX2 entry to 16 bytes; final full runs test that actual public cutoff.
- [Pointer-induction A/B](results/round2-pointer-ab/) was fixed after the four
  final runs to investigate the concrete indexed-address difference. It keeps
  the production preflight and AVX2 checks and changes only the benchmark copy's
  encoder loop induction. Across 64 primary encoding cases its indexed/pointer
  GM is **0.997×**, with 29 cases slower using the candidate: 1.047× lower but
  0.949× upper. Short results are 0.989× lower and 1.017× upper. LLVM emitted
  indexed stores in both versions; [both assembly extracts](results/round2-pointer-ab/)
  and all 15,120 samples are saved. The candidate is rejected and production
  remains at the measured `f69459e8` source.

Exploratory results explain choices. The four committed-source final runs supply
the delivery performance conclusions; none of the rejected samples is deleted.

## Artifacts and reproduction

The four final runs contain **158,200 timed samples** plus **656 explicit
unsupported rows**. Each full external/self sweep has 65,744 samples and
328 unsupported combinations; full generic kernels have 22,680 samples; native
primary kernels have 4,032. The six second-pass exploratory runs retain another
60,480 samples. These counts exclude validation calls outside the timer.

| Run | Scope | Main evidence |
| --- | --- | --- |
| [Generic](results/round2-final-generic/) | All 82 sizes, four offsets, seven competitors and previous-zhex | raw.csv.gz, summary.csv, fastest.csv, previous.csv, metadata.json, source.tar.gz |
| [Native](results/round2-final-native/) | Identical full matrix, native flags | Same complete evidence set |
| [Generic kernels](results/round2-final-kernels/) | Full 82-size scalar/SIMD/public ablation | raw.csv.gz, summary.csv, metadata.json, source.tar.gz |
| [Native kernels](results/round2-final-native-kernels/) | Original primary sizes and invalid positions | raw.csv.gz, summary.csv, metadata.json, source.tar.gz |

Use the exact measured commit, or extract one of its benchmark source snapshots
into a fresh directory. With Rust 1.99.1 on the path, from the repository root:

```sh
taskset -c 0 comparison/run.sh generic comparison/results/reproduce-generic
taskset -c 0 comparison/run.sh native comparison/results/reproduce-native
taskset -c 0 comparison/run.sh generic comparison/results/reproduce-kernels --kernels
taskset -c 0 comparison/run.sh native comparison/results/reproduce-native-kernels --kernels --primary
```

Use an available logical CPU on another host and record that change. The runner
refuses existing evidence directories, snapshots its source before building,
checks it did not drift during compilation, records the exact executable hash
and executes that binary. Generic and native executable SHA-256 values are
`ab80d24f65497d7bc3e574009615888771e2e08f227925e18577084c014b0f5e` and
`fef2f37ceb8cf88cd9319d95762f32dfd129bc4db40cf3d1eb4c5eb2147feb50` respectively. The comparison
workspace, dependencies, research copies and artifacts remain excluded from the
published crate.

### Evidence review

1. **Question:** the fixed fastest-baseline target and the separate historical
   self-comparison answer different, explicitly labeled questions.
2. **Correctness:** independent references, complete byte-pair tests, poisoned
   outputs and exact invalid positions precede timing; no validity work is dropped.
3. **Controls:** same buffers, offsets, allocation policy, compiler settings and
   throughput denominator are used for every eligible library.
4. **Reproducibility:** immutable commits, source archives, lockfiles, registry
   checks, binary hashes, every sample and independent recomputation are retained.
5. **Limits:** shared-host variation, all regressions, early-error cost, the
   unmet 2× target and unmeasured hardware/bandwidth claims are stated directly.
