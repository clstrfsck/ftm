# Every target here is also a CI step (.github/workflows/ci.yml). `make check`
# must be clean at every stage boundary of PLAN.md, not just at the end.

.PHONY: check fmt clippy test build run

check: fmt clippy test build

fmt:
	cargo fmt --check

clippy:
	cargo clippy --all-targets -- -D warnings

test:
	cargo test

build:
	cargo build --release

run:
	cargo run --release
