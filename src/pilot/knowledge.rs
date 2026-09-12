//! What the planner is allowed to know (`PILOT.md` §P2.1, §P2.4).
//!
//! Everything a person watching the screen could work out, and nothing else.
//! The board, the piece in play, the hold slot and the preview are on the
//! screen and arrive in a [`GameView`](crate::core::GameView). What is *not* on
//! the screen, and is still fair, is the **bag remainder**: §9.6 deals seven
//! distinct pieces per bag, so somebody who counts knows which of the seven are
//! still in the open one. Counting what has been seen is fair; reading what has
//! not is not.
//!
//! The counting is done from the event stream rather than from a changing
//! preview, because the event stream says exactly what happened:
//! `PieceSpawned(kind)` names every piece as it is dealt and `HoldUsed` is a
//! separate event (§12.8). A hold swap is **not** a deal — but a swap into an
//! empty slot is followed by one, and the order of the two events is what tells
//! them apart.

use crate::core::{GameEvent, PieceKind};

/// A set of piece kinds: §9.6's seven, as seven bits.
///
/// Integers, like everything else in the planner (§P5): a remainder is a
/// membership question asked thousands of times per search, and this is how it
/// is asked without allocating.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct PieceSet(u8);

impl PieceSet {
    /// No kinds at all.
    pub const NONE: Self = Self(0);
    /// All seven (§9.6).
    pub const ALL: Self = Self(0b111_1111);

    /// The bit a kind occupies, in `PieceKind::ALL`'s canonical order.
    fn bit(kind: PieceKind) -> u8 {
        let index = PieceKind::ALL
            .iter()
            .position(|&other| other == kind)
            .expect("PieceKind::ALL holds every kind");
        1 << index
    }

    pub fn contains(self, kind: PieceKind) -> bool {
        self.0 & Self::bit(kind) != 0
    }

    pub fn insert(&mut self, kind: PieceKind) {
        self.0 |= Self::bit(kind);
    }

    pub fn remove(&mut self, kind: PieceKind) {
        self.0 &= !Self::bit(kind);
    }

    /// The kinds this set does not hold.
    pub fn complement(self) -> Self {
        Self(!self.0 & Self::ALL.0)
    }

    pub fn len(self) -> u32 {
        self.0.count_ones()
    }

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// The kinds it holds, in `PieceKind::ALL`'s order.
    ///
    /// The order is canonical and is part of §P6.4's tie-breaking: every
    /// machine must branch over the same hypotheses in the same sequence, or
    /// two builds choose different moves from the same position.
    pub fn iter(self) -> impl Iterator<Item = PieceKind> {
        PieceKind::ALL
            .into_iter()
            .filter(move |&kind| self.contains(kind))
    }
}

/// The fair state a planner keeps between ticks (§P2.4).
///
/// It is fed the event stream and nothing else, so it cannot learn anything the
/// screen did not show. What it works out is §9.6's bag arithmetic: which
/// pieces have been dealt out of the open bag, and therefore which are left.
#[derive(Clone, Debug)]
pub struct Knowledge {
    /// Which kinds have been dealt out of the **open** bag. Emptied when the
    /// seventh closes it.
    taken: PieceSet,
    /// How many pieces have been dealt altogether (§9.6's sequence position).
    dealt: u32,
    current: Option<PieceKind>,
    held: Option<PieceKind>,
    /// Set by `HoldUsed` when the slot already held a piece: the `PieceSpawned`
    /// that follows is that piece coming back out, and is **not** a deal
    /// (§9.7, §P2.4).
    returning: bool,
}

impl Knowledge {
    /// A tracker for a game that has just begun.
    ///
    /// `first` is the piece already in play: `Game::new` spawns it during
    /// construction and **discards its event** — construction is not a tick —
    /// so it is the one deal no event stream will ever mention. Everything
    /// after it arrives through [`Knowledge::observe`].
    pub fn new(first: PieceKind) -> Self {
        let mut knowledge = Self {
            taken: PieceSet::NONE,
            dealt: 0,
            current: None,
            held: None,
            returning: false,
        };
        knowledge.deal(first);
        knowledge
    }

