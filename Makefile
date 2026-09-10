# The single source of truth for what "clean" means. CI runs `make check` and
# nothing else (.github/workflows/ci.yml), so a step added here is a step CI
# picks up; there is no second list to keep in sync. `make check` must be clean
# at every stage boundary of PLAN.md and EGUI.md, not just at the end.
#
# `--all-features` everywhere, and this is not optional: since EGUI.md G2 the
# front-ends are behind features, and a bare `cargo test` builds only the `tui`
# half. The failure mode is silent -- the other front-end simply stops being
# compiled -- so the flag is on every command that compiles anything.

.PHONY: check fmt clippy test shell portable build run

check: fmt clippy test shell portable build

fmt:
	cargo fmt --check

clippy:
	cargo clippy --all-features --all-targets -- -D warnings

test:
	cargo test --all-features

# The compiler holding the shell boundary the way §17.3's A10 made it hold the
# core's: with neither front-end feature on, `core/` and `shell/` must compile
# alone. It is worth more than any audit, for the same reason A10 was.
shell:
	cargo check --no-default-features

# The same check with the platform taken out too, and the stronger of the two
# (EGUI.md G3, FRONTEND.md's "The three layers"). `wasm32-unknown-unknown` has
# no clock, no filesystem and no OS entropy, so an `Instant::now()`, a
# `std::fs` or a `rand::random` that crept into `shell/` goes red here in the
# same commit -- and `getrandom` refuses to compile for the target at all,
# which is what stops `rand`'s default features drifting back on.
#
# It needs the target installed: `rustup target add wasm32-unknown-unknown`.
# A `cfg(target_arch)` in `shell/` would pass this and is a bug, not a fix.
portable:
	cargo check --no-default-features --target wasm32-unknown-unknown

build:
	cargo build --release --all-features

run:
	cargo run --release
