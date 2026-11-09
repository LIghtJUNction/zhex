# Initial tuning evidence

`candidates-initial` and `generic-primary-initial` are exploratory development
runs. They retain every raw timing sample, the then-current source hashes,
compiler/CPU information and the commit beneath uncommitted implementation
changes. They preceded source-archive capture, poisoned-buffer checks for large
cases, and execution of the previously built binary without a second Cargo
invocation. Their adapter checks nevertheless covered all 65,536 input-byte
pairs and compared results with an independent reference.

`candidates-initial` used `--features force-scalar -- --candidates --primary`.
Its original metadata query omitted the feature flag, so the `resolved_features`
field describes a default metadata query rather than the actual scalar build.
This capture issue is fixed in the runner. Its candidate timings motivated the
arithmetic encoder and LUT decoder, but final committed-source measurements
are required for published performance conclusions.

`generic-primary-initial` used generic target defaults and `--primary` on CPU 0.
The initial SIMD implementation missed the 2× target; those losing results are
retained. It used the original, more expensive AVX2 nibble conversion sequence.
Later optimizations and benchmark changes do not overwrite either run.
