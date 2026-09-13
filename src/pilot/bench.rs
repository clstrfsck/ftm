//! §P8.2's report: what a batch of automated games came to, in integers.
//!
//! This half of the benchmark is **deterministic and clock-free**, which is why
//! it is in `pilot/` at all. A batch is a list of seeds, a piece cap and the two
//! settings blocks; running it plays each game headlessly through the same
//! [`Pilot`] the interactive mode drives, and what comes back is a [`Report`]
//! of nothing but counted quantities. Feed it the same [`Batch`] twice, on any
//! target and in any profile, and it renders byte for byte the same text — which
//! is §P9's C11, and the reason acceptance limits are node counts rather than
//! seconds (§P8.2).
//!
//! The wall clock is the other half and is **not here**, because §P3.3's rule is
//! that nothing in `src/pilot/` may name one: `make portable` compiles this file
//! for a target that has no clock at all. `src/bench.rs` is where the seconds
//! live, beside §P8.1's grammar, exactly as `src/argv.rs` holds the spelling of
//! a flag whose meaning is `shell::config`'s (§6.4).
//!
//! Two things this deliberately does not do. It does not read §6.2's config
//! file — a baseline that depended on whoever ran it would compare nothing, so
//! the caller pins the rules and they are printed in the header. And it does not
//! stop early on a good result: a run ends at the piece cap or at a top out, and
//! nothing else.

use crate::core::{Game, PlayState};
use crate::pilot::{Counted, Pilot, Settings};
use crate::shell::config::{LockDownRule, RulesConfig};

/// A safety net on the tick loop below, in ticks per piece.
///
/// A planned piece takes a handful of ticks plus §9.12's two delays, so this is
/// two orders of magnitude clear of anything the rules can produce. It is here
/// so that a planner that stopped pressing anything — a bug, not a state —
/// could never hang a benchmark: gravity alone would still lock a piece long
/// before this, and a run that hits it says so in the report rather than
/// spinning.
const TICKS_PER_PIECE: u64 = 1_200;

/// One batch of games, resolved: the caller has already clamped the rules
/// (§6.3) and chosen the seeds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Batch {
    /// The rules every game in the batch is played under, pinned by the caller
    /// and never read from a config file (§P8.1).
    pub rules: RulesConfig,
    /// How hard the planner is asked to think (§P3.4).
    pub settings: Settings,
    /// The seeds, in the order they are played and reported.
    pub seeds: Vec<u64>,
    /// The piece cap per game. A game that reaches it is stopped, not finished.
    pub pieces: u32,
}

/// What one seed came to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Run {
    pub seed: u64,
    pub score: u64,
    pub lines: u32,
    pub level: u32,
    /// Pieces locked, which is the cap unless the game ended first.
    pub pieces: u32,
    /// Game time in ticks — a count, not a duration (§15.1).
    pub ticks: u64,
    /// §9.16, and the figure the aggregate's rate is over.
    pub topped_out: bool,
    /// Set when [`TICKS_PER_PIECE`] stopped the game. Always false in practice;
    /// a report carrying it has found a bug, and says so rather than hanging.
    pub stalled: bool,
    /// §P8.2's cost, for this seed alone.
    pub counted: Counted,
}

/// The batch, folded (§P8.2).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Aggregate {
    pub games: u32,
    pub score: u64,
    pub lines: u64,
    pub pieces: u64,
    pub topped_out: u32,
    pub stalled: u32,
    pub counted: Counted,
}

impl Aggregate {
    /// Top-outs per thousand games, which is how the text report prints a rate
    /// without a float (§P5's "integers only", one layer out).
    pub fn top_out_rate_per_mille(&self) -> u64 {
        if self.games == 0 {
            return 0;
        }
        (u64::from(self.topped_out) * 1000).div_ceil(u64::from(self.games))
    }

    /// The mean of a total over the games played, rounded down. Zero games is
    /// zero rather than a panic: an empty seed set is a legal, if pointless,
    /// batch.
    fn mean(&self, total: u64) -> u64 {
        if self.games == 0 {
            0
        } else {
            total / u64::from(self.games)
        }
    }
}

/// A batch and what it came to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Report {
    pub batch: Batch,
    pub runs: Vec<Run>,
    pub aggregate: Aggregate,
}

