//! Screen dispatch, the terminal-too-small screen (§12.1) and the animation
//! timers (§12.5).
//!
//! Everything here draws from `GameView` alone and never reads `Game` (§12.7).
//! The animations are the other half of that bargain: each one is **started by
//! a `GameEvent`** (§12.8) and then runs on the shell's own wall clock, so the
//! core never knows one is in progress and dropping every event costs nothing
//! but the decoration.

pub mod attract;
pub mod cells;
pub mod overlays;
pub mod playfield;
pub mod theme;

use std::io::Stdout;
use std::time::{Duration, Instant};

use ratatui::Frame;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::Rect;

use crate::core::events::OFF_SCREEN;
use crate::core::piece::PieceKind;
use crate::core::{ClearKind, GameEvent, GameView, ScoreReason};
use crate::ui::theme::Theme;

/// The terminal the game draws on.
pub type Tui = Terminal<CrosstermBackend<Stdout>>;

// TODO(stage 12): the terminal-too-small screen and resize handling (§8.4).

/// The settings the screen needs that are not game state.
///
/// `GameView` carries what the rules produced (§12.7); this carries what the
/// player and the terminal decided. `hold_enabled` is here because it is the
/// one layout question the view cannot answer: an empty hold slot and an
/// absent hold mechanic look identical in `GameView::hold` (§12.4).
#[derive(Clone, Copy, Debug)]
pub struct Chrome {
    pub theme: Theme,
    pub show_grid: bool,
    pub hold_enabled: bool,
}

/// What is drawn on top of the playfield, if anything (§12.6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Overlay {
    None,
    /// §9.17: the pause menu, over a blanked playfield.
    Paused {
        selected: usize,
    },
    /// §9.17: the 3-2-1 resume countdown, over a playfield that is visible
    /// again — reading the board is what the countdown is for.
    Resuming {
        count: u8,
    },
    GameOver,
}

/// Draw one frame of the playing screen.
pub fn draw(frame: &mut Frame, view: &GameView, chrome: &Chrome, fx: &Cosmetics, overlay: Overlay) {
    let blanked = matches!(overlay, Overlay::Paused { .. });
    let screen = playfield::render(frame, view, chrome, fx, blanked);
    match overlay {
        Overlay::None => {}
        Overlay::Paused { selected } => overlays::paused(frame, screen, chrome, selected),
        Overlay::Resuming { count } => overlays::resuming(frame, screen, chrome, count),
        Overlay::GameOver => overlays::game_over(frame, screen, view, chrome),
    }
}

/// A `width` x `height` block centred in `area` (§12.1: the UI is a fixed-size
/// block and the extra space is margin), clipped to what there is.
pub fn centred(area: Rect, width: u16, height: u16) -> Rect {
    Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width: width.min(area.width),
        height: height.min(area.height),
    }
}

// ---------------------------------------------------------------------------
// §12.5 animations
// ---------------------------------------------------------------------------

/// The hard-drop trail (§12.5).
const TRAIL: Duration = Duration::from_millis(120);
/// The lock flash (§12.5).
const LOCK_FLASH: Duration = Duration::from_millis(80);
/// The level-up banner (§12.5).
const LEVEL_BANNER: Duration = Duration::from_millis(1_200);
/// The perfect-clear banner (§12.5).
const PERFECT_BANNER: Duration = Duration::from_millis(1_500);
/// The game-over wipe (§12.5).
const WIPE: Duration = Duration::from_millis(500);
/// How long the most recent clear's name stays on the status line (§12.4).
const STATUS: Duration = Duration::from_millis(1_500);
/// The line-clear flash alternates at 12 Hz (§12.5).
const FLASH_PERIOD: Duration = Duration::from_millis(1_000 / 12);

/// A banner over the playfield (§12.5).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Banner {
    /// `LEVEL n`, fading: the percentage is what is left of it.
    LevelUp(u32, u8),
    /// `PERFECT CLEAR`, in the seven piece colours cycling.
    PerfectClear(PieceKind),
}

/// The hard-drop trail: the piece it was, and the cells it passed through.
#[derive(Clone, Debug)]
struct Trail {
    kind: PieceKind,
    cells: Vec<(u8, u8)>,
}

