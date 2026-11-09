# Measurement summary

Raw file: `raw.csv.gz`. 9392 measured combinations; 328 explicitly unsupported combinations. Every sample is retained.

All rates use payload bytes. The CSV files retain every size, offset and baseline. Shared cloud timing is exploratory; these numbers are not a CI gate.

Primary target **not met**: geometric mean speedup **0.794×** against the fastest baseline per case; 0/96 cases reach 2×. Range: 0.326–1.804×. All alignments are included.

| Operation | Geometric mean speedup | Cases ≥2× | Slowest ratio |
| --- | ---: | ---: | ---: |
| decode_mixed | 0.724× | 0/32 | 0.326× |
| encode_lower | 0.862× | 0/32 | 0.497× |
| encode_upper | 0.801× | 0/32 | 0.358× |

`fastest.csv` lists the full primary scorecard. A ratio below 1 means the baseline is faster. No slow samples or losing cases are removed.

Same-process previous-zhex comparison: primary geometric mean **1.055×**; 40/96 cases below parity. This historical implementation is excluded from competitive scores. `previous.csv` retains all valid and invalid cases, including regressions.
