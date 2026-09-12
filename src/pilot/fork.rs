//! The object a search runs on (`PILOT.md` §P2.3).
//!
//! [`Fork`] is the planner's side of the information boundary and
//! `core::SearchGame` is the core's. Two types rather than one, for a reason
//! that is not going away: `SearchGame` is crate-private — §P2.3 requires it,
//! because putting it in the core's façade would grow the public surface
//! §17.3's A10 pins — and a crate-private type may not appear in a public
//! signature. So everything outside this crate that searches, including
//! `tests/`, searches through this.
//!
//! It adds nothing to what the fork can do and takes nothing away. What it is
//! *for* is the queue: a `Fork` is only ever as fair as the pieces it was
//! built from, and [`Fork::of`] is where that list arrives.

use crate::core::{Game, GameEvent, GameView, PieceKind, PlayState, SearchGame, TickInput};

/// A possible future, played by the same rules as the real game.
///
/// The board, the piece in play and the hold slot are the live game's; the
/// pieces to come are the caller's own (§P2.1) — the preview the player can
/// see, and beyond it hypotheses from the bag remainder
/// [`crate::pilot::knowledge`] has inferred. There is no generator inside a
/// fork and no bag behind the queue, so a search cannot read a future nobody
/// has seen, and no test or convention is holding it back from trying.
///
/// When the queue runs out the fork stops spawning and says so
/// ([`Fork::exhausted`]). A search reaches its horizon because the object it
/// runs on does.
pub struct Fork {
    inner: SearchGame,
}

impl Fork {
    /// Fork `game`, dealing exactly `queue` and then nothing (§P2.3).
    ///
    /// The queue must be fair information: the visible preview, then
    /// hypotheses. Nothing here can check that — it is a list of pieces either
    /// way — which is why the *source* of the list is the thing to keep honest.
    /// What this type does guarantee is that the fork has no other source.
    pub fn of(game: &Game, queue: &[PieceKind]) -> Self {
        Self {
            inner: game.fork(queue),
        }
    }

    /// Advance one tick (§15.1), through the game's own rules.
    ///
    /// The buffer is the caller's and is appended to, exactly as
    /// `Game::tick`'s is: a searcher reuses one across thousands of forks
    /// rather than allocating per node.
    pub fn tick(&mut self, input: &TickInput, out: &mut Vec<GameEvent>) {
        self.inner.tick(input, out);
    }

    /// What the fork is doing right now (§12.7).
    ///
    /// A fork that has run out of pieces sits in [`PlayState::Entry`] for ever:
    /// it is not over, and it has not topped out. Ask [`Fork::exhausted`] to
    /// tell the two apart.
    pub fn state(&self) -> PlayState {
        self.inner.state()
    }

    /// The board and the figures §P5's evaluation reads (§12.7).
    pub fn view(&self) -> GameView {
        self.inner.view()
    }

    /// What is left of the queue it was given.
    pub fn upcoming(&self) -> &[PieceKind] {
        self.inner.scripted()
    }

    /// Whether the queue ran out and the fork stopped spawning.
    pub fn exhausted(&self) -> bool {
        self.inner.exhausted()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Action, Game};
    use crate::pilot::knowledge::Knowledge;
    use crate::shell::config::RulesConfig;

    /// Hard-drop `count` pieces, waiting out any clear or entry delay, and
    /// report every piece the fork spawned.
    fn drop_pieces(fork: &mut Fork, count: usize) -> Vec<PieceKind> {
        let mut spawned = Vec::new();
        let mut events = Vec::new();
        for _ in 0..count {
            events.clear();
            fork.tick(&TickInput::action(Action::HardDrop), &mut events);
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
    fn a_fork_plays_the_queue_it_was_built_from() {
        let game = Game::new(RulesConfig::default(), 42);
        let queue: Vec<_> = game.view().next;
        let mut fork = Fork::of(&game, &queue);
        assert_eq!(fork.upcoming(), queue);
        // One drop per piece in the queue, and one more to ask for a piece
        // that is not there: running out is what the fork is asked, not what
        // it volunteers.
        assert_eq!(drop_pieces(&mut fork, queue.len() + 1), queue);
        assert!(fork.exhausted(), "and stops where the preview stops");
    }

    #[test]
    fn a_fair_queue_is_the_preview_and_then_a_hypothesis() {
        // §P2.4 end to end on the planner's side: what the player can see,
        // followed by one of the pieces §9.6 says must still be in the bag.
        // The horizon is one piece longer than the preview, and every
        // hypothesis is a legal continuation.
        let game = Game::new(RulesConfig::default(), 2024);
        let view = game.view();
        let first = view.current.expect("a piece is in play").kind;
        let knowledge = Knowledge::new(first);

        for hypothesis in knowledge.hypotheses(&view.next).iter() {
            let mut queue = view.next.clone();
            queue.push(hypothesis);
            let mut fork = Fork::of(&game, &queue);
            assert_eq!(drop_pieces(&mut fork, queue.len() + 1), queue);
            assert!(fork.exhausted());
        }
    }

    #[test]
    fn an_exhausted_fork_is_quiet_rather_than_broken() {
        // A search that has run out of horizon must be able to stop looking,
        // not have to cope with a game that has ended (§P2.3).
        let game = Game::new(RulesConfig::default(), 7);
        let mut fork = Fork::of(&game, &[]);
        drop_pieces(&mut fork, 1);
        assert!(fork.exhausted());
        assert_ne!(fork.state(), PlayState::ToppedOut);
        assert_eq!(fork.upcoming(), []);

        let mut events = Vec::new();
        for _ in 0..600 {
            fork.tick(&TickInput::default(), &mut events);
        }
        assert!(events.is_empty(), "ten seconds of nothing: {events:?}");
    }
}
