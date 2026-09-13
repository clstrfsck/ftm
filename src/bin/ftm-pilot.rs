//! `PILOT.md` §P8's headless benchmark: the automated player, measured.
//!
//! An entry point and nothing else, exactly as the two front-ends' are (§4).
//! Everything it does is `ftm::bench`'s, so that §P8.1's grammar and §P8.2's
//! arithmetic are unit-tested in the library rather than reachable only by
//! running the program.
//!
//! It is not a front-end: it draws nothing, takes no keys and opens no window
//! or terminal. It is behind `feature = "bench"` for that reason — a benchmark
//! that pulled in `ratatui` or `eframe` to print integers would be saying
//! something false about what it needs.

fn main() -> anyhow::Result<()> {
    ftm::bench::main()
}
