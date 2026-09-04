//! Tetromino kinds, their four orientation patterns and spawn data
//! (§9.2, §9.3, §9.4).
//!
//! A piece is a square bounding box whose contents rotate in place; the box's
//! **origin** is the matrix coordinate of its top-left corner. Rotating changes
//! only the box contents, never the origin — any positional adjustment comes
//! from the kick tables (§9.5).
//!
//! The patterns of §9.3 are stored as bitmasks, one bit per box cell, row major,
//! most significant bit first, so that a literal written as `0b010_111_000` is
//! read top-to-bottom, left-to-right exactly like the picture in the spec. Each
//! mask is written directly under the picture it encodes; that is the whole
//! point of the layout, so keep them side by side.

use serde::{Deserialize, Serialize};

use crate::core::geometry::{Point, Rotation};

/// The seven tetrominoes (§9.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PieceKind {
    I,
    O,
    T,
    S,
    Z,
    J,
    L,
}

/// The guideline colour of a piece (§9.2).
///
/// The core names the colour and gives its truecolor value; the mapping onto a
/// terminal's colour depth — 256, 16, or the mono glyph — is presentation, and
/// lives in `ui::theme` (§12.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Colour {
    Cyan,
    Yellow,
    Purple,
    Green,
    Red,
    Blue,
    Orange,
}

impl Colour {
    /// The truecolor value from the §9.2 table.
    pub const fn rgb(self) -> (u8, u8, u8) {
        match self {
            Colour::Cyan => (0x00, 0xF0, 0xF0),
            Colour::Yellow => (0xF0, 0xF0, 0x00),
            Colour::Purple => (0xA0, 0x00, 0xF0),
            Colour::Green => (0x00, 0xF0, 0x00),
            Colour::Red => (0xF0, 0x00, 0x00),
            Colour::Blue => (0x00, 0x00, 0xF0),
            Colour::Orange => (0xF0, 0xA0, 0x00),
        }
    }
}

/// The number of minos in a tetromino, in every orientation.
pub const MINOS: usize = 4;

impl PieceKind {
    /// All seven kinds. The order is the bag's canonical order (§9.6) and is
    /// part of the seeded sequence, so it must not be reordered.
    pub const ALL: [PieceKind; 7] = [
        PieceKind::I,
        PieceKind::O,
        PieceKind::T,
        PieceKind::S,
        PieceKind::Z,
        PieceKind::J,
        PieceKind::L,
    ];

    /// The side of the piece's square bounding box: 4 for `I`, 2 for `O`, 3 for
    /// the rest (§9.3).
    pub const fn box_size(self) -> i32 {
        match self {
            PieceKind::I => 4,
            PieceKind::O => 2,
            _ => 3,
        }
    }

    /// The guideline colour (§9.2).
    pub const fn colour(self) -> Colour {
        match self {
            PieceKind::I => Colour::Cyan,
            PieceKind::O => Colour::Yellow,
            PieceKind::T => Colour::Purple,
            PieceKind::S => Colour::Green,
            PieceKind::Z => Colour::Red,
            PieceKind::J => Colour::Blue,
            PieceKind::L => Colour::Orange,
        }
    }

    /// The mono glyph (§9.2, §12.3): the piece's own letter.
    pub const fn glyph(self) -> char {
        match self {
            PieceKind::I => 'I',
            PieceKind::O => 'O',
            PieceKind::T => 'T',
            PieceKind::S => 'S',
            PieceKind::Z => 'Z',
            PieceKind::J => 'J',
            PieceKind::L => 'L',
        }
    }

    /// The spawn origin from the §9.4 table, in matrix coordinates.
    pub const fn spawn_origin(self) -> Point {
        match self {
            PieceKind::O => Point::new(4, 18),
            _ => Point::new(3, 18),
        }
    }

