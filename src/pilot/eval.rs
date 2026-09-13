//! §P5's features, and the integer evaluation over them.
//!
//! **Integers only.** No floating point anywhere, for §9.9's reason one layer
//! up: a plan must be identical on every target and in every build profile, and
//! a float would make "the same seed and settings give the same game" a
//! property of the optimiser.
//!
//! Two halves, kept apart on purpose. [`Board`] is what the stack *is* — ten
//! integers over the visible field, each measurable on its own and each tested
//! on its own. [`Outcome`] is what the branch *did* — the rows it cleared, the
//! chain it left running, whether it ended in §9.16. The full evaluation is
//! applied **at leaves only** (§P5), so a persistent defect is charged once
//! rather than once per ply; interior plies are scored from the core's own
//! events and score deltas, which is what actually happened rather than a
//! second opinion about it.
//!
//! [`Weights`] is a separate structure from both, and that is also §P5's: the
//! features are facts about a position and the weights are an opinion about
//! them. Tuning replaces the opinion and leaves the facts alone.

use std::cmp::Reverse;

use crate::core::{PieceKind, VIEW_HEIGHT, VIEW_WIDTH};

/// The visible field, exactly as `GameView::rows` reports it (§12.7).
///
/// The *visible* field and no more: the buffer zone is not on the screen, so it
/// is not something a fair planner measures (§P2.1).
pub type Field = [[Option<PieceKind>; VIEW_WIDTH]; VIEW_HEIGHT];

/// Columns in the visible field.
pub const COLUMNS: usize = VIEW_WIDTH;
/// Rows in the visible field.
pub const ROWS: usize = VIEW_HEIGHT;

/// What the stack is: §P5's board features, each an integer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Board {
    /// Empty cells with a filled cell above them in the same column.
    pub holes: i32,
    /// Filled cells above each hole, summed — a hole three deep costs three
    /// times what a hole under one cell does, because digging it out costs
    /// that.
    pub covered: i32,
    /// The sum of the column heights.
    pub aggregate_height: i32,
    /// The tallest column.
    pub max_height: i32,
    /// The sum of the height differences between adjacent columns.
    pub bumpiness: i32,
    /// Filled↔empty changes across each row, both walls counted as filled.
    pub row_transitions: i32,
    /// Filled↔empty changes down each column, the floor counted as filled and
    /// the space above the stack as empty.
    pub column_transitions: i32,
    /// Filled cells with a hole somewhere below them in the same column. A
    /// blockade is counted once however many holes are under it, which is what
    /// distinguishes it from [`Board::covered`].
    pub blockades: i32,
    /// The depth of each column below both its neighbours, summed — **less the
    /// deepest single column**, which is exempt. A wall is a neighbour of full
    /// height, so a well against the edge counts.
    ///
    /// The exemption is what lets a planner keep **one** open column, which is
    /// the shape a quad is scored out of (§9.14, and [`Weights::default`]). A
    /// planner charged for that column can never build the thing the scoring
    /// table is trying to buy: the well costs its depth every ply it exists and
    /// pays only once. Every *other* well is still a defect, and the deepest is
    /// exempt rather than free-to-a-limit so that two wells are as bad as they
    /// have always been.
    pub wells: i32,
    /// Rows that are filled in every column but the one [`Board::wells`]
    /// exempts, which is empty there — **capped at four**, because that is what
    /// one `I` can clear (§9.14).
    ///
    /// This is the quad made visible at every ply. §P6.1 looks two plies ahead,
    /// so the quad itself is invisible until the stack that earns one already
    /// exists — which is why rewarding `clears[4]` does nothing at all, and why
    /// the lever has to be *progress* rather than the prize. Four is a cap and
    /// not a scale: a well five deep is no better than one four deep, because
    /// the fifth row is one no `I` reaches.
    pub well_rows: i32,
}

