# Measurement summary

Raw file: `raw.csv.gz`. 1504 measured combinations; 32 explicitly unsupported combinations. Every sample is retained.

All rates use payload bytes. The CSV files retain every size, offset and baseline. Shared cloud timing is exploratory; these numbers are not a CI gate.

Primary target **not met**: geometric mean speedup **0.547×** against the fastest baseline per case; 0/96 cases reach 2×. Range: 0.061–1.140×. All alignments are included.

| Operation | Geometric mean speedup | Cases ≥2× | Slowest ratio |
| --- | ---: | ---: | ---: |
| decode_mixed | 0.436× | 0/32 | 0.139× |
| encode_lower | 0.653× | 0/32 | 0.061× |
| encode_upper | 0.575× | 0/32 | 0.100× |

`fastest.csv` lists the full primary scorecard. A ratio below 1 means the baseline is faster. No slow samples or losing cases are removed.