    /// Fold in one tick's events (§12.8).
    ///
    /// Cheap and unconditional: the common tick raises nothing, and a tick that
    /// raises a dozen animations raises at most one spawn among them.
    pub fn observe(&mut self, events: &[GameEvent]) {
        for event in events {
            match event {
                GameEvent::PieceSpawned(kind) => {
                    if self.returning {
                        // §9.7: the piece came out of the hold slot, not out of
                        // the bag. The queue is untouched and so is the bag.
                        self.returning = false;
                        self.current = Some(*kind);
                    } else {
                        self.deal(*kind);
                    }
                }
                GameEvent::HoldUsed => {
                    // A swap into an occupied slot hands the held piece back
                    // and deals nothing; a swap into an empty one is followed
                    // by a deal from the queue (§9.7).
                    self.returning = self.held.is_some();
                    self.held = self.current.take();
                }
                // A refused hold changes nothing, exactly as it resets nothing
                // (§9.7).
                _ => {}
            }
        }
    }

    /// The piece in play, as far as the events have said.
    pub fn current(&self) -> Option<PieceKind> {
        self.current
    }

    /// The piece in the hold slot (§9.7).
    pub fn held(&self) -> Option<PieceKind> {
        self.held
    }

    /// How many pieces have been dealt from §9.6's sequence.
    pub fn dealt(&self) -> u32 {
        self.dealt
    }

    /// The kinds still in the open bag — not yet **dealt**, which is not the
    /// same as not yet seen: the preview is drawn from this remainder and is
    /// still in it.
    ///
    /// [`Knowledge::hypotheses`] is what a search wants; this is what it is
    /// built from.
    pub fn remainder(&self) -> PieceSet {
        self.taken.complement()
    }

    /// The kinds that could follow `preview` — §P6.3's chance nodes.
    ///
    /// The preview is the front of the same sequence, so it comes out of the
    /// same bag: what could come after it is the remainder with the preview
    /// taken out of it, and a preview long enough to close the bag opens the
    /// next one, where anything is possible again. §6.3 allows a preview of 6
    /// and a bag holds 7, so both halves of that are reachable in an ordinary
    /// configuration.
    pub fn hypotheses(&self, preview: &[PieceKind]) -> PieceSet {
        let mut taken = self.taken;
        for &kind in preview {
            debug_assert!(
                !taken.contains(kind),
                "§9.6 deals each kind once per bag: {kind:?} twice in {preview:?}",
            );
            taken.insert(kind);
            taken = Self::close_full_bag(taken);
        }
        taken.complement()
    }

    /// Record a piece dealt out of the bag.
    fn deal(&mut self, kind: PieceKind) {
        debug_assert!(
            !self.taken.contains(kind),
            "§9.6 deals each kind once per bag: {kind:?} twice",
        );
        self.taken.insert(kind);
        self.taken = Self::close_full_bag(self.taken);
        self.dealt += 1;
        self.current = Some(kind);
    }

