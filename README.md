# Termino

Termino is a guideline-conformant falling-block game for the terminal: a single
Rust binary with no server, no `unsafe`, and a pure rules core that knows nothing
about the terminal it is drawn on. Pieces fall on a fixed 60 Hz tick, so the same
seed and the same inputs always produce the same game.

```
cargo run --release
```

The specification is [TERMINO.md](TERMINO.md) and it is ground truth: if the code
and the spec disagree, the spec is wrong until it is amended.
[PLAN.md](PLAN.md) sequences the implementation into twelve stages.

Requires Rust 1.85 or later (edition 2024). `make check` runs everything CI runs:
`cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test` and a release
build.
