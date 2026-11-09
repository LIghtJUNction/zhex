# Benchmark protocol (fixed before timing)

The goal is at least 2× lower latency than the fastest eligible baseline on the
primary workload below. Every case remains in the result, including regressions.
The headline statistic is the geometric mean of per-case speedups against the
fastest competitor for that case. Report each case and the number reaching 2×;
a broad 2× claim additionally requires no primary case below parity.

## Workload

- Primary payload sizes: 16, 32, 64, 256, 4,096, 65,536, 1,048,576 and 8,388,608
  bytes. Primary operations: lower encode, upper encode, mixed-case decode.
- Input/output offsets modulo 64: (0, 0), (1, 3), (7, 15), (31, 1). The harness
  aligns backing allocations to 64 bytes before applying each offset, with safe
  slice indexing. All libraries use the very same input and output addresses.
- Coverage sweep: every payload size 0–64; 65, 95, 96, 97, 127, 128, 129, 255,
  256, 257, 511, 512, 513; and the large primary sizes.
- Decode rejection: first, middle and last input byte invalid at all primary
  sizes. These are reported separately and never mixed into valid-input scores.
- Fixed xorshift64 seed: `0x4d595df4d0f33173`. Mixed case alternates by input
  character position. Invalid samples replace a character with `0x80`.

## Fairness and measurement

All codecs write to initialized, preallocated byte slices. No allocation, random
generation, output clearing, output validation or I/O is inside a timed loop.
All decoded bytes are validated outside timing against an independent reference;
each error is checked against its first invalid input index. Calls, inputs,
outputs and results pass through `std::hint::black_box`. The same function-pointer
call mechanism is used for every adapter. Buffers are warmed and reused: these
are hot-buffer results, not a cold-cache or NUMA benchmark.

Each case/library warms up for at least 1 ms, calibrates repetitions to a 2 ms
sample, then records seven unfiltered samples. The library order rotates between
samples. Slow operations always execute at least once. Raw CSV contains elapsed
nanoseconds and iteration counts; summaries use sample medians and report range.
There is no benchmark subtraction. Timer/function-call overhead is included,
particularly relevant at 0–8 bytes. Shared/cloud CPU timings are exploratory and
are never a CI performance gate.

Throughput is **payload GiB/s** (`payload_bytes / seconds / 2^30`): input bytes
for encode, decoded output bytes for decode. Successful conversion reads+writes
three times that many bytes. Failed decodes report ns/op, never throughput.

Every competitor uses checked slice APIs, the same release profile, the same
toolchain and the same `RUSTFLAGS`. The adapters give them the zhex error order
and output-length convention. Libraries without an error index scan for the
first invalid byte **only after their checked decoder returns an error**.
`const-hex`'s accepted `0x`/`0X` prefix is rejected by its adapter. `hex` has no
uppercase nonallocating slice API, so upper encoding is explicitly unsupported
and excluded for that library, without synthesizing an allocating baseline.

## Builds and ablations

Run generic target defaults, `-C target-cpu=native`, and zhex `force-scalar` as
separate builds. `force-scalar` disables zhex's hand-written SIMD only; ordinary
compiler optimization remains enabled, so this is not a claim that machine
code contains no SIMD. Competitor defaults retain their runtime dispatch.
`--candidates` measures safe prototype arithmetic, pair-LUT and SWAR encoders;
these prototypes are not competitors or public APIs.

The additional `--kernels` ablation includes the actual production scalar and
SIMD source files only in the comparison binary. Each wrapper repeats safe
length/capacity and CPU/OS checks, but always enters an available SIMD backend
even below its public short-input cutoff. This compares short-path benefit on
the same compile target. Native compilation folds feature detection away; its
effect is reported alongside the compiler's other native-code changes, not
attributed entirely to dispatch. Unsupported SIMD hardware is explicitly logged
and uses production's scalar fallback. This ablation was added after initial
scalar exploration and before any kernel-ablation timings; primary cases remain
unchanged.

`--smoke` reduces coverage to 0, 1, 15, 16, 31, 32, 63, 64, 65, 256 and 4,096
bytes at two offsets, uses two 50 μs samples with a 20 μs warmup, and still checks
outputs and errors. Its timings are not evidence for performance claims.

Save exact crate versions, checksums, source commit IDs where published, CPU,
OS, compiler, compile flags, source commit and working-tree state beside raw
results. If a dependency cannot build, keep the failure log and label it
unavailable; do not silently change its version or delete its cases.