/// One running animation: what it covers and when it started.
#[derive(Clone, Debug)]
struct Running<T> {
    what: T,
    since: Instant,
}

/// The cosmetic state of the screen: every §12.5 animation and §12.4's status
/// line, driven entirely by the event stream.
///
/// This is the whole of the "events are a notification, never a mechanism"
/// contract in one place (§12.8): it consumes `&[GameEvent]` and a clock, and
/// has no way to reach the core even if it wanted to.
pub struct Cosmetics {
    /// The line-clear flash lasts exactly the core's clear pause, so the flash
    /// covers the pause and stops when the rows collapse (§12.5, §12.8).
    clear_delay: Duration,
    now: Instant,
    flash: Option<Running<Vec<u8>>>,
    trail: Option<Running<Trail>>,
    lock: Option<Running<Vec<(u8, u8)>>>,
    level_up: Option<Running<u32>>,
    perfect: Option<Running<()>>,
    wipe: Option<Running<()>>,
    status: Option<Running<&'static str>>,
    /// The rows the hard drop covered, waiting for the `PieceLocked` that says
    /// which columns they were: §12.8 splits the two across one tick.
    dropped: Option<u8>,
}

impl Cosmetics {
    pub fn new(clear_delay: Duration, now: Instant) -> Self {
        Self {
            clear_delay,
            now,
            flash: None,
            trail: None,
            lock: None,
            level_up: None,
            perfect: None,
            wipe: None,
            status: None,
            dropped: None,
        }
    }

    /// Fold in the events of one frame and expire whatever has run its course.
    ///
    /// `events` may be empty, which is the common case; the clock still has to
    /// advance, because that is what ends an animation.
    pub fn absorb(&mut self, events: &[GameEvent], now: Instant) {
        self.now = now;
        for event in events {
            self.start(event, now);
        }
        expire(&mut self.flash, now, self.clear_delay);
        expire(&mut self.trail, now, TRAIL);
        expire(&mut self.lock, now, LOCK_FLASH);
        expire(&mut self.level_up, now, LEVEL_BANNER);
        expire(&mut self.perfect, now, PERFECT_BANNER);
        expire(&mut self.status, now, STATUS);
        // The wipe is not expired: it settles on a fully greyed stack and the
        // game-over overlay sits on top of it (§9.16).
    }

    fn start(&mut self, event: &GameEvent, now: Instant) {
        match event {
            GameEvent::HardDropped { rows } => self.dropped = Some(*rows),
            GameEvent::PieceLocked { cells, kind } => {
                let drawable: Vec<(u8, u8)> =
                    cells.iter().copied().filter(|c| *c != OFF_SCREEN).collect();
                if let Some(rows) = self.dropped.take().filter(|rows| *rows > 0) {
                    self.trail = Some(Running {
                        what: Trail {
                            kind: *kind,
                            cells: trail_cells(&drawable, rows),
                        },
                        since: now,
                    });
                }
                self.lock = Some(Running {
                    what: drawable,
                    since: now,
                });
            }
            GameEvent::LinesCleared { rows, clear, .. } => {
                self.flash = Some(Running {
                    what: rows.clone(),
                    since: now,
                });
                self.status = Some(Running {
                    what: name(*clear),
                    since: now,
                });
            }
            GameEvent::ScoreAwarded {
                reason: ScoreReason::LineClear(clear),
                ..
            } if clear.lines() == 0 => {
                // §12.8: a lock that spun without completing a row raises no
                // `LinesCleared`, so this is the only notice the status line
                // ever gets of a bare T-spin.
                self.status = Some(Running {
                    what: name(*clear),
                    since: now,
                });
            }
            GameEvent::PerfectClear => {
                self.perfect = Some(Running {
                    what: (),
                    since: now,
                });
                self.status = Some(Running {
                    what: "PERFECT CLEAR",
                    since: now,
                });
            }
            GameEvent::LevelUp(level) => {
                self.level_up = Some(Running {
                    what: *level,
                    since: now,
                });
            }
            GameEvent::ToppedOut(_) => {
                self.wipe = Some(Running {
                    what: (),
                    since: now,
                });
            }
            _ => {}
        }
    }

