# The single source of truth for what "clean" means. CI runs `make check` and
# nothing else (.github/workflows/ci.yml), so a step added here is a step CI
# picks up; there is no second list to keep in sync. `make check` must be clean
# at every stage boundary of PLAN.md, not just at the end.

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