/// Play every seed in the batch and report it (§P8.2).
///
/// The loop is §15.2's order with the screen and the clock taken out — `input`,
/// `tick`, `observe` — which is exactly what `App::advance` runs a tick of, and
/// is the reason a benchmark measures the same player the player watches.
pub fn run(batch: &Batch) -> Report {
    let runs: Vec<Run> = batch.seeds.iter().map(|&seed| play(batch, seed)).collect();
    let mut aggregate = Aggregate::default();
    for run in &runs {
        aggregate.games += 1;
        aggregate.score += run.score;
        aggregate.lines += u64::from(run.lines);
        aggregate.pieces += u64::from(run.pieces);
        aggregate.topped_out += u32::from(run.topped_out);
        aggregate.stalled += u32::from(run.stalled);
        aggregate.counted.nodes += run.counted.nodes;
        aggregate.counted.placements += run.counted.placements;
    }
    Report {
        batch: batch.clone(),
        runs,
        aggregate,
    }
}

/// One game, to the piece cap or to §9.16.
fn play(batch: &Batch, seed: u64) -> Run {
    let mut game = Game::new(batch.rules.clone(), seed);
    let mut pilot = Pilot::new(&batch.rules, batch.settings);
    let mut events = Vec::new();
    let limit = u64::from(batch.pieces) * TICKS_PER_PIECE;
    let mut ticks = 0;

    while game.view().pieces < batch.pieces && game.state() != PlayState::ToppedOut && ticks < limit
    {
        let input = pilot.input(&game);
        events.clear();
        game.tick(&input, &mut events);
        pilot.observe(&events);
        ticks += 1;
    }

    let view = game.view();
    Run {
        seed,
        score: view.score,
        lines: view.lines,
        level: view.level,
        pieces: view.pieces,
        ticks: view.ticks,
        topped_out: game.state() == PlayState::ToppedOut,
        stalled: ticks >= limit,
        counted: pilot.counted(),
    }
}

impl Report {
    /// §P8.2's report as text, and **this is the byte-comparable artefact**: it
    /// is integers all the way down, so two machines that disagree about it
    /// disagree about the game rather than about their speed (§P9's C11).
    pub fn text(&self) -> String {
        let mut out = String::new();
        out.push_str(&self.header());
        out.push('\n');
        out.push_str(&self.table());
        out.push('\n');
        out.push_str(&self.summary());
        out
    }

    /// The resolved rules and settings, printed because the batch pins them and
    /// the reader has no config file to check them against (§P8.1).
    fn header(&self) -> String {
        let rules = &self.batch.rules;
        let settings = &self.batch.settings;
        let mut out = String::new();
        out.push_str("ftm-pilot -- PILOT.md §P8\n\n");
        out.push_str(&format!(
            "rules    preview {}  hold {}  180 {}  ghost {}  lock-down {}\n",
            rules.preview_count,
            on_off(rules.hold_enabled),
            on_off(rules.allow_180_rotation),
            on_off(rules.ghost_piece),
            lock_down(rules.lock_down),
        ));
        out.push_str(&format!(
            "         start level {}  lines/level {}  lock delay {}t  soft drop x{}\n",
            rules.start_level,
            rules.lines_per_level,
            rules.lock_delay_ticks,
            rules.soft_drop_factor,
        ));
        out.push_str(&format!(
            "         das {}t  arr {}t  clear delay {}t  entry delay {}t\n",
            rules.das_ticks, rules.arr_ticks, rules.line_clear_delay_ticks, rules.entry_delay_ticks,
        ));
        out.push_str(&format!(
            "search   depth {}  beam {}  nodes {}  placements {}\n",
            settings.depth,
            settings.beam,
            settings.nodes,
            // §P4.1 or §P4.2, spelled out rather than as a boolean: the two
            // generators answer the same question and a baseline that did not
            // say which one it asked would be a baseline nobody could repeat.
            if settings.exact { "exact" } else { "simple" },
        ));
        out.push_str(&format!(
            "batch    {} seed{}  {} pieces\n",
            self.batch.seeds.len(),
            if self.batch.seeds.len() == 1 { "" } else { "s" },
            self.batch.pieces,
        ));
        out
    }

    /// One row per seed, in the order they were played.
    fn table(&self) -> String {
        let mut out = String::new();
        out.push_str(
            "         seed        score     lines    pieces   level  ended         nodes\n",
        );
        for run in &self.runs {
            out.push_str(&format!(
                "  {:>11}  {:>11}  {:>8}  {:>8}  {:>6}  {:>5}  {:>12}\n",
                run.seed,
                run.score,
                run.lines,
                run.pieces,
                run.level,
                ended(run),
                run.counted.nodes,
            ));
        }
        out
    }

