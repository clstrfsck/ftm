//! The 7-bag randomiser (§9.6).
//!
//! A bag holds one of each tetromino, is shuffled with Fisher-Yates when empty,
//! and is drawn from the front. The next queue is kept topped up to at least
//! `preview_count + 1`, which is why a preview of more than 6 is meaningless:
//! the queue would have to see past a bag boundary it has not shuffled yet.
//!
//! The shuffle is written out rather than delegated, because the piece sequence
//! is part of the determinism contract (§15.4): the same seed must give the same
//! game, so the exact order of draws is a rule, not an implementation detail.
//! [`Bag::uniform`] is written out for the same reason, one level down.

use std::collections::VecDeque;

use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};

use crate::core::piece::PieceKind;

/// The run's generator, seeded so that a seed always means the same game
/// (§9.6).
///
/// `SmallRng::seed_from_u64` is not this function: it expands the seed
/// differently in different versions of `rand` — 0.8 used `rand_core`'s own
/// PCG32 default, and by 0.10 `Xoshiro256PlusPlus` overrides it with SplitMix64
/// — so a seed recorded under one release names a different game under the
/// next. The generator itself has not changed and is what `rand` is here for;
/// only its starting state is ours to fix, so the PCG32 expansion is written
/// out here and the seed is handed over as 32 bytes.
fn seeded(mut state: u64) -> SmallRng {
    // PCG32, four bytes at a time, filling the generator's 32-byte seed.
    let mut word = || {
        const MUL: u64 = 6_364_136_223_846_793_005;
        const INC: u64 = 11_634_580_027_462_260_723;
        // Advance first, to get away from an input of low Hamming weight.
        state = state.wrapping_mul(MUL).wrapping_add(INC);
        let xorshifted = (((state >> 18) ^ state) >> 27) as u32;
        let rotation = (state >> 59) as u32;
        xorshifted.rotate_right(rotation).to_le_bytes()
    };
    let mut seed = [0u8; 32];
    for chunk in seed.as_chunks_mut::<4>().0 {
        *chunk = word();
    }
    SmallRng::from_seed(seed)
}

/// The 7-bag randomiser and the visible next queue.
#[derive(Clone, Debug)]
pub struct Bag {
    rng: SmallRng,
    /// The current bag, drawn from the front.
    bag: VecDeque<PieceKind>,
    /// The next queue, kept at `preview_count + 1` or longer.
    queue: VecDeque<PieceKind>,
    /// How many pieces the player can see, from `RulesConfig` (§6.3).
    preview_count: u8,
    /// Whether the very first bag is still to be dealt: the S/Z courtesy of
    /// §9.6 applies to it and to no other.
    first_bag: bool,
}

impl Bag {
    /// A randomiser seeded for a run. The queue is filled immediately, so the
    /// first piece and the whole preview are decided before the first tick.
    pub fn new(seed: u64, preview_count: u8) -> Self {
        let mut bag = Self {
            rng: seeded(seed),
            bag: VecDeque::with_capacity(7),
            queue: VecDeque::with_capacity(8),
            preview_count,
            first_bag: true,
        };
        bag.top_up();
        bag
    }

    /// Take the next piece, topping the queue back up behind it.
    pub fn next_piece(&mut self) -> PieceKind {
        let piece = self.queue.pop_front().expect("the queue is never empty");
        self.top_up();
        piece
    }