/// What a branch did: the outcome features of §P5's table.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Outcome {
    /// Rows cleared by the lock (§9.12).
    pub lines: i32,
    /// The clears the branch made, counted by how many rows each took: a quad is
    /// `clears[4]` and index 0 is never used.
    ///
    /// [`Outcome::lines`] is the same event summed, and the pair is two features
    /// rather than one for the reason §9.14 gives: a quad pays **800** and four
    /// singles **400**, so a figure linear in rows cannot tell apart the two
    /// things the scoring table prices most differently.
    pub clears: [i32; 5],
    /// Whether the branch ended in §9.16.
    pub topped_out: bool,
    /// §9.15's combo counter as the branch leaves it. It starts at -1 and a
    /// lock that clears nothing returns it there, so only a positive value is
    /// worth anything.
    pub combo: i32,
    /// Whether §9.15's chain is live as the branch leaves it.
    pub back_to_back: bool,
    /// Whether the branch earned §9.15's perfect-clear bonus.
    pub perfect_clear: bool,
}

/// A position, as the planner sees it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Features {
    pub board: Board,
    pub outcome: Outcome,
}

/// What each feature is worth. Integers, and deliberately not a float in
/// disguise: these are the numbers, not a scaling of them.
///
/// **Hand-tuned, and provisional.** §P8's benchmark is what says whether a
/// change to them helped; `PILOT-PLAN.md` keeps automated tuning out of scope
/// and would have it consume that report rather than replace it. A weight
/// change moves the plan snapshots by design (§P8.3) — which is the opposite of
/// what §17.2's I1 snapshot is for, and the reason the two do not live in the
/// same file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Weights {
    pub lines: i32,
    /// What a clear is worth for its *kind*, on top of [`Weights::lines`] for
    /// its rows, indexed by rows cleared (§9.14).
    pub clears: [i32; 5],
    pub holes: i32,
    pub covered: i32,
    pub aggregate_height: i32,
    pub max_height: i32,
    pub bumpiness: i32,
    pub row_transitions: i32,
    pub column_transitions: i32,
    pub blockades: i32,
    pub wells: i32,
    pub well_rows: i32,
    pub top_out: i32,
    pub combo: i32,
    pub back_to_back: i32,
    pub perfect_clear: i32,
}

impl Default for Weights {
    /// The opinion (§P5).
    ///
    /// Negative for everything that makes a stack harder to play out of, and
    /// the two that dominate are holes and column transitions: a hole cannot be
    /// filled without first removing what is on top of it, and a jagged surface
    /// is what produces holes. `top_out` is large enough that no arrangement of
    /// the others can buy one.
    ///
    /// **`clears` is what makes it play for score rather than for rows**, and it
    /// reads backwards until you see what it is for: a single is *punished*, at
    /// -1,200 against the +340 its row earns, so the net price of clearing one
    /// row is negative and the planner would rather stack. §9.14 is why — four
    /// singles pay 400 and a quad 800, a chained quad 1,200 — so a planner
    /// indifferent between them leaves two thirds of the score on the table.
    /// The band is narrow: -1,200 is worth ~27% more score than a flat opinion
    /// at no cost in lines or top-outs, and -5,000 tops out seven games in eight
    /// because a stack nothing is allowed to clear is a stack that reaches the
    /// ceiling.
    ///
    /// Rewarding the quad instead did **nothing at all** when it was first
    /// tried, and that measurement is the whole reason `well_rows` exists: at
    /// §P6.1's two plies a quad is invisible until the stack that earns one
    /// already exists, so `clears[4]` was never collected and the figures did
    /// not move by a single point. The lever has to be one the planner can see
    /// at *every* ply. "Do not spend rows cheaply" is one, and
    /// [`Board::well_rows`] — progress towards the quad rather than the quad —
    /// is the other, and the larger: it is worth **+45%** score and turns 35
    /// quads in 16,000 pieces into 1,262.
    ///
    /// With it, `clears[4]` stops being inert and starts paying: removing the
    /// 10,000 now costs 19%, where before it cost nothing. The two are one
    /// decision — the bonus is the prize and `well_rows` is what makes the
    /// planner able to see it coming.
    fn default() -> Self {
        Self {
            lines: 340,
            clears: [0, -1_200, -600, 0, 10_000],
            holes: -790,
            covered: -30,
            aggregate_height: -51,
            max_height: -100,
            bumpiness: -30,
            row_transitions: -320,
            column_transitions: -930,
            blockades: -20,
            wells: -340,
            well_rows: 2_000,
            top_out: -1_000_000,
            combo: 20,
            back_to_back: 2_000,
            perfect_clear: 1_000,
        }
    }
}