    /// Whether `row` is mid-flash, and whether it is on the white half of the
    /// alternation (§12.5).
    pub fn flashing(&self, row: u8) -> Option<bool> {
        let flash = self.flash.as_ref()?;
        if !flash.what.contains(&row) {
            return None;
        }
        let elapsed = self.now.saturating_duration_since(flash.since);
        Some(elapsed.as_nanos() / FLASH_PERIOD.as_nanos() % 2 == 0)
    }

    /// The piece a hard drop was, and the cells it passed through (§12.5).
    pub fn trail(&self) -> Option<(PieceKind, &[(u8, u8)])> {
        self.trail
            .as_ref()
            .map(|t| (t.what.kind, t.what.cells.as_slice()))
    }

    /// The cells of the piece that just locked, while its flash lasts (§12.5).
    pub fn lock_flash(&self) -> &[(u8, u8)] {
        self.lock.as_ref().map_or(&[], |l| &l.what)
    }

    /// How many rows, counted from the top, the game-over wipe has reached
    /// (§12.5).
    pub fn wiped_rows(&self, height: u8) -> u8 {
        let Some(wipe) = self.wipe.as_ref() else {
            return 0;
        };
        let elapsed = self.now.saturating_duration_since(wipe.since);
        if elapsed >= WIPE {
            return height;
        }
        ((elapsed.as_nanos() * u128::from(height)) / WIPE.as_nanos()) as u8
    }

    /// The banner over the playfield, if one is up (§12.5).
    ///
    /// A perfect clear outranks the level-up it so often arrives with: it is
    /// the rarer thing to have done.
    pub fn banner(&self) -> Option<Banner> {
        if let Some(perfect) = self.perfect.as_ref() {
            let elapsed = self.now.saturating_duration_since(perfect.since);
            let step = (elapsed.as_millis() / 120) as usize % PieceKind::ALL.len();
            return Some(Banner::PerfectClear(PieceKind::ALL[step]));
        }
        let level = self.level_up.as_ref()?;
        let elapsed = self.now.saturating_duration_since(level.since);
        let left = LEVEL_BANNER.saturating_sub(elapsed);
        let percent = (left.as_millis() * 100 / LEVEL_BANNER.as_millis()) as u8;
        Some(Banner::LevelUp(level.what, percent))
    }

    /// Whether anything on screen is still moving.
    ///
    /// The loop draws only when the frame has changed (§15.2 step 5), and an
    /// animation changes the frame without changing the view — so it has to say
    /// so itself. The wipe is asked whether it has *finished*, not whether it
    /// happened, because it settles rather than expiring.
    pub fn animating(&self) -> bool {
        self.flash.is_some()
            || self.trail.is_some()
            || self.lock.is_some()
            || self.level_up.is_some()
            || self.perfect.is_some()
            || self
                .wipe
                .as_ref()
                .is_some_and(|w| self.now.saturating_duration_since(w.since) < WIPE)
    }

    /// The most recent clear's name, for the 1.5 s it is shown (§12.4).
    pub fn clear_name(&self) -> Option<&'static str> {
        self.status.as_ref().map(|s| s.what)
    }
}

/// Drop a finished animation.
fn expire<T>(slot: &mut Option<Running<T>>, now: Instant, life: Duration) {
    if slot
        .as_ref()
        .is_some_and(|running| now.saturating_duration_since(running.since) >= life)
    {
        *slot = None;
    }
}

/// The cells a hard drop of `rows` passed through, above where it landed.
fn trail_cells(landed: &[(u8, u8)], rows: u8) -> Vec<(u8, u8)> {
    let mut cells: Vec<(u8, u8)> = landed
        .iter()
        .flat_map(|&(col, row)| (1..=rows).filter_map(move |up| Some((col, row.checked_sub(up)?))))
        .collect();
    // Two minos in the same column trail over the same cells; painting one of
    // them twice is wasted work, not a second shade.
    cells.sort_unstable();
    cells.dedup();
    cells
}

