#!/usr/bin/env python3
"""Summarize every raw sample and compare each primary case to its fastest baseline."""
import csv
import gzip
from collections import defaultdict
from pathlib import Path
import math
import statistics
import sys

raw = Path(sys.argv[1])
destination = Path(sys.argv[2]) if len(sys.argv) > 2 else raw.parent
destination.mkdir(parents=True, exist_ok=True)
columns = ["build", "group", "operation", "payload_bytes", "input_offset", "output_offset", "implementation"]
groups = defaultdict(list)
unsupported = set()
with (gzip.open(raw, "rt") if raw.suffix == ".gz" else raw.open()) as handle:
    for row in csv.DictReader(handle):
        key = tuple(row[c] for c in columns)
        if row["status"] == "unsupported":
            unsupported.add(key)
        elif row["status"] == "ok":
            groups[key].append(float(row["ns_per_op"]))
summaries = []
for key, samples in groups.items():
    row = dict(zip(columns, key))
    median = statistics.median(samples)
    payload = int(row["payload_bytes"])
    row.update(samples=len(samples), median_ns=f"{median:.6f}", min_ns=f"{min(samples):.6f}", max_ns=f"{max(samples):.6f}", payload_gib_s="" if row["group"] == "invalid" else f"{payload / median * 1e9 / 2**30:.6f}")
    summaries.append(row)
with (destination / "summary.csv").open("w") as handle:
    writer = csv.DictWriter(handle, fieldnames=columns + ["samples", "median_ns", "min_ns", "max_ns", "payload_gib_s"])
    writer.writeheader()
    writer.writerows(summaries)
cases = defaultdict(dict)
for row in summaries:
    key = tuple(row[c] for c in columns[:-1])
    cases[key][row["implementation"]] = float(row["median_ns"])
comparisons = []
for key, results in cases.items():
    if key[1] != "primary" or "zhex" not in results:
        continue
    baselines = {k: v for k, v in results.items() if k != "zhex" and not k.startswith(("candidate-", "kernel-", "previous-"))}
    if not baselines:
        continue
    fastest = min(baselines, key=baselines.get)
    row = dict(zip(columns[:-1], key))
    row.update(fastest_baseline=fastest, baseline_ns=f"{baselines[fastest]:.6f}", zhex_ns=f"{results['zhex']:.6f}", speedup=f"{baselines[fastest] / results['zhex']:.6f}")
    comparisons.append(row)
with (destination / "fastest.csv").open("w") as handle:
    writer = csv.DictWriter(handle, fieldnames=columns[:-1] + ["fastest_baseline", "baseline_ns", "zhex_ns", "speedup"])
    writer.writeheader()
    writer.writerows(comparisons)
previous = []
for key, results in cases.items():
    if "zhex" in results and "previous-zhex" in results:
        row = dict(zip(columns[:-1], key))
        row.update(previous_ns=f"{results['previous-zhex']:.6f}", zhex_ns=f"{results['zhex']:.6f}", speedup=f"{results['previous-zhex'] / results['zhex']:.6f}")
        previous.append(row)
if previous:
    with (destination / "previous.csv").open("w") as handle:
        writer = csv.DictWriter(handle, fieldnames=columns[:-1] + ["previous_ns", "zhex_ns", "speedup"])
        writer.writeheader()
        writer.writerows(previous)
encoding_ab = []
for key, results in cases.items():
    if key[2] in {"encode_lower", "encode_upper"} and "kernel-avx2-no-short" in results and "candidate-pointer-avx2" in results:
        row = dict(zip(columns[:-1], key))
        indexed, pointer = results["kernel-avx2-no-short"], results["candidate-pointer-avx2"]
        row.update(indexed_ns=f"{indexed:.6f}", pointer_ns=f"{pointer:.6f}", speedup=f"{indexed / pointer:.6f}")
        encoding_ab.append(row)
if encoding_ab:
    with (destination / "encoding-ab.csv").open("w") as handle:
        writer = csv.DictWriter(handle, fieldnames=columns[:-1] + ["indexed_ns", "pointer_ns", "speedup"])
        writer.writeheader()
        writer.writerows(encoding_ab)
lines = ["# Measurement summary", "", f"Raw file: `{raw.name}`. {len(summaries)} measured combinations; {len(unsupported)} explicitly unsupported combinations. Every sample is retained.", "", "All rates use payload bytes. The CSV files retain every size, offset and baseline. Shared cloud timing is exploratory; these numbers are not a CI gate."]
if comparisons:
    speedups = [float(x["speedup"]) for x in comparisons]
    geometric_mean = math.exp(statistics.mean(math.log(x) for x in speedups))
    reached = sum(x >= 2 for x in speedups)
    result = "met" if geometric_mean >= 2 and min(speedups) >= 1 else "not met"
    lines += ["", f"Primary target **{result}**: geometric mean speedup **{geometric_mean:.3f}×** against the fastest baseline per case; {reached}/{len(speedups)} cases reach 2×. Range: {min(speedups):.3f}–{max(speedups):.3f}×. All alignments are included.", "", "| Operation | Geometric mean speedup | Cases ≥2× | Slowest ratio |", "| --- | ---: | ---: | ---: |"]
    for operation in sorted({x["operation"] for x in comparisons}):
        values = [float(x["speedup"]) for x in comparisons if x["operation"] == operation]
        mean = math.exp(statistics.mean(math.log(v) for v in values))
        lines.append(f"| {operation} | {mean:.3f}× | {sum(v >= 2 for v in values)}/{len(values)} | {min(values):.3f}× |")
    lines += ["", "`fastest.csv` lists the full primary scorecard. A ratio below 1 means the baseline is faster. No slow samples or losing cases are removed."]
if previous:
    values = [float(x["speedup"]) for x in previous if x["group"] == "primary"]
    mean = math.exp(statistics.mean(math.log(v) for v in values))
    lines += ["", f"Same-process previous-zhex comparison: primary geometric mean **{mean:.3f}×**; {sum(v < 1 for v in values)}/{len(values)} cases below parity. This historical implementation is excluded from competitive scores. `previous.csv` retains all valid and invalid cases, including regressions."]
if encoding_ab:
    values = [float(x["speedup"]) for x in encoding_ab if x["group"] == "primary"]
    mean = math.exp(statistics.mean(math.log(v) for v in values))
    lines += ["", f"Pointer-loop A/B: primary encoding geometric mean indexed/pointer ratio **{mean:.3f}×**; {sum(v < 1 for v in values)}/{len(values)} cases slower with the candidate. `encoding-ab.csv` retains all encoding cases. Neither implementation is an external competitor."]
(destination / "SUMMARY.md").write_text("\n".join(lines) + "\n")
print("\n".join(lines))