    /// The §9.3 cell pattern as a row-major bitmask over the bounding box.
    const fn pattern(self, rotation: Rotation) -> u16 {
        match self {
            //  North (0)      East (1)       South (2)      West (3)
            // . . . .        . . I .        . . . .        . I . .
            // I I I I        . . I .        . . . .        . I . .
            // . . . .        . . I .        I I I I        . I . .
            // . . . .        . . I .        . . . .        . I . .
            PieceKind::I => match rotation {
                Rotation::North => 0b0000_1111_0000_0000,
                Rotation::East => 0b0010_0010_0010_0010,
                Rotation::South => 0b0000_0000_1111_0000,
                Rotation::West => 0b0100_0100_0100_0100,
            },
            // O O   — identical in all four orientations; `O` never kicks.
            // O O
            PieceKind::O => 0b11_11,
            // . T .          . T .          . . .          . T .
            // T T T          . T T          T T T          T T .
            // . . .          . T .          . T .          . T .
            PieceKind::T => match rotation {
                Rotation::North => 0b010_111_000,
                Rotation::East => 0b010_011_010,
                Rotation::South => 0b000_111_010,
                Rotation::West => 0b010_110_010,
            },
            // . S S          . S .          . . .          S . .
            // S S .          . S S          . S S          S S .
            // . . .          . . S          S S .          . S .
            PieceKind::S => match rotation {
                Rotation::North => 0b011_110_000,
                Rotation::East => 0b010_011_001,
                Rotation::South => 0b000_011_110,
                Rotation::West => 0b100_110_010,
            },
            // Z Z .          . . Z          . . .          . Z .
            // . Z Z          . Z Z          Z Z .          Z Z .
            // . . .          . Z .          . Z Z          Z . .
            PieceKind::Z => match rotation {
                Rotation::North => 0b110_011_000,
                Rotation::East => 0b001_011_010,
                Rotation::South => 0b000_110_011,
                Rotation::West => 0b010_110_100,
            },
            // J . .          . J J          . . .          . J .
            // J J J          . J .          J J J          . J .
            // . . .          . J .          . . J          J J .
            PieceKind::J => match rotation {
                Rotation::North => 0b100_111_000,
                Rotation::East => 0b011_010_010,
                Rotation::South => 0b000_111_001,
                Rotation::West => 0b010_010_110,
            },
            // . . L          . L .          . . .          L L .
            // L L L          . L .          L L L          . L .
            // . . .          . L L          L . .          . L .
            PieceKind::L => match rotation {
                Rotation::North => 0b001_111_000,
                Rotation::East => 0b010_010_011,
                Rotation::South => 0b000_111_100,
                Rotation::West => 0b110_010_010,
            },
        }
    }

    /// The four minos of this orientation, as offsets from the box origin,
    /// ordered top-to-bottom then left-to-right.
    pub fn cells(self, rotation: Rotation) -> [Point; MINOS] {
        let size = self.box_size();
        let pattern = self.pattern(rotation);
        let mut cells = [Point::new(0, 0); MINOS];
        let mut found = 0;
        for row in 0..size {
            for col in 0..size {
                let bit = size * size - 1 - (row * size + col);
                if pattern >> bit & 1 == 1 {
                    debug_assert!(found < MINOS, "{self:?} {rotation:?} has over 4 minos");
                    cells[found] = Point::new(col, row);
                    found += 1;
                }
            }
        }
        debug_assert_eq!(found, MINOS, "{self:?} {rotation:?} has under 4 minos");
        cells
    }

