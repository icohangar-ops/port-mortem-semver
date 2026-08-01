#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
python3 - <<'PY'
import hashlib, pathlib
r = pathlib.Path("tests/original")
now = "\n".join(
    f"{hashlib.sha256(p.read_bytes()).hexdigest()}  ./{p.relative_to(r)}"
    for p in sorted(r.rglob("*"))
    if p.is_file()
) + "\n"
ok = now == pathlib.Path("tests/original.SHA256SUMS").read_text()
print("HASHES MATCH — originals untouched" if ok else "HASH MISMATCH")
raise SystemExit(0 if ok else 1)
PY
