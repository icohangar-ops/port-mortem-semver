#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
python3 - <<'PY'
import json
r = json.load(open("bench/results.json"))
print("startup version-check:", r["startup"]["version-check"]["speedup"], "x")
print("throughput satisfies:", r["throughput"]["satisfies"]["speedup"], "x")
print(
    "byte-identical stdout:",
    r["throughput"]["sort"]["sameOutput"],
    r["throughput"]["satisfies"]["sameOutput"],
)
PY