impl Board {
    /// Measure a field (§P5).
    ///
    /// One pass for the heights and one per family after it: the field is 200
    /// cells and this runs once per leaf, so it is written to be read rather
    /// than to be clever.
    pub fn of(field: &Field) -> Self {
        let heights = heights(field);
        let (holes, covered, blockades) = holes(field, &heights);
        Self {
            holes,
            covered,
            blockades,
            aggregate_height: heights.iter().sum(),
            max_height: heights.iter().copied().max().unwrap_or(0),
            bumpiness: heights
                .windows(2)
                .map(|pair| (pair[0] - pair[1]).abs())
                .sum(),
            row_transitions: row_transitions(field),
            column_transitions: column_transitions(field),
            wells: wells(&heights),
            well_rows: well_rows(field, &heights),
        }
    }
}

impl Features {
    /// Score a position against an opinion of it — a **leaf**, which is the only
    /// place the board is charged for (§P5).
    pub fn evaluate(&self, weights: &Weights) -> i32 {
        self.board
            .evaluate(weights)
            .saturating_add(self.outcome.evaluate(weights))
    }
}

impl Board {
    /// What the stack is worth (§P5).
    ///
    /// Saturating throughout: the weights are a player's to set, and a
    /// preposterous one must give a preposterous answer rather than a wrapped
    /// one that happens to look attractive.
    pub fn evaluate(&self, weights: &Weights) -> i32 {
        let mut score: i32 = 0;
        let mut add = |weight: i32, count: i32| {
            score = score.saturating_add(weight.saturating_mul(count));
        };
        add(weights.holes, self.holes);
        add(weights.covered, self.covered);
        add(weights.aggregate_height, self.aggregate_height);
        add(weights.max_height, self.max_height);
        add(weights.bumpiness, self.bumpiness);
        add(weights.row_transitions, self.row_transitions);
        add(weights.column_transitions, self.column_transitions);
        add(weights.blockades, self.blockades);
        add(weights.wells, self.wells);
        add(weights.well_rows, self.well_rows);
        score
    }
}

impl Outcome {
    /// What the branch did, in full: the events **and** the state they left
    /// behind. A leaf's half of [`Features::evaluate`].
    pub fn evaluate(&self, weights: &Weights) -> i32 {
        let mut score = self.interior(weights);
        // §9.15's combo counter rests at -1, which is not a penalty.
        score = score.saturating_add(weights.combo.saturating_mul(self.combo.max(0)));
        score.saturating_add(weights.back_to_back * i32::from(self.back_to_back))
    }

    /// What the branch did, for an **interior** ply: what happened, and not the
    /// state it happens to be in on the way past (§P5).
    ///
    /// §P5 scores interior plies "from the core's own events and score deltas",
    /// and the three here are exactly the events — a clear, §9.15's perfect
    /// clear bonus, and §9.16. Combo and back-to-back are deliberately **not**
    /// among them, because they are *state* rather than events and the leaf is
    /// where state is read: charging a chain at every ply it survives would pay
    /// for one back-to-back two or three times over, and a planner paid twice
    /// for the same thing over-values it by exactly the depth it is searching.
    pub fn interior(&self, weights: &Weights) -> i32 {
        let mut score: i32 = 0;
        let mut add = |weight: i32, count: i32| {
            score = score.saturating_add(weight.saturating_mul(count));
        };
        add(weights.lines, self.lines);
        for (rows, &count) in self.clears.iter().enumerate() {
            add(weights.clears[rows], count);
        }
        add(weights.top_out, i32::from(self.topped_out));
        add(weights.perfect_clear, i32::from(self.perfect_clear));
        score
    }
}

