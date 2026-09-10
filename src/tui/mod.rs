//! The terminal front-end.
//!
//! Today it holds only the crossterm key adapter; `EGUI.md` stage G2 moves the
//! rest of `ui/`, plus `main.rs`'s loop and terminal handling, in beside it and
//! puts the whole module behind `feature = "tui"`.

pub mod keys;