    /// The aggregate §P8.2 asks for, top-out rate included.
    fn summary(&self) -> String {
        let a = &self.aggregate;
        let mut out = String::new();
        out.push_str(&format!(
            "total    {} games  {} pieces  {} lines  {} score  {} nodes\n",
            a.games, a.pieces, a.lines, a.score, a.counted.nodes,
        ));
        out.push_str(&format!(
            "mean     {} score  {} lines  {} pieces per game\n",
            a.mean(a.score),
            a.mean(a.lines),
            a.mean(a.pieces),
        ));
        out.push_str(&format!(
            "top out  {} of {} ({}/1000)\n",
            a.topped_out,
            a.games,
            a.top_out_rate_per_mille(),
        ));
        if a.stalled > 0 {
            out.push_str(&format!(
                "STALLED  {} of {} games hit the tick guard -- this is a bug\n",
                a.stalled, a.games,
            ));
        }
        out
    }

    /// The same figures as a JSON value (§P8.1's `--json`).
    ///
    /// A `Value` rather than a string, so `src/bench.rs` can hang its timing
    /// block off the same object and serialise once. The schema is built by
    /// hand rather than derived: these are §P8.2's *names*, promised to whatever
    /// consumes the report, and a `Serialize` derive would let a field rename
    /// change them silently. The keys come out sorted rather than in the order
    /// written here — `serde_json`'s maps are ordered by key unless it is built
    /// with `preserve_order`, and a JSON consumer reads by name anyway.
    pub fn json(&self) -> serde_json::Value {
        let rules = &self.batch.rules;
        let settings = &self.batch.settings;
        let a = &self.aggregate;
        serde_json::json!({
            "rules": {
                "preview_count": rules.preview_count,
                "hold_enabled": rules.hold_enabled,
                "allow_180_rotation": rules.allow_180_rotation,
                "ghost_piece": rules.ghost_piece,
                "lock_down": lock_down(rules.lock_down),
                "start_level": rules.start_level,
                "lines_per_level": rules.lines_per_level,
                "lock_delay_ticks": rules.lock_delay_ticks,
                "das_ticks": rules.das_ticks,
                "arr_ticks": rules.arr_ticks,
                "soft_drop_factor": rules.soft_drop_factor,
                "line_clear_delay_ticks": rules.line_clear_delay_ticks,
                "entry_delay_ticks": rules.entry_delay_ticks,
            },
            "search": {
                "depth": settings.depth,
                "beam": settings.beam,
                "nodes": settings.nodes,
                "placements": if settings.exact { "exact" } else { "simple" },
            },
            "batch": { "seeds": self.batch.seeds, "pieces": self.batch.pieces },
            "runs": self.runs.iter().map(|run| serde_json::json!({
                "seed": run.seed,
                "score": run.score,
                "lines": run.lines,
                "level": run.level,
                "pieces": run.pieces,
                "ticks": run.ticks,
                "topped_out": run.topped_out,
                "stalled": run.stalled,
                "nodes": run.counted.nodes,
                "placements": run.counted.placements,
            })).collect::<Vec<_>>(),
            "aggregate": {
                "games": a.games,
                "score": a.score,
                "lines": a.lines,
                "pieces": a.pieces,
                "topped_out": a.topped_out,
                "stalled": a.stalled,
                "top_out_per_mille": a.top_out_rate_per_mille(),
                "nodes": a.counted.nodes,
                "placements": a.counted.placements,
            },
        })
    }
}

/// How a run ended, in the width the table reserves for it.
fn ended(run: &Run) -> &'static str {
    if run.stalled {
        "STALL"
    } else if run.topped_out {
        "top"
    } else {
        "cap"
    }
}

fn on_off(value: bool) -> &'static str {
    if value { "on" } else { "off" }
}