/// The name §12.4's status line announces a clear by.
const fn name(clear: ClearKind) -> &'static str {
    match clear {
        ClearKind::Single => "SINGLE",
        ClearKind::Double => "DOUBLE",
        ClearKind::Triple => "TRIPLE",
        ClearKind::Quad => "QUAD",
        ClearKind::TSpin => "T-SPIN",
        ClearKind::TSpinSingle => "T-SPIN SINGLE",
        ClearKind::TSpinDouble => "T-SPIN DOUBLE",
        ClearKind::TSpinTriple => "T-SPIN TRIPLE",
        ClearKind::TSpinMini => "T-SPIN MINI",
        ClearKind::TSpinMiniSingle => "T-SPIN MINI SINGLE",
        ClearKind::TSpinMiniDouble => "T-SPIN MINI DOUBLE",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::TopOutCause;

    const CLEAR_DELAY: Duration = Duration::from_millis(250);

    fn fx(now: Instant) -> Cosmetics {
        Cosmetics::new(CLEAR_DELAY, now)
    }

    #[test]
    fn a_hard_drop_leaves_a_trail_above_where_it_landed() {
        // §12.5: "the columns the piece passed through are drawn dimmed behind
        // it". §12.8 splits the news across two events in the same tick.
        let start = Instant::now();
        let mut fx = fx(start);
        fx.absorb(
            &[
                GameEvent::HardDropped { rows: 2 },
                GameEvent::PieceLocked {
                    cells: [(4, 19), (5, 19), (6, 19), (5, 18)],
                    kind: PieceKind::T,
                },
            ],
            start,
        );
        let (kind, cells) = fx.trail().expect("a trail");
        assert_eq!(kind, PieceKind::T, "drawn in the piece's own colour");
        let mut trail = cells.to_vec();
        trail.sort_unstable();
        assert_eq!(
            trail,
            [
                (4, 17),
                (4, 18),
                (5, 16),
                (5, 17),
                (5, 18),
                (6, 17),
                (6, 18)
            ],
            "two rows above each landed mino, each cell once",
        );

        fx.absorb(&[], start + TRAIL);
        assert!(fx.trail().is_none(), "and gone after 120 ms");
    }

    #[test]
    fn a_lock_that_was_not_dropped_leaves_no_trail() {
        let start = Instant::now();
        let mut fx = fx(start);
        fx.absorb(
            &[GameEvent::PieceLocked {
                cells: [(0, 19), (1, 19), (2, 19), (0, 18)],
                kind: PieceKind::J,
            }],
            start,
        );
        assert!(fx.trail().is_none());
        assert_eq!(fx.lock_flash().len(), 4, "but it does flash");
        fx.absorb(&[], start + LOCK_FLASH);
        assert!(fx.lock_flash().is_empty());
    }

    #[test]
    fn a_mino_above_the_field_is_not_drawn() {
        // §12.8: `PieceLocked` encodes an off-screen mino as (255, 255), and
        // nothing downstream may treat that as a coordinate.
        let start = Instant::now();
        let mut fx = fx(start);
        fx.absorb(
            &[
                GameEvent::HardDropped { rows: 1 },
                GameEvent::PieceLocked {
                    cells: [OFF_SCREEN, OFF_SCREEN, (3, 0), (3, 1)],
                    kind: PieceKind::I,
                },
            ],
            start,
        );
        assert_eq!(fx.lock_flash(), [(3, 0), (3, 1)]);
        let (_, cells) = fx.trail().expect("a trail");
        assert_eq!(cells, [(3, 0)], "and row 0 has nothing above it");
    }

    #[test]
    fn the_line_clear_flash_alternates_and_lasts_the_clear_pause() {
        // §12.5: 12 Hz, for `line_clear_delay_ms`, which is what makes it cover
        // the core's pause exactly (§12.8).
        let start = Instant::now();
        let mut fx = fx(start);
        fx.absorb(
            &[GameEvent::LinesCleared {
                rows: vec![18, 19],
                clear: ClearKind::Double,
                b2b: false,
                combo: 0,
            }],
            start,
        );
        assert_eq!(fx.flashing(19), Some(true));
        assert_eq!(fx.flashing(17), None, "an untouched row does not flash");

        fx.absorb(&[], start + FLASH_PERIOD);
        assert_eq!(fx.flashing(19), Some(false), "the other half of 12 Hz");
        fx.absorb(&[], start + FLASH_PERIOD * 2);
        assert_eq!(fx.flashing(19), Some(true));

        fx.absorb(&[], start + CLEAR_DELAY);
        assert_eq!(fx.flashing(19), None, "then the rows collapse");
    }

    #[test]
    fn the_status_line_names_the_clear_for_a_second_and_a_half() {
        let start = Instant::now();
        let mut fx = fx(start);
        fx.absorb(
            &[GameEvent::LinesCleared {
                rows: vec![19],
                clear: ClearKind::TSpinSingle,
                b2b: true,
                combo: 0,
            }],
            start,
        );
        assert_eq!(fx.clear_name(), Some("T-SPIN SINGLE"));
        fx.absorb(&[], start + STATUS);
        assert_eq!(fx.clear_name(), None);
    }

    #[test]
    fn a_spin_that_cleared_nothing_still_reaches_the_status_line() {
        // §12.8: it raises no `LinesCleared`, so `ScoreAwarded` is the only
        // notice of it there is.
        let start = Instant::now();
        let mut fx = fx(start);
        fx.absorb(
            &[GameEvent::ScoreAwarded {
                points: 400,
                reason: ScoreReason::LineClear(ClearKind::TSpin),
            }],
            start,
        );
        assert_eq!(fx.clear_name(), Some("T-SPIN"));
        assert_eq!(
            fx.flashing(19),
            None,
            "and no rows flash, because none went"
        );
    }

    #[test]
    fn a_scored_clear_does_not_overwrite_its_own_name() {
        // The clear's `ScoreAwarded` follows its `LinesCleared` in the same
        // tick; only the no-line spins may set the status from a score.
        let start = Instant::now();
        let mut fx = fx(start);
        fx.absorb(
            &[
                GameEvent::LinesCleared {
                    rows: vec![19],
                    clear: ClearKind::TSpinSingle,
                    b2b: false,
                    combo: 0,
                },
                GameEvent::ScoreAwarded {
                    points: 800,
                    reason: ScoreReason::LineClear(ClearKind::TSpinSingle),
                },
            ],
            start,
        );
        assert_eq!(fx.clear_name(), Some("T-SPIN SINGLE"));
    }

    #[test]
    fn the_perfect_clear_banner_outranks_the_level_up_it_arrives_with() {
        let start = Instant::now();
        let mut fx = fx(start);
        fx.absorb(&[GameEvent::LevelUp(5)], start);
        assert_eq!(fx.banner(), Some(Banner::LevelUp(5, 100)));

        fx.absorb(&[GameEvent::PerfectClear], start);
        assert!(matches!(fx.banner(), Some(Banner::PerfectClear(_))));

        // The level-up outlives neither: 1.2 s and 1.5 s from §12.5.
        fx.absorb(&[], start + LEVEL_BANNER);
        assert!(matches!(fx.banner(), Some(Banner::PerfectClear(_))));
        fx.absorb(&[], start + PERFECT_BANNER);
        assert_eq!(fx.banner(), None);
    }

    #[test]
    fn the_level_up_banner_fades() {
        let start = Instant::now();
        let mut fx = fx(start);
        fx.absorb(&[GameEvent::LevelUp(7)], start);
        fx.absorb(&[], start + LEVEL_BANNER / 2);
        assert_eq!(fx.banner(), Some(Banner::LevelUp(7, 50)));
    }

    #[test]
    fn the_game_over_wipe_greys_the_stack_downwards_and_stays() {
        // §12.5: 500 ms, top row first. Unlike the others it does not expire —
        // §9.16 leaves the greyed matrix under the overlay.
        let start = Instant::now();
        let mut fx = fx(start);
        assert_eq!(fx.wiped_rows(20), 0);
        fx.absorb(&[GameEvent::ToppedOut(TopOutCause::LockOut)], start);
        assert_eq!(fx.wiped_rows(20), 0);
        fx.absorb(&[], start + WIPE / 2);
        assert_eq!(fx.wiped_rows(20), 10);
        fx.absorb(&[], start + WIPE);
        assert_eq!(fx.wiped_rows(20), 20);
        fx.absorb(&[], start + WIPE * 4);
        assert_eq!(fx.wiped_rows(20), 20, "and it stays wiped");
    }
}
