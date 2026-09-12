//! The fork a search runs on (`PILOT.md` §P2.3).
//!
//! This is the whole of the information boundary between the automated player
//! and the game it plays, and it is **a type rather than a convention**: an
//! audit proves nothing about the commit after it, which is the trade §17.3's
//! A10 already made for the core's façade.
//!
//! `Game` is `Clone`, and a clone carries the real generator, the real bag and
//! every piece past the preview. A searcher holding one is one autocomplete
//! away from cheating silently, and no test would notice until somebody read
//! the diff. So the searcher never holds a `Game`: it holds a [`SearchGame`],
//! whose randomiser has been *replaced* by a scripted queue the caller built
//! from what it could see (§P2.1), and which refuses to spawn past the end of
//! it.
//!
//! **§17.3's A10 is unchanged.** `core/mod.rs` re-exports this with
//! `pub(crate) use`, never `pub use`, so the core's public surface is exactly
//! what it was and a §19 client is handed exactly what it was handed before.

use crate::core::events::GameEvent;
use crate::core::game::{Game, PlayState, TickInput};
use crate::core::piece::PieceKind;
use crate::core::view::GameView;

impl Game {
    /// A fork for search: these rules and this board, with the randomiser
    /// replaced by exactly the pieces the caller supplies (`PILOT.md` §P2.3).
    ///
    /// The queue is the caller's own — the visible preview, then hypotheses
    /// drawn from the bag remainder it has inferred from pieces already dealt
    /// (§P2.4). Changing the live game's hidden future, with visible
    /// information held constant, therefore cannot change what a fork does.
    ///
    /// This is `SearchGame`'s only constructor.
    pub(crate) fn fork(&self, queue: &[PieceKind]) -> SearchGame {
        SearchGame {
            game: self.with_scripted_bag(queue),
        }
    }
}

/// A game a search may run, and nothing it may read.
///
/// What it does **not** expose is the point of it: no `bag_remaining`, no
/// `preview` past the queue it was given, and no way back to the `Game`
/// inside it. §P2.3's list is an **upper bound** on this surface rather than a
/// shopping list, so each accessor lands with the stage that reads it — the
/// pose and the hold state with P3's placement generator, which is the first
/// thing that has a use for them. Nothing here is ever wider than that list.
pub(crate) struct SearchGame {
    game: Game,
}

impl SearchGame {
    /// Advance one tick, exactly as the live game would (§15.1).
    ///
    /// The same `Game::tick`, under the same rules: a placement that replays
    /// here is a placement the game will perform, which is the reason
    /// `PILOT-PLAN.md` reuses `Game` rather than writing a second simulator.
    pub(crate) fn tick(&mut self, input: &TickInput, out: &mut Vec<GameEvent>) {
        self.game.tick(input, out);
    }

    pub(crate) fn state(&self) -> PlayState {
        self.game.state()
    }

    /// The board and the figures, as §12.7 reports them. `next` here is the
    /// caller's own scripted queue, not a bag's.
    pub(crate) fn view(&self) -> GameView {
        self.game.view()
    }

    /// What is left of the scripted queue — the caller's own pieces.
    pub(crate) fn scripted(&self) -> &[PieceKind] {
        self.game.scripted()
    }

