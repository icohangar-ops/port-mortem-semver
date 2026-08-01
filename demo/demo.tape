# Port Mortem 2026 — Track F demo
Output demo/port-mortem-semver-demo.gif
Output demo/port-mortem-semver-demo.mp4

Set Shell "zsh"
Set FontSize 16
Set Width 1280
Set Height 800
Set TypingSpeed 30ms
Set Theme "Catppuccin Mocha"
Set PlaybackSpeed 1.0

Hide
Type "cd /Users/cubiczan/Desktop/icohangar-repos/port-mortem-semver && export CARGO_TARGET_DIR=$PWD/target && clear"
Enter
Sleep 500ms
Show

Type "# Port Mortem 2026 · Track F · JavaScript → Rust"
Enter
Type "# Source: npm/node-semver → Target: idiomatic safe Rust (zero unsafe)"
Enter
Sleep 2s

Type "cat .port-mortem.toml"
Enter
Sleep 2s

Type "echo Pinned: $(cat tests/ORIGINAL_COMMIT.txt)"
Enter
Sleep 1.5s

Type "echo 'Kickoff hashes (sample):' && head -3 tests/original.SHA256SUMS"
Enter
Sleep 2s

Type "make build"
Enter
Sleep 12s

Type "./target/release/semver 1.2.3 2.0.0 1.5.0"
Enter
Sleep 1.2s

Type "./target/release/semver -r '^1' 1.2.3 2.0.0 1.9.9-beta 1.9.9"
Enter
Sleep 2s

Type "# NORTH STAR — original unmodified suite against the Rust port"
Enter
Sleep 1s

Type "make parity"
Enter
Sleep 50s

Type "./demo/check_hashes.sh"
Enter
Sleep 2s

Type "# Differential fuzz: vendored JS vs Rust RPC"
Enter
Type "make fuzz"
Enter
Sleep 75s

Type "tail -10 fuzz/log.txt"
Enter
Sleep 3s

Type "make bench"
Enter
Sleep 100s

Type "./demo/show_bench.sh"
Enter
Sleep 3s

Type "echo"
Enter
Type "echo '9,182/9,182 original assertions — unmodified suite'"
Enter
Type "echo '0 unsafe · fuzz survivor · DECISIONS.md ready'"
Enter
Type "echo 'https://github.com/Cubiczan/port-mortem-semver'"
Enter
Sleep 6s