/// The height of each column: the number of rows from the floor up to and
/// including its topmost filled cell. Zero for an empty column.
///
/// A hole does not reduce a column's height — that is what makes height and
/// holes two features rather than one.
fn heights(field: &Field) -> [i32; COLUMNS] {
    let mut heights = [0; COLUMNS];
    for (col, height) in heights.iter_mut().enumerate() {
        for (row, cells) in field.iter().enumerate() {
            if cells[col].is_some() {
                *height = (ROWS - row) as i32;
                break;
            }
        }
    }
    heights
}

/// Holes, the depth they are covered to, and the blockades over them.
///
/// All three come out of one downward pass per column, because all three are
/// about the same thing: what is stacked on top of what.
fn holes(field: &Field, heights: &[i32; COLUMNS]) -> (i32, i32, i32) {
    let (mut holes, mut covered, mut blockades) = (0, 0, 0);
    for (col, &height) in heights.iter().enumerate() {
        // The column from its own surface down: everything above that is empty
        // by definition and is not a hole.
        let stack = &field[ROWS - height as usize..];
        let mut above = 0;
        let mut under = 0;
        for cells in stack {
            if cells[col].is_some() {
                above += 1;
            } else {
                holes += 1;
                covered += above;
                under += 1;
            }
        }
        if under > 0 {
            // Every filled cell in the column that has a hole below it. Counted
            // from the bottom up, so the ones under the lowest hole are not.
            let mut seen_hole = false;
            for cells in stack.iter().rev() {
                match cells[col].is_some() {
                    true if seen_hole => blockades += 1,
                    true => {}
                    false => seen_hole = true,
                }
            }
        }
    }
    (holes, covered, blockades)
}

/// Filled↔empty changes across each row, with both walls counted as filled
/// (§9.1: out of bounds is solid to the left and right).
///
/// A completed row costs nothing and an empty row costs two, so this is a
/// measure of how *unlike* a finished row each row is.
fn row_transitions(field: &Field) -> i32 {
    let mut transitions = 0;
    for row in field {
        let mut previous = true; // the left wall
        for cell in row {
            let filled = cell.is_some();
            transitions += i32::from(filled != previous);
            previous = filled;
        }
        transitions += i32::from(!previous); // the right wall
    }
    transitions
}

/// Filled↔empty changes down each column, with the floor counted as filled and
/// everything above the stack as empty (§9.1).
fn column_transitions(field: &Field) -> i32 {
    let mut transitions = 0;
    for col in 0..COLUMNS {
        let mut previous = false; // above the field
        for row in field {
            let filled = row[col].is_some();
            transitions += i32::from(filled != previous);
            previous = filled;
        }
        transitions += i32::from(!previous); // the floor
    }
    transitions
}

/// How far each column sits below the lower of its two neighbours.
///
/// The walls are neighbours of full height, so a well down the side of the
/// board is a well; otherwise the one place an `I` is always welcome would
/// measure as flat.
fn well_depths(heights: &[i32; COLUMNS]) -> [i32; COLUMNS] {
    let mut each = [0i32; COLUMNS];
    for col in 0..COLUMNS {
        let left = if col == 0 {
            ROWS as i32
        } else {
            heights[col - 1]
        };
        let right = if col == COLUMNS - 1 {
            ROWS as i32
        } else {
            heights[col + 1]
        };
        each[col] = (left.min(right) - heights[col]).max(0);
    }
    each
}

/// The column [`Board::wells`] exempts, when there is one.
///
/// The deepest, and the leftmost of the deepest when several tie. The tie-break
/// is not a preference but it is a **fixed** one, because two builds that chose
/// differently here would measure the same board differently (§P5).
fn well_column(heights: &[i32; COLUMNS]) -> Option<usize> {
    let depths = well_depths(heights);
    let col = (0..COLUMNS).max_by_key(|&col| (depths[col], Reverse(col)))?;
    (depths[col] > 0).then_some(col)
}