    /// Seven deals close a bag, and the next one is open with nothing taken
    /// from it (§9.6, §P2.4).
    ///
    /// The reset happens on the **seventh** piece rather than on the eighth.
    /// Waiting for the eighth would leave the remainder empty for one piece —
    /// reading "nothing can come next" at the one moment every one of the seven
    /// can.
    fn close_full_bag(taken: PieceSet) -> PieceSet {
        if taken == PieceSet::ALL {
            PieceSet::NONE
        } else {
            taken
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::PieceKind::{I, J, L, O, S, T, Z};
    use crate::core::{Action, Game, GameEvent, PlayState, Shift, TickInput};
    use crate::shell::config::RulesConfig;

    fn set(kinds: &[PieceKind]) -> PieceSet {
        let mut set = PieceSet::NONE;
        for &kind in kinds {
            set.insert(kind);
        }
        set
    }

    /// The defaults with §6.3's two gameplay settings changed.
    ///
    /// Built by field rather than through `RulesConfig::from_settings`, because
    /// §P3.4 lets the planner name `RulesConfig` and nothing else in the shell,
    /// and a test that reached past that would be testing a module allowed to
    /// do something this one is not. Both values below are inside §6.3's range,
    /// so there is nothing for the loader to clamp.
    fn rules(preview_count: u8, hold_enabled: bool) -> RulesConfig {
        RulesConfig {
            preview_count,
            hold_enabled,
            ..RulesConfig::default()
        }
    }

    /// Play one piece, feeding every event to the tracker as a real round would
    /// (§15.2 step 3): hold it first if asked, spread it across the board, and
    /// hard-drop it.
    ///
    /// The spreading is the core's own `play_twenty` trick and is not
    /// decoration: twenty pieces dropped where they spawn top the game out at
    /// eleven, and a tracker test that ended early would be asserting about a
    /// game that stopped rather than a bag that turned over.
    fn play(game: &mut Game, knowledge: &mut Knowledge, piece: usize, hold: bool) {
        let mut events = Vec::new();
        let mut step = |game: &mut Game, knowledge: &mut Knowledge, input: TickInput| {
            events.clear();
            game.tick(&input, &mut events);
            knowledge.observe(&events);
        };
        if hold {
            step(game, knowledge, TickInput::action(Action::Hold));
        }
        // To the left wall, then out to a column that moves on by three each
        // piece: 0, 3, 6, 1, 4, 7, ... which keeps the surface flat enough for
        // a long round. A shift of ten cells is legal (§10.3's ARR of 0) and
        // stops at the wall.
        step(
            game,
            knowledge,
            TickInput {
                shift: Some(Shift::Left),
                shift_cells: 10,
                ..TickInput::default()
            },
        );
        step(
            game,
            knowledge,
            TickInput {
                shift: Some(Shift::Right),
                shift_cells: (piece as u8 * 3) % 8,
                ..TickInput::default()
            },
        );
        step(game, knowledge, TickInput::action(Action::HardDrop));
        while game.view().current.is_none() && game.state() != PlayState::ToppedOut {
            step(game, knowledge, TickInput::default());
        }
    }

    #[test]
    fn a_set_holds_the_seven_and_counts_them() {
        assert_eq!(PieceSet::ALL.len(), 7);
        assert_eq!(PieceSet::NONE.len(), 0);
        assert!(PieceSet::NONE.is_empty());
        assert_eq!(PieceSet::ALL.complement(), PieceSet::NONE);
        assert_eq!(PieceSet::NONE.complement(), PieceSet::ALL);

        let mut some = set(&[T, I, L]);
        assert!(some.contains(T) && !some.contains(S));
        some.remove(T);
        assert_eq!(some, set(&[I, L]));
        // Canonical order (§P6.4), not insertion order.
        assert_eq!(set(&[L, I, T]).iter().collect::<Vec<_>>(), vec![I, T, L]);
        assert_eq!(PieceSet::ALL.iter().collect::<Vec<_>>(), PieceKind::ALL);
    }

    #[test]
    fn the_first_piece_is_a_deal_no_event_ever_mentions() {
        // `Game::new` spawns it during construction and throws the event away.
        // A tracker that waited for the event would be one piece out for the
        // whole game, and its remainder would name a piece already in play.
        let game = Game::new(RulesConfig::default(), 42);
        let first = game.view().current.expect("a piece is in play").kind;
        let knowledge = Knowledge::new(first);
        assert_eq!(knowledge.current(), Some(first));
        assert_eq!(knowledge.dealt(), 1);
        assert!(!knowledge.remainder().contains(first));
        assert_eq!(knowledge.remainder().len(), 6);
    }

    #[test]
    fn deals_come_out_of_the_open_bag_until_it_closes() {
        // §9.6 through §P2.4: seven distinct pieces, then a fresh bag with all
        // seven available again.
        let mut game = Game::new(RulesConfig::default(), 42);
        let first = game.view().current.expect("a piece").kind;
        let mut knowledge = Knowledge::new(first);
        let mut seen = vec![first];

        for piece in 1..7 {
            play(&mut game, &mut knowledge, piece, false);
            seen.push(knowledge.current().expect("a piece"));
            assert_eq!(knowledge.dealt(), piece as u32 + 1);
            if piece < 6 {
                assert_eq!(
                    knowledge.remainder(),
                    set(&seen).complement(),
                    "after {} deals",
                    piece + 1,
                );
            }
        }
        assert_eq!(set(&seen), PieceSet::ALL, "a bag is a permutation");
        assert_eq!(
            knowledge.remainder(),
            PieceSet::ALL,
            "the seventh closed the bag, and the next one is untouched",
        );

        play(&mut game, &mut knowledge, 7, false);
        assert_eq!(knowledge.dealt(), 8);
        assert_eq!(
            knowledge.remainder(),
            set(&[knowledge.current().unwrap()]).complement(),
            "the eighth piece is one out of a fresh bag",
        );
    }

    #[test]
    fn the_bag_is_reset_on_the_seventh_piece_and_not_the_eighth() {
        // The off-by-one that would read "nothing can come next" at the one
        // moment any of the seven can.
        let mut game = Game::new(RulesConfig::default(), 99);
        let first = game.view().current.expect("a piece").kind;
        let mut knowledge = Knowledge::new(first);
        for piece in 1..7 {
            play(&mut game, &mut knowledge, piece, false);
        }
        assert_eq!(knowledge.dealt(), 7);
        assert_eq!(knowledge.remainder(), PieceSet::ALL);
    }

    #[test]
    fn a_hold_swap_is_not_a_deal() {
        // §P2.4, and the one case a tracker that counted `PieceSpawned` alone
        // would get wrong. The first hold fills an empty slot and *is* followed
        // by a deal; every hold after it swaps, and deals nothing.
        let mut game = Game::new(RulesConfig::default(), 2024);
        let first = game.view().current.expect("a piece").kind;
        let mut knowledge = Knowledge::new(first);

        // Hold on the first piece: the slot is empty, so a piece is dealt.
        let mut events = Vec::new();
        game.tick(&TickInput::action(Action::Hold), &mut events);
        knowledge.observe(&events);
        assert_eq!(knowledge.held(), Some(first));
        assert_eq!(knowledge.dealt(), 2, "an empty slot costs a deal");
        let second = knowledge.current().expect("a piece is in play");

        // Lock it, then hold the piece after it: now the slot is occupied, the
        // held piece comes back, and nothing is dealt.
        play(&mut game, &mut knowledge, 1, false);
        let third = knowledge.current().expect("a piece is in play");
        assert_eq!(knowledge.dealt(), 3);
        events.clear();
        game.tick(&TickInput::action(Action::Hold), &mut events);
        knowledge.observe(&events);
        assert_eq!(knowledge.dealt(), 3, "a swap deals nothing");
        assert_eq!(knowledge.current(), Some(first), "the held piece is back");
        assert_eq!(knowledge.held(), Some(third));
        let _ = second;
    }

    #[test]
    fn the_tracker_agrees_with_the_game_over_a_long_round() {
        // The whole of §P2.4 against the core itself: over two hundred pieces
        // with holds scattered through them, what the tracker believes is in
        // play and in the hold slot is what the game says, and its remainder
        // never names a piece the game has already dealt.
        let mut game = Game::new(rules(5, true), 7);
        let first = game.view().current.expect("a piece").kind;
        let mut knowledge = Knowledge::new(first);
        for piece in 0..200 {
            play(
                &mut game,
                &mut knowledge,
                piece,
                piece % 3 == 0 || piece % 7 == 2,
            );
            if game.state() == PlayState::ToppedOut {
                break;
            }
            let view = game.view();
            assert_eq!(
                knowledge.current(),
                view.current.map(|piece| piece.kind),
                "the piece in play, after {piece} pieces",
            );
            assert_eq!(knowledge.held(), view.hold, "the hold slot");
            // Whatever the planner thinks could come next, the piece the game
            // is actually about to deal is in it (§P2.4). A tracker that had
            // drifted would rule out a piece that then turned up.
            let next = *view.next.first().expect("a preview of at least one");
            assert!(
                knowledge.remainder().contains(next),
                "after {piece} pieces the bag had ruled {next:?} out",
            );
        }
        // How far it gets is the dropping pattern's business, not the
        // tracker's; what matters is that it got far enough to turn the bag
        // over several times with holds scattered through it.
        assert!(knowledge.dealt() > 30, "only {} pieces", knowledge.dealt());
    }

    #[test]
    fn the_hypotheses_are_the_bag_the_preview_leaves_behind() {
        // §P6.3's chance nodes. What can follow the preview is what neither the
        // deals nor the preview have used.
        // Checked against the future the game really deals, which is the one
        // thing the planner may not look at: whatever it comes up with, the
        // piece that actually follows the preview has to be among them.
        for seed in 0..25u64 {
            let mut game = Game::new(RulesConfig::default(), seed);
            let first = game.view().current.expect("a piece").kind;
            let mut knowledge = Knowledge::new(first);
            for piece in 0..3 {
                play(&mut game, &mut knowledge, piece, false);
            }

            let preview = game.view().next;
            let hypotheses = knowledge.hypotheses(&preview);
            assert!(
                !hypotheses.is_empty(),
                "there is always something that could come next",
            );
            // Play the preview out and see what the bag really had next.
            let mut shadow = knowledge.clone();
            for piece in 0..preview.len() {
                play(&mut game, &mut shadow, piece, false);
            }
            let actual = game.view().next[0];
            assert!(
                hypotheses.contains(actual),
                "seed {seed}: the bag dealt {actual:?}, which was ruled out",
            );
        }
    }

    #[test]
    fn a_preview_that_closes_a_bag_opens_the_next_one() {
        // §6.3 allows a preview of 6 and a bag holds 7, so a preview that
        // reaches the boundary is ordinary rather than exotic. Three deals and
        // a preview of four use the bag up exactly, and anything at all may
        // follow.
        let knowledge = Knowledge {
            taken: set(&[I, O, T]),
            dealt: 3,
            current: Some(T),
            held: None,
            returning: false,
        };
        assert_eq!(knowledge.hypotheses(&[S, Z]), set(&[J, L]));
        assert_eq!(knowledge.hypotheses(&[S, Z, J]), set(&[L]));
        assert_eq!(
            knowledge.hypotheses(&[S, Z, J, L]),
            PieceSet::ALL,
            "the bag closed, so the next piece is any of the seven",
        );
        assert_eq!(
            knowledge.hypotheses(&[S, Z, J, L, I, O]),
            set(&[I, O]).complement(),
            "and the two after the boundary come out of the new bag",
        );
    }

    #[test]
    fn hold_and_preview_settings_do_not_disturb_the_count() {
        // §6.3's `hold_enabled` may be off and `preview_count` may be 1: a
        // tracker that inferred deals from the length of the preview would be
        // wrong in both configurations. This one counts events.
        for preview_count in [1u8, 6] {
            for hold_enabled in [true, false] {
                let mut game = Game::new(rules(preview_count, hold_enabled), 31);
                let first = game.view().current.expect("a piece").kind;
                let mut knowledge = Knowledge::new(first);
                for piece in 0..20 {
                    play(
                        &mut game,
                        &mut knowledge,
                        piece,
                        [0, 3, 4, 11].contains(&piece),
                    );
                }
                assert_ne!(
                    game.state(),
                    PlayState::ToppedOut,
                    "preview {preview_count}, hold {hold_enabled}: the round ended early",
                );
                let dealt = knowledge.dealt();
                assert_eq!(
                    knowledge.current(),
                    game.view().current.map(|piece| piece.kind),
                    "preview {preview_count}, hold {hold_enabled}",
                );
                // Twenty locks, plus one deal for each hold that filled an
                // empty slot — one, if hold is on at all.
                let expected = 21 + u32::from(hold_enabled);
                assert_eq!(
                    dealt, expected,
                    "preview {preview_count}, hold {hold_enabled}"
                );
            }
        }
    }

    #[test]
    fn events_the_tracker_does_not_care_about_change_nothing() {
        // §12.8: dropping every event must change nothing about the game, and
        // the mirror of that here is that every event but two must change
        // nothing about the tracker.
        let knowledge = Knowledge::new(T);
        let mut noisy = knowledge.clone();
        noisy.observe(&[
            GameEvent::PieceMoved,
            GameEvent::RotationFailed,
            GameEvent::HoldRejected,
            GameEvent::PieceRotated { kick_index: 4 },
            GameEvent::HardDropped { rows: 19 },
            GameEvent::PerfectClear,
            GameEvent::LevelUp(3),
        ]);
        assert_eq!(noisy.current(), knowledge.current());
        assert_eq!(noisy.held(), knowledge.held());
        assert_eq!(noisy.dealt(), knowledge.dealt());
        assert_eq!(noisy.remainder(), knowledge.remainder());
    }
}
