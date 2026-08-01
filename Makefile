.PHONY: build test parity fuzz bench docker

# The Node adapter and the benchmarks look for the binaries in ./target/release,
# so pin the target directory rather than inheriting one from the environment.
export CARGO_TARGET_DIR = $(CURDIR)/target

build:
	cargo build --release

test:
	cargo test

parity: build
	cd adapter && npm install --no-fund --no-audit && npm test

fuzz: build
	./scripts/diff_fuzz.sh

bench: build
	cargo bench --bench compare
	node bench/run.js

docker:
	docker build -t node-semver-rs .