    /// The upcoming pieces the player can see: exactly `preview_count` of them
    /// (§12.7).
    pub fn preview(&self) -> impl Iterator<Item = PieceKind> + '_ {
        self.queue.iter().copied().take(self.preview_count as usize)
    }

    /// What is left of the current bag, for §12.4's debug strip.
    ///
    /// This is **not** what the player is shown: the queue is drawn from the
    /// front of the bag, so anything still in here beyond `preview_count` is
    /// hidden information (§12.7).
    pub fn remaining(&self) -> impl Iterator<Item = PieceKind> + '_ {
        self.bag.iter().copied()
    }

    /// Keep the queue at `preview_count + 1`: everything on show, plus the one
    /// about to be taken.
    fn top_up(&mut self) {
        while self.queue.len() <= self.preview_count as usize {
            if self.bag.is_empty() {
                self.refill();
            }
            let piece = self.bag.pop_front().expect("just refilled");
            self.queue.push_back(piece);
        }
    }

    /// Refill and shuffle the bag (§9.6).
    fn refill(&mut self) {
        let mut pieces = PieceKind::ALL;
        // Fisher-Yates, high to low: for i from n-1 down to 1, swap i with a
        // uniform j in 0..=i.
        for i in (1..pieces.len()).rev() {
            let j = self.uniform(i);
            pieces.swap(i, j);
        }
        if self.first_bag {
            Self::apply_first_piece_courtesy(&mut pieces);
            self.first_bag = false;
        }
        self.bag.extend(pieces);
    }

    /// A uniform `0..=max`, drawn the way the shuffle must always have drawn it
    /// (§9.6).
    ///
    /// Written out rather than left to `rand`'s own range sampling, because
    /// that is **not stable across versions** — `SmallRng` is documented as
    /// non-portable, and `rand` 0.9 changed how a draw is mapped onto a range —
    /// while §9.6 requires a seed to give the same game. The generator's stream
    /// is one thing and what the shuffle makes of it is another; only the
    /// second is ours to promise, so it lives here.
    ///
    /// The method is Lemire's: multiply a full-width draw by the range, keep
    /// the high half, and reject the low half's short final bucket so every
    /// value is equally likely. `max` is at most 6 here, so the rejection loop
    /// effectively never runs a second time.
    fn uniform(&mut self, max: usize) -> usize {
        let range = max as u64 + 1;
        // `range << leading_zeros` always lands in `2^63..2^64`, so this is the
        // largest multiple of the range that fits, less one.
        let zone = (range << range.leading_zeros()).wrapping_sub(1);
        loop {
            let wide = u128::from(self.rng.next_u64()) * u128::from(range);
            let (high, low) = ((wide >> 64) as u64, wide as u64);
            if low <= zone {
                return high as usize;
            }
        }
    }

    /// §9.6: the first piece of a game is never `S` or `Z`. If it is, swap it
    /// with the first piece in the bag that is neither.
    ///
    /// This applies to the first bag of a game and to no other, and it is a swap
    /// rather than a redraw so the bag stays a permutation of the seven.
    fn apply_first_piece_courtesy(pieces: &mut [PieceKind; 7]) {
        let awkward = |kind| matches!(kind, PieceKind::S | PieceKind::Z);
        if awkward(pieces[0])
            && let Some(swap) = pieces.iter().position(|&kind| !awkward(kind))
        {
            pieces.swap(0, swap);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Take `count` pieces from a freshly seeded bag.
    fn sequence(seed: u64, count: usize) -> Vec<PieceKind> {
        let mut bag = Bag::new(seed, 5);
        (0..count).map(|_| bag.next_piece()).collect()
    }

    #[test]
    fn every_piece_appears_exactly_once_per_thousand_bags() {
        // T4. 7000 pieces is 1000 bags, so a correct randomiser deals each kind
        // exactly 1000 times -- an unshuffled bag would pass this, which is why
        // the window test below exists too.
        let pieces = sequence(42, 7000);
        for kind in PieceKind::ALL {
            let count = pieces.iter().filter(|&&p| p == kind).count();
            assert_eq!(count, 1000, "{kind:?} appeared {count} times");
        }
    }

    #[test]
    fn no_kind_appears_three_times_in_seven() {
        // T4. Two is reachable across a bag boundary; three is not, and its
        // absence is what distinguishes a bag from a random stream.
        let pieces = sequence(7, 7000);
        for window in pieces.windows(7) {
            for kind in PieceKind::ALL {
                let count = window.iter().filter(|&&p| p == kind).count();
                assert!(count <= 2, "{kind:?} appeared {count} times in {window:?}");
            }
        }
    }

    #[test]
    fn each_bag_is_a_permutation_of_the_seven() {
        let pieces = sequence(99, 700);
        for (i, bag) in pieces.chunks(7).enumerate() {
            let mut seen = bag.to_vec();
            seen.sort_by_key(|k| format!("{k:?}"));
            let mut all = PieceKind::ALL.to_vec();
            all.sort_by_key(|k| format!("{k:?}"));
            assert_eq!(seen, all, "bag {i} is not a permutation: {bag:?}");
        }
    }

    #[test]
    fn seed_42_deals_the_bag_it_has_always_dealt() {
        // §9.6: a seed names the same game across releases, which means across
        // upgrades of `rand` too -- so the sequence is written down, not just
        // asserted to be self-consistent. These are the seven `rand` 0.8 dealt
        // before its seeding and its range draw both moved under us; `seeded`
        // and `uniform` exist to keep them.
        //
        // If this fails after a dependency bump, the fix is in those two
        // functions, never in this list. The I1 snapshot fails with it.
        use PieceKind::{I, J, L, O, S, T, Z};
        assert_eq!(sequence(42, 7), vec![J, T, S, I, L, Z, O]);
    }

    #[test]
    fn a_fixed_seed_gives_a_fixed_sequence() {
        // T4, and the foundation of §15.4: two games with one seed are one game.
        assert_eq!(sequence(42, 200), sequence(42, 200));
        assert_ne!(sequence(42, 200), sequence(43, 200));
        // Independently advanced instances stay in step.
        let mut a = Bag::new(1234, 5);
        let mut b = Bag::new(1234, 5);
        for _ in 0..50 {
            assert_eq!(a.next_piece(), b.next_piece());
            assert_eq!(
                a.preview().collect::<Vec<_>>(),
                b.preview().collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn the_first_piece_of_a_game_is_never_s_or_z() {
        // T4, the guideline courtesy of §9.6. Checked across enough seeds that
        // an unshuffled first bag could not hide.
        for seed in 0..2000 {
            let first = sequence(seed, 1)[0];
            assert!(
                !matches!(first, PieceKind::S | PieceKind::Z),
                "seed {seed} opened with {first:?}",
            );
        }
    }

    #[test]
    fn the_courtesy_swaps_rather_than_redraws() {
        // The first bag stays a permutation, so the swap costs the player
        // nothing later in the bag.
        let mut swapped_seeds = 0;
        for seed in 0..500 {
            let pieces = sequence(seed, 7);
            let mut seen = pieces.clone();
            seen.sort_by_key(|k| format!("{k:?}"));
            let mut all = PieceKind::ALL.to_vec();
            all.sort_by_key(|k| format!("{k:?}"));
            assert_eq!(seen, all, "seed {seed} broke the first bag");
            if pieces[1..].iter().take(2).any(|&k| k == PieceKind::S) {
                swapped_seeds += 1;
            }
        }
        assert!(swapped_seeds > 0, "no seed exercised the swap");
    }

    #[test]
    fn the_courtesy_applies_to_the_first_bag_only() {
        // §9.6 is explicit that this is a one-off. Over many seeds, S and Z must
        // turn up at the head of later bags at roughly their natural rate; if
        // the rule leaked, they would never appear there at all.
        let mut later_bag_openings = 0;
        for seed in 0..500 {
            let pieces = sequence(seed, 21);
            if matches!(pieces[7], PieceKind::S | PieceKind::Z)
                || matches!(pieces[14], PieceKind::S | PieceKind::Z)
            {
                later_bag_openings += 1;
            }
        }
        assert!(
            later_bag_openings > 100,
            "only {later_bag_openings} of 500 seeds opened a later bag with S or Z",
        );
    }

    #[test]
    fn the_queue_shows_exactly_the_preview_count() {
        // §12.7: the view carries exactly `preview_count` entries. The queue
        // holds one more, the piece about to be taken.
        for preview_count in 1..=6u8 {
            let mut bag = Bag::new(5, preview_count);
            for _ in 0..100 {
                let shown: Vec<_> = bag.preview().collect();
                assert_eq!(shown.len(), preview_count as usize);
                let taken = bag.next_piece();
                assert_eq!(taken, *shown.first().unwrap_or(&taken));
            }
        }
    }

    #[test]
    fn the_preview_size_does_not_disturb_the_sequence() {
        // Part of the Stage 8 exit criteria, and cheaper to assert here: how far
        // ahead the player can see must not change what they are dealt.
        let baseline = sequence(2024, 100);
        for preview_count in 1..=6u8 {
            let mut bag = Bag::new(2024, preview_count);
            let pieces: Vec<_> = (0..100).map(|_| bag.next_piece()).collect();
            assert_eq!(pieces, baseline, "preview_count = {preview_count}");
        }
    }
}
