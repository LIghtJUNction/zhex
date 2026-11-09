# Measurement summary

Raw file: `raw.csv.gz`. 9392 measured combinations; 328 explicitly unsupported combinations. Every sample is retained.

All rates use payload bytes. The CSV files retain every size, offset and baseline. Shared cloud timing is exploratory; these numbers are not a CI gate.

Primary target **not met**: geometric mean speedup **0.783×** against the fastest baseline per case; 0/96 cases reach 2×. Range: 0.251–1.080×. All alignments are included.

| Operation | Geometric mean speedup | Cases ≥2× | Slowest ratio |
| --- | ---: | ---: | ---: |
| decode_mixed | 0.716× | 0/32 | 0.251× |
| encode_lower | 0.794× | 0/32 | 0.320× |
| encode_upper | 0.844× | 0/32 | 0.422× |

`fastest.csv` lists the full primary scorecard. A ratio below 1 means the baseline is faster. No slow samples or losing cases are removed.

Same-process previous-zhex comparison: primary geometric mean **1.012×**; 39/96 cases below parity. This historical implementation is excluded from competitive scores. `previous.csv` retains all valid and invalid cases, including regressions.
