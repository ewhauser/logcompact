.PHONY: build test fmt lint doc boundary check package-check fuzz-smoke bench

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

check: fmt boundary test lint doc

package-check:
	cargo package --list -p tokencompact-core --locked --allow-dirty
	cargo package --list -p tokencompact-builtins --locked --allow-dirty
	cargo package --list -p tokencompact --locked --allow-dirty
	cargo package -p tokencompact-core --locked --allow-dirty

fuzz-smoke:
	rustup run nightly cargo fuzz run tokencompact -- -max_total_time=10

bench:
	cargo bench -p tokencompact-builtins --bench streaming