    /// The four minos in matrix coordinates, for a box at `origin`.
    pub fn minos(self, origin: Point, rotation: Rotation) -> [Point; MINOS] {
        self.cells(rotation).map(|c| origin.translate(c.x, c.y))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The §9.4 spawn table, transcribed literally. It is written out rather
    /// than recomputed so that a transcription bug in `pattern` or
    /// `spawn_origin` cannot agree with itself: this table is the spec's own
    /// answer, and everything downstream depends on it.
    const SPAWN_TABLE: [(PieceKind, Point, [Point; MINOS]); 7] = [
        (
            PieceKind::I,
            Point::new(3, 18),
            [
                Point::new(3, 19),
                Point::new(4, 19),
                Point::new(5, 19),
                Point::new(6, 19),
            ],
        ),
        (
            PieceKind::O,
            Point::new(4, 18),
            [
                Point::new(4, 18),
                Point::new(5, 18),
                Point::new(4, 19),
                Point::new(5, 19),
            ],
        ),
        (
            PieceKind::T,
            Point::new(3, 18),
            [
                Point::new(4, 18),
                Point::new(3, 19),
                Point::new(4, 19),
                Point::new(5, 19),
            ],
        ),
        (
            PieceKind::S,
            Point::new(3, 18),
            [
                Point::new(4, 18),
                Point::new(5, 18),
                Point::new(3, 19),
                Point::new(4, 19),
            ],
        ),
        (
            PieceKind::Z,
            Point::new(3, 18),
            [
                Point::new(3, 18),
                Point::new(4, 18),
                Point::new(4, 19),
                Point::new(5, 19),
            ],
        ),
        (
            PieceKind::J,
            Point::new(3, 18),
            [
                Point::new(3, 18),
                Point::new(3, 19),
                Point::new(4, 19),
                Point::new(5, 19),
            ],
        ),
        (
            PieceKind::L,
            Point::new(3, 18),
            [
                Point::new(5, 18),
                Point::new(3, 19),
                Point::new(4, 19),
                Point::new(5, 19),
            ],
        ),
    ];

    /// Render an orientation back into the §9.3 picture, so a failure shows the
    /// shape that is wrong rather than a bitmask.
    fn picture(kind: PieceKind, rotation: Rotation) -> String {
        let size = kind.box_size();
        let cells = kind.cells(rotation);
        (0..size)
            .map(|row| {
                (0..size)
                    .map(|col| {
                        if cells.contains(&Point::new(col, row)) {
                            kind.glyph()
                        } else {
                            '.'
                        }
                    })
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn every_orientation_has_exactly_four_minos() {
        for kind in PieceKind::ALL {
            for rotation in Rotation::ALL {
                let cells = kind.cells(rotation);
                let mut distinct = cells;
                distinct.sort_by_key(|p| (p.y, p.x));
                distinct.windows(2).for_each(|w| {
                    assert_ne!(w[0], w[1], "{kind:?} {rotation:?} repeats a mino");
                });
                let size = kind.box_size();
                for cell in cells {
                    assert!(
                        (0..size).contains(&cell.x) && (0..size).contains(&cell.y),
                        "{kind:?} {rotation:?} has a mino outside its {size}x{size} box",
                    );
                }
            }
        }
    }

    #[test]
    fn four_clockwise_turns_restore_the_pattern() {
        for kind in PieceKind::ALL {
            for rotation in Rotation::ALL {
                let mut turned = rotation;
                for _ in 0..4 {
                    turned = turned.cw();
                }
                assert_eq!(turned, rotation);
                assert_eq!(
                    kind.cells(turned),
                    kind.cells(rotation),
                    "{kind:?} does not return to itself after four turns",
                );
            }
        }
    }

    #[test]
    fn o_piece_is_identical_in_every_orientation() {
        // §9.3: `O` is the same in all four orientations, which is why it never
        // kicks (§9.5).
        for rotation in Rotation::ALL {
            assert_eq!(
                PieceKind::O.cells(rotation),
                PieceKind::O.cells(Rotation::North),
            );
        }
    }

    #[test]
    fn spawn_matches_the_spec_table() {
        for (kind, origin, minos) in SPAWN_TABLE {
            assert_eq!(kind.spawn_origin(), origin, "{kind:?} spawn origin");
            assert_eq!(
                kind.minos(origin, Rotation::North),
                minos,
                "{kind:?} spawn minos\n{}",
                picture(kind, Rotation::North),
            );
        }
    }

    #[test]
    fn every_piece_spawns_with_its_lowest_minos_in_row_19() {
        // §9.4: "Every piece therefore spawns with its lowest minos in row 19".
        for kind in PieceKind::ALL {
            let minos = kind.minos(kind.spawn_origin(), Rotation::North);
            let lowest = minos.iter().map(|p| p.y).max().unwrap();
            assert_eq!(
                lowest, 19,
                "{kind:?} spawns with its lowest mino in {lowest}"
            );
            assert!(
                minos.iter().all(|p| p.y >= 0),
                "{kind:?} spawns above row 0, which §9.1 forbids",
            );
        }
    }

    #[test]
    fn patterns_match_the_spec_pictures() {
        // A spot check in the spec's own notation: if a bitmask nibble is
        // transposed, this is where it shows.
        assert_eq!(picture(PieceKind::T, Rotation::North), ".T.\nTTT\n...");
        assert_eq!(picture(PieceKind::T, Rotation::East), ".T.\n.TT\n.T.");
        assert_eq!(picture(PieceKind::T, Rotation::South), "...\nTTT\n.T.");
        assert_eq!(picture(PieceKind::T, Rotation::West), ".T.\nTT.\n.T.");
        assert_eq!(
            picture(PieceKind::I, Rotation::North),
            "....\nIIII\n....\n....",
        );
        assert_eq!(
            picture(PieceKind::I, Rotation::East),
            "..I.\n..I.\n..I.\n..I.",
        );
        assert_eq!(picture(PieceKind::S, Rotation::North), ".SS\nSS.\n...");
        assert_eq!(picture(PieceKind::Z, Rotation::North), "ZZ.\n.ZZ\n...");
        assert_eq!(picture(PieceKind::J, Rotation::North), "J..\nJJJ\n...");
        assert_eq!(picture(PieceKind::L, Rotation::North), "..L\nLLL\n...");
    }

    #[test]
    fn colours_match_the_spec_table() {
        assert_eq!(PieceKind::I.colour().rgb(), (0x00, 0xF0, 0xF0));
        assert_eq!(PieceKind::O.colour().rgb(), (0xF0, 0xF0, 0x00));
        assert_eq!(PieceKind::T.colour().rgb(), (0xA0, 0x00, 0xF0));
        assert_eq!(PieceKind::S.colour().rgb(), (0x00, 0xF0, 0x00));
        assert_eq!(PieceKind::Z.colour().rgb(), (0xF0, 0x00, 0x00));
        assert_eq!(PieceKind::J.colour().rgb(), (0x00, 0x00, 0xF0));
        assert_eq!(PieceKind::L.colour().rgb(), (0xF0, 0xA0, 0x00));
        for kind in PieceKind::ALL {
            assert_eq!(kind.glyph(), format!("{kind:?}").chars().next().unwrap());
        }
    }
}