/// §6.3's spelling, which is the config file's and the command line's
/// (`src/argv.rs`). Written out rather than taken from `Debug`, because a
/// report a player compares against their own file has to use their own words.
fn lock_down(rule: LockDownRule) -> &'static str {
    match rule {
        LockDownRule::Extended => "extended",
        LockDownRule::Infinite => "infinite",
        LockDownRule::Classic => "classic",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A batch small enough to run in a debug build, where §P3.1's divergence
    /// assertion replays every plan a second time.
    fn batch(seeds: Vec<u64>, pieces: u32) -> Batch {
        Batch {
            rules: RulesConfig::default(),
            settings: Settings::default(),
            seeds,
            pieces,
        }
    }

    #[test]
    fn a_batch_plays_every_seed_to_the_cap() {
        let report = run(&batch(vec![1, 2, 3], 20));
        assert_eq!(report.runs.len(), 3);
        for run in &report.runs {
            assert_eq!(run.pieces, 20, "seed {}", run.seed);
            assert!(!run.topped_out);
            assert!(!run.stalled);
            assert!(run.counted.nodes > 0, "a planned piece costs nodes");
            assert!(
                run.counted.nodes > run.counted.placements,
                "§P8.2: at §P6's default depth a beam evaluates interior states \
                 that are nobody's placement -- {:?}",
                run.counted,
            );
        }
        assert_eq!(report.aggregate.games, 3);
        assert_eq!(report.aggregate.pieces, 60);
        assert_eq!(report.aggregate.topped_out, 0);
        assert_eq!(report.aggregate.top_out_rate_per_mille(), 0);
    }

    #[test]
    fn one_ply_evaluates_a_placement_and_nothing_else() {
        // §P8.2's two counters, from the one side where they still agree: at a
        // single ply every node is a placement, because there is no interior to
        // expand and nothing to look up. Everything about their parting above is
        // read against this.
        let mut batch = batch(vec![1], 20);
        batch.settings.depth = 1;
        for run in &run(&batch).runs {
            assert_eq!(run.counted.nodes, run.counted.placements);
        }
    }

    #[test]
    fn the_same_batch_reports_the_same_figures_twice() {
        // §P9's C3 and C11 at the level this module can assert them: the report
        // is a pure function of the batch, so a second run of the same batch in
        // the same process cannot differ either.
        let batch = batch(vec![42, 7], 25);
        assert_eq!(run(&batch).text(), run(&batch).text());
        assert_eq!(run(&batch).json(), run(&batch).json());
    }

    #[test]
    fn the_seed_set_is_the_report_and_its_order_is_kept() {
        // The seeds are a list rather than a range, because §P8.1's two
        // spellings both resolve to one and a checked-in set need not be
        // contiguous.
        let report = run(&batch(vec![9, 1, 5], 10));
        let seeds: Vec<u64> = report.runs.iter().map(|run| run.seed).collect();
        assert_eq!(seeds, vec![9, 1, 5]);
    }

    #[test]
    fn a_report_names_the_rules_it_pinned() {
        // §P8.1: the resolved rules go in the header, because the runner never
        // reads a config file and the reader has nothing else to check them
        // against.
        let mut batch = batch(vec![0], 5);
        batch.rules.preview_count = 1;
        batch.rules.hold_enabled = false;
        batch.rules.lock_down = LockDownRule::Classic;
        let text = run(&batch).text();
        assert!(text.contains("preview 1"), "{text}");
        assert!(text.contains("hold off"), "{text}");
        assert!(text.contains("lock-down classic"), "{text}");
        assert!(text.contains("search   depth 2"), "{text}");
    }

    #[test]
    fn an_empty_batch_is_a_report_of_nothing() {
        let report = run(&batch(Vec::new(), 100));
        assert_eq!(report.aggregate, Aggregate::default());
        assert_eq!(report.aggregate.top_out_rate_per_mille(), 0);
        assert!(report.text().contains("total    0 games"));
    }

    #[test]
    fn the_json_carries_every_figure_the_text_does() {
        let report = run(&batch(vec![3], 15));
        let value = report.json();
        assert_eq!(value["aggregate"]["games"], 1);
        assert_eq!(value["aggregate"]["pieces"], 15);
        assert_eq!(value["runs"][0]["seed"], 3);
        assert_eq!(value["rules"]["preview_count"], 5);
        assert_eq!(value["rules"]["lock_down"], "extended");
        assert_eq!(value["search"]["depth"], 2);
        assert_eq!(value["batch"]["seeds"], serde_json::json!([3]));
        assert_eq!(
            value["aggregate"]["nodes"],
            serde_json::json!(report.aggregate.counted.nodes),
        );
    }

    #[test]
    fn a_top_out_rate_is_counted_in_games_and_not_in_pieces() {
        let mut aggregate = Aggregate {
            games: 8,
            topped_out: 1,
            ..Aggregate::default()
        };
        assert_eq!(aggregate.top_out_rate_per_mille(), 125);
        aggregate.topped_out = 8;
        assert_eq!(aggregate.top_out_rate_per_mille(), 1000);
        aggregate.topped_out = 0;
        assert_eq!(aggregate.top_out_rate_per_mille(), 0);
        // Rounded up, so a rate that is not zero never prints as zero: one
        // top out in a thousand games is the figure a baseline would otherwise
        // lose entirely.
        aggregate.games = 3000;
        aggregate.topped_out = 1;
        assert_eq!(aggregate.top_out_rate_per_mille(), 1);
    }
}
