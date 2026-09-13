# The single source of truth for what "clean" means. CI runs `make check` and
# nothing else (.github/workflows/ci.yml), so a step added here is a step CI
# picks up; there is no second list to keep in sync. `make check` must be clean
# at every stage boundary of TERMINAL-PLAN.md and EGUI-PLAN.md, not just at the end.
#
# `--all-features` everywhere, and this is not optional: since EGUI-PLAN.md G2 the
# front-ends are behind features, and a bare `cargo test` builds only the `tui`
# half. The failure mode is silent -- the other front-end simply stops being
# compiled -- so the flag is on every command that compiles anything.

.PHONY: check fmt clippy test shell portable web-check build web run run-gui run-web bench

check: fmt clippy test shell portable web-check build

fmt:
	cargo fmt --check

clippy:
	cargo clippy --all-features --all-targets -- -D warnings

test:
	cargo test --all-features

# The compiler holding the shell boundary the way §17.3's A10 made it hold the
# core's: with neither front-end feature on, `core/`, `shell/` and — since
# PILOT-PLAN.md P2 — `pilot/` must compile alone. It is worth more than any
# audit, for the same reason A10 was.
shell:
	cargo check --no-default-features

# The same check with the platform taken out too, and the stronger of the two
# (EGUI-PLAN.md G3, FRONTEND.md's "The three layers"). `wasm32-unknown-unknown` has
# no clock, no filesystem and no OS entropy, so an `Instant::now()`, a
# `std::fs` or a `rand::random` that crept into `shell/` goes red here in the
# same commit -- and `getrandom` refuses to compile for the target at all,
# which is what stops `rand`'s default features drifting back on.
#
# It needs the target installed: `rustup target add wasm32-unknown-unknown`.
# A `cfg(target_arch)` in `shell/` would pass this and is a bug, not a fix, and
# the same goes for `pilot/`: PILOT.md §P3.3's "the planner takes no clock" is
# this check, and the cadence invariance of §P9's C4 rests on it.
portable:
	cargo check --no-default-features --target wasm32-unknown-unknown

# The web front-end, linted for the only target it exists on (EGUI-PLAN.md G6,
# GUI.md §G8). `clippy` above lints `--all-features` for the host, which
# compiles `gui/host_native.rs` and never `gui/host_web.rs` or the web `main`,
# so without this the half of the window front-end that lives in a browser tab
# is compiled by nobody until trunk runs. `portable` holds the shell with no
# platform under it; this holds the web front-end with only a browser under
# it. Needs the target, not trunk.
web-check:
	cargo clippy --no-default-features --features gui --target wasm32-unknown-unknown --lib --bins -- -D warnings

build:
	cargo build --release --all-features

# The web build's artefact, in dist/ (Trunk.toml, index.html). Not part of
# `check`, because it needs trunk, which is not a cargo component and fetches
# `wasm-bindgen` and `wasm-opt` on first use; CI's `web` job runs it with a
# pinned trunk. Locally: `cargo install trunk --locked`.
web:
	trunk build --release

run:
	cargo run --release

# PILOT.md §P8's baseline, and the command its recorded result came from
# (PILOT-PLAN.md P5). Release only: a debug build replays every plan a second
# time for §P3.1's divergence assertion, so it measures the assertion rather
# than the planner.
# `make bench BENCH_ARGS="--seeds 32 --pieces 500"` for anything else.
BENCH_ARGS ?= --seeds 8 --pieces 2000
bench:
	cargo run --release --features bench --bin ftm-pilot -- $(BENCH_ARGS)

# The window front-end (GUI.md, EGUI-PLAN.md G5). `default-run` picks `ftm` of the
# two binaries, so this one has to be named.
run-gui:
	cargo run --release --features gui --bin ftm-gui

# The same front-end in a browser tab, at http://127.0.0.1:8080, rebuilt on
# save. `?seed=42` in the URL is `--seed 42` (GUI.md §G8.4).
run-web:
	trunk serve
