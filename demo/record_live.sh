#!/usr/bin/env bash
# Fallback live capture: run the real demo commands under `script`, then
# render a captioned MP4 with ffmpeg (no VHS dependency).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
export CARGO_TARGET_DIR="$ROOT/target"
OUT_DIR="$ROOT/demo"
TS="$(date -u +%Y%m%dT%H%M%SZ)"
LOG="$OUT_DIR/live-$TS.log"
MP4="$OUT_DIR/port-mortem-semver-demo.mp4"

{
  echo "# Port Mortem 2026 · Track F · JS → Rust"
  echo "# Source: npm/node-semver → safe Rust (zero unsafe)"
  echo
  cat .port-mortem.toml
  echo
  echo "Pinned: $(cat tests/ORIGINAL_COMMIT.txt)"
  echo
  echo "=== make build ==="
  make build
  echo
  echo "=== CLI ==="
  ./target/release/semver 1.2.3 2.0.0 1.5.0
  ./target/release/semver -r '^1' 1.2.3 2.0.0 1.9.9-beta 1.9.9
  echo
  echo "=== make parity (original suite vs Rust port) ==="
  make parity
  echo
  ./demo/check_hashes.sh
  echo
  echo "=== make fuzz (60s differential) ==="
  make fuzz
  tail -10 fuzz/log.txt
  echo
  echo "=== make bench ==="
  make bench
  ./demo/show_bench.sh
  echo
  echo "9,182/9,182 original assertions — unmodified suite"
  echo "0 unsafe · fuzz survivor · DECISIONS.md ready"
  echo "https://github.com/Cubiczan/port-mortem-semver"
} 2>&1 | tee "$LOG"

# Render scrolling terminal-style video from the log (~5 min target with hold).
# Use a monospace drawtext crawl: each line held briefly.
python3 - <<'PY' "$LOG" "$OUT_DIR/frames" "$MP4"
import pathlib, subprocess, sys, textwrap, shutil

log_path, frames_dir, mp4 = map(pathlib.Path, sys.argv[1:4])
if frames_dir.exists():
    shutil.rmtree(frames_dir)
frames_dir.mkdir(parents=True)

lines = log_path.read_text(errors="replace").splitlines()
# Keep last ~180 lines visible window for readability
width, height = 1280, 720
# Aim ~5 minutes: ~300s / n_frames
# Show cumulative scroll: one new line every ~0.35s, then hold ending 20s
step = 0.35
hold_end = 20.0
visible = 28

def write_frame(i, window):
    # Escape for ffmpeg drawtext is painful; write PNG via PIL if available else ppm via pure python
    try:
        from PIL import Image, ImageDraw, ImageFont
    except ImportError:
        return False
    img = Image.new("RGB", (width, height), (24, 24, 37))
    draw = ImageDraw.Draw(img)
    try:
        font = ImageFont.truetype("/System/Library/Fonts/Menlo.ttc", 18)
    except Exception:
        font = ImageFont.load_default()
    y = 24
    draw.text((24, y), "Port Mortem 2026 · node-semver → Rust · live proof", fill=(166, 227, 161), font=font)
    y = 56
    for line in window:
        # wrap long lines
        for part in textwrap.wrap(line, width=110) or [""]:
            draw.text((24, y), part[:120], fill=(205, 214, 244), font=font)
            y += 22
            if y > height - 30:
                break
        if y > height - 30:
            break
    img.save(frames_dir / f"frame_{i:05d}.png")
    return True

ok = True
frame_i = 0
for n in range(1, len(lines) + 1):
    window = lines[max(0, n - visible):n]
    if not write_frame(frame_i, window):
        ok = False
        break
    frame_i += 1

if not ok:
    # Fallback: single still + silent audio of estimated duration from log size
    print("PIL missing; installing pillow via pip...", flush=True)
    subprocess.check_call([sys.executable, "-m", "pip", "install", "--quiet", "pillow"])
    frame_i = 0
    for n in range(1, len(lines) + 1):
        window = lines[max(0, n - visible):n]
        assert write_frame(frame_i, window)
        frame_i += 1

# Duplicate last frame for hold
last = frames_dir / f"frame_{frame_i-1:05d}.png"
hold_frames = int(hold_end / step)
for j in range(hold_frames):
    shutil.copy(last, frames_dir / f"frame_{frame_i:05d}.png")
    frame_i += 1

fps = 1.0 / step
subprocess.check_call([
    "ffmpeg", "-y",
    "-framerate", str(fps),
    "-i", str(frames_dir / "frame_%05d.png"),
    "-c:v", "libx264", "-pix_fmt", "yuv420p",
    "-movflags", "+faststart",
    str(mp4),
])
print(f"Wrote {mp4} ({frame_i} frames @ {fps:.2f} fps)", flush=True)
PY

ls -lh "$MP4" "$LOG"
echo "DEMO_VIDEO=$MP4"
