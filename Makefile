.PHONY: build test fmt lint doc boundary release-config check package-check fuzz-smoke bench bench-core bench-builtins

build:
	cargo build --workspace --all-features --locked

test:
	cargo test --workspace --all-features --locked

fmt:
	cargo fmt --all -- --check

lint:
	cargo clippy --workspace --all-targets --all-features --locked -- -D warnings

doc:
	RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps --locked

boundary:
	python3 scripts/check-boundary.py

release-config:
	python3 scripts/check-release-please-config.py

check: fmt boundary release-config test lint doc

package-check:
	cargo package --list -p logcompact-core --locked --allow-dirty
	cargo package --list -p logcompact-builtins --locked --allow-dirty
	cargo package --list -p logcompact --locked --allow-dirty
	cargo package -p logcompact-core --locked --allow-dirty

fuzz-smoke:
	rustup run nightly cargo fuzz run logcompact -- -max_total_time=10

bench:
	cargo bench --workspace --all-features --locked

bench-core:
	cargo bench -p logcompact-core --bench core --locked

bench-builtins:
	cargo bench -p logcompact-builtins --bench streaming --all-features --locked