    /// Whether the scripted queue ran out and the fork stopped spawning.
    ///
    /// A fork that has run out is not over and has not topped out: it is simply
    /// at the end of what the caller told it about, and a search that reaches
    /// this has reached its own horizon.
    pub(crate) fn exhausted(&self) -> bool {
        self.game.bag_exhausted()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::game::tests::{new_game, place, set_matrix};
    use crate::core::game::{Action, Shift};
    use crate::core::geometry::{Point, Rotation};
    use crate::core::matrix::tests::from_bottom_rows;
    use crate::core::piece::PieceKind::{I, J, L, O, S, T, Z};

    /// Tick until the piece in play locks and the next one is up, or until the
    /// fork stops dealing. Every spawn it saw, in order.
    fn drop_pieces(fork: &mut SearchGame, count: usize) -> Vec<PieceKind> {
        let mut spawned = Vec::new();
        let mut events = Vec::new();
        for _ in 0..count {
            events.clear();
            fork.tick(&TickInput::action(Action::HardDrop), &mut events);
            // The entry delay is 0 by default (§6.6), so the next piece is
            // already in play; a non-zero one would need the wait.
            while fork.view().current.is_none()
                && !fork.exhausted()
                && fork.state() != PlayState::ToppedOut
            {
                fork.tick(&TickInput::default(), &mut events);
            }
            spawned.extend(events.iter().filter_map(|event| match event {
                GameEvent::PieceSpawned(kind) => Some(*kind),
                _ => None,
            }));
        }
        spawned
    }

    #[test]
    fn a_fork_deals_the_callers_queue_and_nothing_else() {
        // §P2.3. The live game's generator is still in there in the sense that
        // the fork was cloned from it — but it is not *reachable*, because the
        // bag it would have dealt from has been replaced.
        let game = new_game(42);
        let mut fork = game.fork(&[O, O, O]);
        assert_eq!(drop_pieces(&mut fork, 3), vec![O, O, O]);
    }

    #[test]
    fn a_fork_cannot_see_a_hidden_future() {
        // §P9's C2, at the level the type settles it: the same visible state
        // and the same scripted queue must give the same game, whatever the
        // live game's own bag was about to deal. Twenty seeds deal twenty
        // different futures, and the forks are indistinguishable.
        let board = from_bottom_rows(&["####.#####", "###...####", "T........."]);
        let mut views = Vec::new();
        for seed in 0..20u64 {
            let mut game = new_game(seed);
            set_matrix(&mut game, board.clone());
            place(&mut game, T, Point::new(3, 20), Rotation::North);
            let mut fork = game.fork(&[I, L, S]);
            assert_eq!(drop_pieces(&mut fork, 4), vec![I, L, S]);
            views.push(fork.view());
        }
        let first = &views[0];
        for (seed, view) in views.iter().enumerate() {
            assert_eq!(view, first, "seed {seed} played a different game");
        }
        // ...and the live games really did differ, or the assertion above is
        // about nothing.
        let futures: Vec<_> = (0..20u64)
            .map(|seed| new_game(seed).preview().collect::<Vec<_>>())
            .collect();
        assert!(
            futures.iter().any(|preview| preview != &futures[0]),
            "the seeds must not all deal the same preview",
        );
    }

    #[test]
    fn a_fork_runs_out_rather_than_inventing_a_piece() {
        // §P2.3: "the search stops at its own horizon because the object it
        // runs on cannot go past it." Two pieces given, two played, and then
        // the fork sits there.
        let game = new_game(7);
        let mut fork = game.fork(&[J, Z]);
        assert_eq!(fork.scripted(), [J, Z]);
        assert!(!fork.exhausted());

        // The piece already in play is the live game's, so three locks empty a
        // queue of two.
        let spawned = drop_pieces(&mut fork, 3);
        assert_eq!(spawned, vec![J, Z], "and nothing after them");
        assert!(fork.exhausted());
        assert_eq!(fork.scripted(), []);
        assert_eq!(fork.view().current, None, "nothing is in play");
        assert_eq!(fork.state(), PlayState::Entry);

        // It stays that way: an exhausted fork is quiet, not broken.
        let mut events = Vec::new();
        for _ in 0..120 {
            fork.tick(&TickInput::default(), &mut events);
        }
        assert!(events.is_empty(), "and raises nothing: {events:?}");
        assert_eq!(fork.view().current, None);
        assert_ne!(
            fork.state(),
            PlayState::ToppedOut,
            "running out is not a top out"
        );
    }

    #[test]
    fn a_fork_carries_the_position_it_was_taken_from() {
        // The board, the piece in play, the hold slot and the figures: a search
        // that started from a different position would answer a different
        // question (§P2.3).
        let mut game = new_game(11);
        set_matrix(&mut game, from_bottom_rows(&["##########", "####.#####"]));
        place(&mut game, S, Point::new(3, 24), Rotation::East);
        let fork = game.fork(&[T]);
        let (before, after) = (game.view(), fork.view());
        assert_eq!(after.rows, before.rows, "the board");
        assert_eq!(after.current, before.current, "the piece in play");
        assert_eq!(after.ghost, before.ghost);
        assert_eq!(after.hold, before.hold, "the hold slot");
        assert_eq!(after.hold_locked, before.hold_locked);
        assert_eq!(after.score, before.score);
        assert_eq!(after.level, before.level);
        assert_eq!(after.lines, before.lines);
        assert_eq!(after.combo, before.combo);
        assert_eq!(after.back_to_back, before.back_to_back);
        assert_eq!(fork.state(), game.state());
        // The one thing that is deliberately *not* carried over: what is
        // coming. The caller said, and the caller is all there is (§P2.3).
        assert_eq!(after.next, vec![T]);
    }

    #[test]
    fn forking_does_not_touch_the_game_it_was_taken_from() {
        // `fork(&self)` settles the fields at compile time; what this adds is
        // the randomiser, whose state no view shows — the same thing T15 checks
        // of `Game::view`.
        let mut game = new_game(13);
        let mut untouched = game.clone();
        for _ in 0..20 {
            let mut fork = game.fork(&[I, O, T, S, Z, J, L]);
            drop_pieces(&mut fork, 7);
        }
        for _ in 0..300 {
            let input = TickInput::action(Action::HardDrop);
            game.tick(&input, &mut Vec::new());
            untouched.tick(&input, &mut Vec::new());
        }
        assert_eq!(game.view(), untouched.view());
        assert_eq!(game.debug(), untouched.debug(), "the bag included");
    }

    #[test]
    fn a_fork_plays_by_the_rules_it_was_forked_with() {
        // `PILOT-PLAN.md`'s first decision: reuse `Game`, do not write a second
        // simulator. A fork clears a line the same way the game does, and the
        // score and the level move with it.
        let mut game = new_game(17);
        set_matrix(&mut game, from_bottom_rows(&["####.#####"]));
        place(&mut game, I, Point::new(2, 36), Rotation::East);
        let mut fork = game.fork(&[O, O]);
        let mut events = Vec::new();
        fork.tick(&TickInput::action(Action::HardDrop), &mut events);
        assert!(
            events
                .iter()
                .any(|event| matches!(event, GameEvent::LinesCleared { .. })),
            "the fork cleared the row: {events:?}",
        );
        assert_eq!(fork.state(), PlayState::Clearing);
        assert!(fork.view().score > 0);
    }

    #[test]
    fn a_forks_shifts_and_rotations_are_the_games() {
        // Not a smoke test for its own sake: P3 generates placements as input
        // sequences, and every one of them is replayed through this.
        let game = new_game(19);
        let mut fork = game.fork(&[T]);
        let before = fork.view().current.expect("a piece is in play");
        fork.tick(&TickInput::shift(Shift::Left), &mut Vec::new());
        let moved = fork.view().current.expect("still falling");
        for (after, before) in moved.cells.iter().zip(before.cells) {
            assert_eq!((after.0 + 1, after.1), before, "one cell left");
        }
        fork.tick(&TickInput::action(Action::RotateCw), &mut Vec::new());
        let rotated = fork.view().current.expect("still falling");
        assert_ne!(rotated.cells, moved.cells, "and it turned");
        assert_eq!(rotated.kind, before.kind, "without changing piece");
    }
}
