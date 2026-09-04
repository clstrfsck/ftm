//! `GameEvent`: what happened during a tick (§12.8).
//!
//! Events drive cosmetics only. Dropping every event must change nothing about
//! the game state (T16): the core raises them on the way past, never reads them
//! back, and never branches on whether anyone is listening.
//!
//! **Coordinates are visible-field coordinates**, exactly as in `GameView`
//! (§12.7): `(col, row)` with `row` counted from the topmost *visible* row, so
//! the renderer never has to know that a buffer zone exists. A cell above the
//! visible field is omitted, encoded as [`OFF_SCREEN`].

use crate::core::piece::PieceKind;

/// A mino that lies above the visible field and cannot be drawn (§12.7).
pub const OFF_SCREEN: (u8, u8) = (255, 255);

/// The classification of a lock, one row of the §9.14 score table.
///
/// The variants that name a T-spin are produced from Stage 7 onwards; until the
/// classifier of §9.13 exists, every clear is a plain one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ClearKind {
    Single,
    Double,
    Triple,
    Quad,
    TSpin,
    TSpinSingle,
    TSpinDouble,
    TSpinTriple,
    TSpinMini,
    TSpinMiniSingle,
    TSpinMiniDouble,
}

impl ClearKind {
    /// The classification of a lock that cleared `lines` rows without a T-spin.
    ///
    /// A plain lock that clears nothing is not a clear at all and has no kind,
    /// which is why this returns an `Option`.
    pub const fn plain(lines: usize) -> Option<Self> {
        match lines {
            1 => Some(ClearKind::Single),
            2 => Some(ClearKind::Double),
            3 => Some(ClearKind::Triple),
            4 => Some(ClearKind::Quad),
            _ => None,
        }
    }

    /// How many rows this clear removed.
    pub const fn lines(self) -> u8 {
        match self {
            ClearKind::TSpin | ClearKind::TSpinMini => 0,
            ClearKind::Single | ClearKind::TSpinSingle | ClearKind::TSpinMiniSingle => 1,
            ClearKind::Double | ClearKind::TSpinDouble | ClearKind::TSpinMiniDouble => 2,
            ClearKind::Triple | ClearKind::TSpinTriple => 3,
            ClearKind::Quad => 4,
        }
    }

    /// Whether this is a **difficult** clear, and so continues a back-to-back
    /// chain (§9.15): any Quad, and any T-spin that cleared a line.
    pub const fn is_difficult(self) -> bool {
        match self {
            ClearKind::Quad => true,
            ClearKind::Single | ClearKind::Double | ClearKind::Triple => false,
            ClearKind::TSpin | ClearKind::TSpinMini => false,
            _ => true,
        }
    }
}

/// Why points were awarded (§9.14).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ScoreReason {
    LineClear(ClearKind),
    Combo,
    PerfectClear,
    SoftDrop,
    HardDrop,
}

/// How the game ended (§9.16).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TopOutCause {
    /// A newly spawned piece overlapped a locked mino at its spawn position.
    BlockOut,
    /// A piece locked with all four minos inside the buffer zone.
    LockOut,
}

/// Something the rules did during one tick (§12.8).
///
/// Emitted in the order the rules produced them. The common tick emits nothing
/// at all, which is why `Game::tick` fills a caller-supplied buffer rather than
/// returning a fresh collection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GameEvent {
    PieceSpawned(PieceKind),
    /// Any successful translation of the falling piece: a player shift or a
    /// gravity step. The one-row drop that is part of spawning is reported by
    /// `PieceSpawned`, and the rows a hard drop covers by `HardDropped`.
    PieceMoved,
    PieceRotated {
        kick_index: u8,
    },
    RotationFailed,
    HoldUsed,
    HoldRejected,
    PieceLocked {
        cells: [(u8, u8); 4],
        kind: PieceKind,
    },
    HardDropped {
        rows: u8,
    },
    /// Raised when the rows are found, at the start of the line-clear pause
    /// (§9.12 step 5), so the flash animation covers the pause exactly.
    LinesCleared {
        rows: Vec<u8>,
        clear: ClearKind,
        b2b: bool,
        combo: i32,
    },
    PerfectClear,
    LevelUp(u32),
    ScoreAwarded {
        points: u64,
        reason: ScoreReason,
    },
    ToppedOut(TopOutCause),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plain_clear_is_named_after_its_row_count() {
        assert_eq!(ClearKind::plain(0), None);
        assert_eq!(ClearKind::plain(1), Some(ClearKind::Single));
        assert_eq!(ClearKind::plain(4), Some(ClearKind::Quad));
        assert_eq!(ClearKind::plain(5), None, "four minos cannot clear five");
    }

    #[test]
    fn every_kind_reports_the_rows_its_name_implies() {
        for (kind, lines) in [
            (ClearKind::Single, 1),
            (ClearKind::Double, 2),
            (ClearKind::Triple, 3),
            (ClearKind::Quad, 4),
            (ClearKind::TSpin, 0),
            (ClearKind::TSpinSingle, 1),
            (ClearKind::TSpinDouble, 2),
            (ClearKind::TSpinTriple, 3),
            (ClearKind::TSpinMini, 0),
            (ClearKind::TSpinMiniSingle, 1),
            (ClearKind::TSpinMiniDouble, 2),
        ] {
            assert_eq!(kind.lines(), lines, "{kind:?}");
        }
    }

    #[test]
    fn difficult_clears_are_the_quad_and_the_scoring_t_spins() {
        // §9.15: "any Quad, and any T-spin or T-spin Mini that clears at
        // least one line." A T-spin that clears nothing does not qualify, and
        // does not break the chain either -- that half is Stage 7's.
        assert!(ClearKind::Quad.is_difficult());
        assert!(ClearKind::TSpinSingle.is_difficult());
        assert!(ClearKind::TSpinMiniDouble.is_difficult());
        assert!(!ClearKind::Triple.is_difficult());
        assert!(!ClearKind::TSpin.is_difficult());
        assert!(!ClearKind::TSpinMini.is_difficult());
    }
}