/// Rows complete but for the exempt well column, capped at four (§9.14).
///
/// The cap is arithmetic rather than taste: an `I` is four cells, so the fifth
/// row of a well is one nothing can clear. Uncapped, a planner paid by the row
/// goes on digging — which was measured, and is how a stack that may never be
/// cleared reaches the ceiling.
fn well_rows(field: &Field, heights: &[i32; COLUMNS]) -> i32 {
    let Some(well) = well_column(heights) else {
        return 0;
    };
    let ready = field
        .iter()
        .filter(|row| {
            row[well].is_none()
                && row
                    .iter()
                    .enumerate()
                    .all(|(col, cell)| col == well || cell.is_some())
        })
        .count();
    (ready as i32).min(4)
}

fn wells(heights: &[i32; COLUMNS]) -> i32 {
    let each = well_depths(heights);
    // The deepest single well is exempt: it is the column a quad is scored out
    // of, and a planner charged for it can never build one. See the field's own
    // documentation for why that is a feature and not a blind spot.
    let deepest = each.iter().copied().max().unwrap_or(0);
    each.iter().sum::<i32>() - deepest
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A field from a picture of its bottom rows, the last string being the
    /// floor. `.` is empty and anything else is filled.
    ///
    /// Written out here rather than borrowed from the core's own fixtures:
    /// `src/pilot/` may name the core's façade and §P2.3's seam, and nothing
    /// else inside it (§P3.4).
    fn field(rows: &[&str]) -> Field {
        let mut field: Field = [[None; COLUMNS]; ROWS];
        for (i, row) in rows.iter().rev().enumerate() {
            assert_eq!(row.chars().count(), COLUMNS, "{row:?} is not 10 wide");
            for (col, cell) in row.chars().enumerate() {
                if cell != '.' {
                    field[ROWS - 1 - i][col] = Some(PieceKind::I);
                }
            }
        }
        field
    }

    #[test]
    fn an_empty_field_measures_as_nothing_but_its_walls() {
        // The baseline every other case is read against. Both transition
        // counts are non-zero on an empty board by construction: twenty rows
        // with two walls each, and ten columns with a floor each.
        let board = Board::of(&field(&[]));
        assert_eq!(
            board,
            Board {
                holes: 0,
                covered: 0,
                aggregate_height: 0,
                max_height: 0,
                bumpiness: 0,
                row_transitions: 2 * ROWS as i32,
                column_transitions: COLUMNS as i32,
                blockades: 0,
                wells: 0,
                well_rows: 0,
            },
        );
    }

    #[test]
    fn height_is_measured_to_the_topmost_filled_cell() {
        let board = Board::of(&field(&["#.........", "#.......##", "##########"]));
        assert_eq!(board.max_height, 3);
        // Column 0 at 3, columns 1..=7 at 1 each, and the last two at 2.
        assert_eq!(board.aggregate_height, 3 + 7 + 2 + 2);
    }

    #[test]
    fn a_hole_is_an_empty_cell_with_something_over_it() {
        // One hole, under one cell: a cell missing from the middle of the floor
        // with a roof over it.
        let board = Board::of(&field(&["####.#####", "##########"]));
        assert_eq!(board.holes, 0, "nothing is over the notch yet");

        let board = Board::of(&field(&["##########", "####.#####"]));
        assert_eq!(board.holes, 1);
        assert_eq!(board.covered, 1, "one filled cell above it");
        assert_eq!(board.blockades, 1, "and that cell is a blockade");
    }

    #[test]
    fn covered_depth_counts_every_cell_over_a_hole() {
        // Three cells stacked over one hole: one hole, three deep, three
        // blockades.
        let board = Board::of(&field(&[
            "....#.....",
            "....#.....",
            "....#.....",
            "..........",
        ]));
        assert_eq!((board.holes, board.covered, board.blockades), (1, 3, 3));
    }

    #[test]
    fn two_holes_under_one_stack_are_counted_separately() {
        // The case that tells `covered` and `blockades` apart. Column 4 holds,
        // from the top: two filled, a hole, one filled, a hole. The upper hole
        // has two cells over it and the lower has three, so `covered` is five;
        // the blockades are the three filled cells with a hole below them.
        let board = Board::of(&field(&[
            "....#.....",
            "....#.....",
            "..........",
            "....#.....",
            "..........",
        ]));
        assert_eq!(board.holes, 2);
        assert_eq!(board.covered, 2 + 3);
        assert_eq!(board.blockades, 3);
    }

    #[test]
    fn bumpiness_is_the_surface_and_not_the_height() {
        // A flat stack of any height is smooth; a staircase is not.
        let flat = Board::of(&field(&["##########", "##########"]));
        assert_eq!(flat.bumpiness, 0);
        assert_eq!(flat.aggregate_height, 20);

        let steps = Board::of(&field(&["#.........", "##........", "###......."]));
        // Heights 3, 2, 1, 0, 0, 0, 0, 0, 0, 0.
        assert_eq!(steps.bumpiness, 1 + 1 + 1);
    }

    #[test]
    fn row_transitions_count_the_walls() {
        // A row with a single gap in the middle costs four: wall, gap, gap,
        // wall. A completed row costs nothing, which is what makes filling one
        // worth something to a feature that is otherwise about texture.
        let one_gap = Board::of(&field(&["####.#####"]));
        let full = Board::of(&field(&["##########"]));
        assert_eq!(one_gap.row_transitions - full.row_transitions, 2);
        // Nineteen empty rows above it, at two each, and the floor itself.
        assert_eq!(full.row_transitions, 2 * (ROWS as i32 - 1));
    }

    #[test]
    fn column_transitions_count_the_floor() {
        // A column filled to the top of the stack has one transition — the
        // surface — and an empty one has one, the floor. A hole in the middle
        // of a column adds two.
        let solid = Board::of(&field(&["##########"]));
        assert_eq!(solid.column_transitions, COLUMNS as i32);
        let holed = Board::of(&field(&["##########", "####.#####"]));
        assert_eq!(holed.column_transitions - solid.column_transitions, 2);
    }

    #[test]
    fn a_well_is_a_column_below_both_its_neighbours() {
        // The classic shape: nine columns four high and a shaft at the edge
        // waiting for an `I`. It measures as **nothing**, because one such shaft
        // is exactly what the exemption is for.
        let board = Board::of(&field(&[
            ".#########",
            ".#########",
            ".#########",
            ".#########",
        ]));
        assert_eq!(board.wells, 0, "the one well a quad is scored out of");
        assert_eq!(board.holes, 0, "an open shaft is not a hole");

        // A second shaft is not exempt, and is measured against the shallower
        // neighbour. Four cells of it here, with the deeper one taken off.
        let board = Board::of(&field(&[
            ".####.####",
            ".####.####",
            ".####.####",
            ".####.####",
        ]));
        assert_eq!(board.wells, 4, "the second well is an ordinary defect");
    }

    #[test]
    fn one_well_is_exempt_and_the_deepest_is_the_one() {
        // The exemption takes off the *deepest* rather than the first, so that
        // adding a shallower well somewhere else can never reduce the total.
        let shallow = Board::of(&field(&["#####.####", "##########"]));
        assert_eq!(shallow.wells, 0, "one well, exempt");

        // Two wells, three deep and one deep: the three-deep one is taken off.
        let board = Board::of(&field(&["#.###.####", "#.########", "#.########"]));
        assert_eq!(board.wells, 1, "the deeper of the two is the exempt one");
    }

    #[test]
    fn the_weights_would_rather_have_a_quad_than_four_singles() {
        // §9.14 pays 800 for a quad and 400 for four singles, and this is the
        // feature that lets the planner tell them apart at all — `lines` alone
        // is linear in rows and scores the two identically. See
        // `Weights::default` for why the single is priced below its own row.
        let weights = Weights::default();
        let quad = Outcome {
            lines: 4,
            clears: [0, 0, 0, 0, 1],
            ..Outcome::default()
        };
        let four_singles = Outcome {
            lines: 4,
            clears: [0, 4, 0, 0, 0],
            ..Outcome::default()
        };
        assert!(quad.evaluate(&weights) > four_singles.evaluate(&weights));
        // ...and a single is worth less than not clearing at all, which is what
        // makes the planner stack instead of spending a row for 100 points.
        let nothing = Outcome::default();
        let single = Outcome {
            lines: 1,
            clears: [0, 1, 0, 0, 0],
            ..Outcome::default()
        };
        assert!(single.evaluate(&weights) < nothing.evaluate(&weights));
    }

    #[test]
    fn a_plateau_has_no_wells() {
        let board = Board::of(&field(&["##########", "##########"]));
        assert_eq!(board.wells, 0);
        let board = Board::of(&field(&["#####.....", "##########"]));
        assert_eq!(board.wells, 0, "a step down is not a well");
    }

    #[test]
    fn the_evaluation_is_the_weighted_sum_and_nothing_else() {
        // Each weight applies to its own feature, in integers. Checked by
        // moving one weight at a time against a fixed position.
        let features = Features {
            board: Board::of(&field(&["##########", "####.#####"])),
            outcome: Outcome {
                lines: 2,
                combo: 3,
                back_to_back: true,
                ..Outcome::default()
            },
        };
        let zero = Weights {
            lines: 0,
            clears: [0; 5],
            holes: 0,
            covered: 0,
            aggregate_height: 0,
            max_height: 0,
            bumpiness: 0,
            row_transitions: 0,
            column_transitions: 0,
            blockades: 0,
            wells: 0,
            well_rows: 0,
            top_out: 0,
            combo: 0,
            back_to_back: 0,
            perfect_clear: 0,
        };
        assert_eq!(features.evaluate(&zero), 0, "no opinion, no score");

        let lines_only = Weights { lines: 10, ..zero };
        assert_eq!(features.evaluate(&lines_only), 20);
        let holes_only = Weights { holes: -7, ..zero };
        assert_eq!(features.evaluate(&holes_only), -7);
        let combo_only = Weights { combo: 5, ..zero };
        assert_eq!(features.evaluate(&combo_only), 15);
        let chain_only = Weights {
            back_to_back: 11,
            ..zero
        };
        assert_eq!(features.evaluate(&chain_only), 11);
    }

    #[test]
    fn an_interior_ply_is_charged_for_what_happened_and_not_for_what_it_left() {
        // §P5, and the one arithmetic decision P6 had to make: the leaf pays for
        // the board and for §9.15's chain state, and an interior ply pays for
        // the events alone. A chain charged at every ply it survived would be
        // paid for once per ply, and a planner over-values a back-to-back by
        // exactly the depth it searches.
        let weights = Weights::default();
        let outcome = Outcome {
            lines: 2,
            clears: [0, 0, 1, 0, 0],
            combo: 3,
            back_to_back: true,
            perfect_clear: false,
            topped_out: false,
        };
        assert_eq!(
            outcome.interior(&weights),
            2 * weights.lines + weights.clears[2],
        );
        assert_eq!(
            outcome.evaluate(&weights),
            outcome.interior(&weights) + 3 * weights.combo + weights.back_to_back,
        );
        // ...and a leaf is the board's opinion of itself plus all of that.
        let features = Features {
            board: Board::of(&field(&["##########", "####.#####"])),
            outcome,
        };
        assert_eq!(
            features.evaluate(&weights),
            features.board.evaluate(&weights) + outcome.evaluate(&weights),
        );
    }

    #[test]
    fn a_resting_combo_counter_is_not_a_penalty() {
        // §9.15: the counter sits at -1 when no chain is running, and a
        // positive combo weight must not turn that into a cost.
        let weights = Weights {
            combo: 100,
            ..Weights::default()
        };
        let idle = Features::default();
        let resting = Features {
            outcome: Outcome {
                combo: -1,
                ..Outcome::default()
            },
            ..Features::default()
        };
        assert_eq!(idle.evaluate(&weights), resting.evaluate(&weights));
    }

    #[test]
    fn a_top_out_outweighs_everything_the_board_can_offer() {
        // §P5: no arrangement of the other features may buy one. The best board
        // there is, ended in §9.16, must still score below the worst board that
        // survives.
        let weights = Weights::default();
        let best = Features {
            outcome: Outcome {
                lines: 4,
                clears: [0, 0, 0, 0, 1],
                combo: 20,
                back_to_back: true,
                perfect_clear: true,
                topped_out: true,
            },
            ..Features::default()
        };
        let worst = Features {
            board: Board::of(&field(&[
                "#.#.#.#.#.",
                ".#.#.#.#.#",
                "#.#.#.#.#.",
                ".#.#.#.#.#",
                "#.#.#.#.#.",
            ])),
            ..Features::default()
        };
        assert!(
            best.evaluate(&weights) < worst.evaluate(&weights),
            "a top out is not a price worth paying",
        );
    }

    #[test]
    fn a_clean_stack_beats_the_same_stack_with_a_hole_in_it() {
        // The one judgement the starting weights have to get right, or nothing
        // built on them can play at all.
        //
        // The covering row is load-bearing in the fixture and was not always
        // here: `#####.####` under `#####.....` leaves column 5 open to the
        // sky, which is a *well* and not a hole, and since `well_rows` it is
        // one the planner is right to prefer. A hole is a cell with something
        // on top of it, so the fixture puts something on top of it.
        let weights = Weights::default();
        let clean = Features {
            board: Board::of(&field(&["#####.....", "##########"])),
            ..Features::default()
        };
        let holed = Features {
            board: Board::of(&field(&["######....", "#####.####"])),
            ..Features::default()
        };
        assert_eq!(holed.board.holes, 1, "the fixture holds a real hole");
        assert_eq!(clean.board.holes, 0);
        assert!(clean.evaluate(&weights) > holed.evaluate(&weights));
    }

    #[test]
    fn a_well_ready_row_is_one_short_of_a_quad_row() {
        // §9.14's quad, made visible at every ply. A row filled but for the
        // exempt well column is progress towards the one clear the scoring
        // table pays 800 for; a row with its gap anywhere else is not.
        let ready = Board::of(&field(&["#########.", "#########.", "#########."]));
        assert_eq!(ready.well_rows, 3);
        assert_eq!(ready.holes, 0, "an open column is not a hole");

        // Elsewhere is not the well, so it is not progress.
        let scattered = Board::of(&field(&["#########.", "########.#", "#########."]));
        assert_eq!(scattered.well_rows, 2, "only the rows gapped at the well");
    }

    #[test]
    fn a_well_deeper_than_four_earns_nothing_more() {
        // The cap is §9.14's arithmetic and not a taste: an `I` is four cells,
        // so the fifth row of a well is one nothing can clear. Without the cap
        // a planner would go on digging, which is how a stack that may never be
        // cleared reaches the ceiling.
        let four = Board::of(&field(&["#########."; 4]));
        let seven = Board::of(&field(&["#########."; 7]));
        assert_eq!(four.well_rows, 4);
        assert_eq!(seven.well_rows, 4, "capped, not scaled");
    }

    #[test]
    fn a_flat_field_has_no_well_and_no_well_rows() {
        // There is no exempt column until a column is actually lower than its
        // neighbours, so nothing on an empty or a flat field is credited to a
        // well that does not exist.
        assert_eq!(Board::of(&field(&[])).well_rows, 0);
        assert_eq!(Board::of(&field(&["####..####"])).well_rows, 0);
    }

    #[test]
    fn the_evaluation_holds_no_floating_point() {
        // §P5 in the only place a test can check it: every field of every
        // structure here is an integer or a boolean, so the arithmetic has
        // nowhere to become a float. The types are the assertion; this is the
        // canary that the answer is exactly reproducible.
        let features = Features {
            board: Board::of(&field(&["##########", "####.#####"])),
            outcome: Outcome {
                lines: 1,
                ..Outcome::default()
            },
        };
        let score = features.evaluate(&Weights::default());
        assert_eq!(score, features.evaluate(&Weights::default()));
        assert_eq!(score, features.clone().evaluate(&Weights::default()));
    }
}
