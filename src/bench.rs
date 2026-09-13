//! §P8's runner: the grammar of the benchmark, and its clock.
//!
//! `ftm-pilot` is not a front-end. It draws nothing, has no screen, no keys and
//! no menus, and it is not behind `tui` or `gui` — it is behind `bench`, which
//! names `clap` and `anyhow` and nothing that renders. What it is, is
//! `src/argv.rs`'s neighbour: an argv is a desktop capability, and so is a
//! monotonic clock of the kind that measures a batch.
//!
//! **The split with [`pilot::bench`](crate::pilot::bench) is §P3.3's line,
//! drawn once more.** Everything that decides anything is over there, where
//! `make portable` compiles it for a target with no clock at all; everything
//! that measures how long the deciding took is here. The consequence is the one
//! §P8.2 cares about: the report proper is integers, reproduces byte for byte
//! and is what acceptance limits are written against, and the timing block
//! below is a separate thing that describes *this machine* and is expected to
//! differ on the next one.
//!
//! **It never reads §6.2's config file** (§P8.1). `preview_count` alone changes
//! how well the planner plays, so a baseline that depended on whoever ran it
//! would compare nothing. The defaults are the specification's, the flags are
//! the only way to change them, and the resolved rules are printed in the
//! header.

use std::time::{Duration, Instant};

use anyhow::{Result, bail};
use clap::Parser;

use crate::pilot::Settings;
use crate::pilot::bench::{Batch, Report};
use crate::shell::config::{
    GameplaySettings, LockDownRule, MAX_CATCH_UP_TICKS, RulesConfig, TICK, TimingSettings,
};

/// §P8.1's grammar.
///
/// Deliberately *not* §6.4's: this program does not play a game a person
/// watches, so it has no `--config`, no `--seed` (it takes a set), no `--color`
/// and no `--print-config`. What it shares with §6.4 is the four rules flags and
/// the spelling of `--lock-down`'s values, which is `src/argv.rs`'s.
#[derive(Debug, Parser)]
#[command(
    name = "ftm-pilot",
    version,
    about = "PILOT.md §P8: the automated player, measured headlessly",
    long_about = None,
    disable_help_subcommand = true
)]
pub struct Cli {
    /// Seeds: a count from 0, or a range "A..B" or "A..=B"
    #[arg(long, value_name = "SPEC", default_value = "8")]
    seeds: String,
    /// Pieces per game, before the run is stopped
    #[arg(long, value_name = "N", default_value_t = 1_000)]
    pieces: u32,
    /// Search plies (§P6.1)
    #[arg(long, value_name = "N")]
    depth: Option<u8>,
    /// States kept per ply (§P6.2)
    #[arg(long, value_name = "N")]
    beam: Option<u16>,
    /// Node budget per search (§P6.4)
    #[arg(long, value_name = "N")]
    nodes: Option<u32>,
    /// Pieces shown in the preview window [1-6]
    #[arg(long, value_name = "N")]
    preview: Option<u8>,
    /// Starting level [1-15]
    #[arg(long, value_name = "N")]
    start_level: Option<u32>,
    /// Enable the hold mechanic
    #[arg(long, overrides_with = "no_hold")]
    hold: bool,
    /// Disable the hold mechanic
    #[arg(long, overrides_with = "hold")]
    no_hold: bool,
    /// Enable 180-degree rotation
    #[arg(long, overrides_with = "no_rot180")]
    rot180: bool,
    /// Disable 180-degree rotation
    #[arg(long, overrides_with = "rot180")]
    no_rot180: bool,
    /// Lock-down rule
    #[arg(long, value_name = "RULE")]
    lock_down: Option<LockDownRule>,
    /// Report as JSON rather than text
    #[arg(long)]
    json: bool,
}

impl Cli {
    /// The batch this command line asks for, with the rules resolved through
    /// §6.3's clamping.
    ///
    /// Through `RulesConfig::from_settings` rather than by field, because a
    /// `--preview 9` should land on 6 the way the config file's would (§6.3):
    /// clamping is silent here, and there is nobody to warn. This is also the
    /// one thing `pilot/bench.rs` could not do for itself — §P3.4 lets the
    /// planner name `RulesConfig` and neither of the two settings structs it is
    /// built from.
    pub fn batch(&self) -> Result<Batch> {
        let defaults = GameplaySettings::default();
        let gameplay = GameplaySettings {
            preview_count: self.preview.unwrap_or(defaults.preview_count),
            hold_enabled: paired(self.hold, self.no_hold).unwrap_or(defaults.hold_enabled),
            allow_180_rotation: paired(self.rot180, self.no_rot180)
                .unwrap_or(defaults.allow_180_rotation),
            lock_down: self.lock_down.unwrap_or(defaults.lock_down),
            start_level: self.start_level.unwrap_or(defaults.start_level),
            ..defaults
        };
        let settings = Settings::default();
        Ok(Batch {
            rules: RulesConfig::from_settings(&gameplay, &TimingSettings::default()),
            settings: Settings {
                depth: self.depth.unwrap_or(settings.depth),
                beam: self.beam.unwrap_or(settings.beam),
                nodes: self.nodes.unwrap_or(settings.nodes),
            },
            seeds: seeds(&self.seeds)?,
            pieces: self.pieces,
        })
    }
}

/// §6.4's two-flag booleans, in the one place this program needs them.
///
/// `Overrides::paired` is the shell's answer to the same question and this is
/// not it: an override says "leave the file alone" with `None`, and there is no
/// file here — the absence of both flags means the specification's default.
fn paired(on: bool, off: bool) -> Option<bool> {
    match (on, off) {
        (true, false) => Some(true),
        (false, true) => Some(false),
        _ => None,
    }
}

/// §P8.1's `--seeds`: `N` for N seeds from 0, or a range either way inclusive.
///
/// Both spellings resolve to a list, because a checked-in seed set need not be
/// contiguous and the report prints what it played rather than what it was
/// asked for.
fn seeds(spec: &str) -> Result<Vec<u64>> {
    let Some((from, to)) = spec.split_once("..") else {
        let count: u64 = spec
            .parse()
            .map_err(|_| anyhow::anyhow!("--seeds wants a count or a range, not {spec:?}"))?;
        return Ok((0..count).collect());
    };
    let (to, inclusive) = match to.strip_prefix('=') {
        Some(to) => (to, true),
        None => (to, false),
    };
    let from: u64 = from
        .parse()
        .map_err(|_| anyhow::anyhow!("--seeds {spec:?}: {from:?} is not a seed"))?;
    let to: u64 = to
        .parse()
        .map_err(|_| anyhow::anyhow!("--seeds {spec:?}: {to:?} is not a seed"))?;
    let end = if inclusive { to.saturating_add(1) } else { to };
    if end < from {
        bail!("--seeds {spec:?} runs backwards");
    }
    Ok((from..end).collect())
}

/// What a batch cost on this machine — and only on this machine (§P8.2).
///
/// Kept apart from [`Report`] rather than folded into it, because the two have
/// opposite properties: the report is what a baseline is compared against and
/// must not move, and every figure here moves with the load, the profile and
/// the hardware. A report that mixed them would be one nobody could diff.
#[derive(Clone, Copy, Debug)]
pub struct Timing {
    /// Wall clock over the whole batch, the report's rendering excluded.
    pub elapsed: Duration,
    /// Pieces played, which is what the per-piece cost is over.
    pub pieces: u64,
    /// Placements generated, which at one ply is also the nodes evaluated.
    pub placements: u64,
    pub nodes: u64,
}

impl Timing {
    /// Nodes a second, or zero for a batch too short to have taken any time.
    pub fn nodes_per_second(&self) -> u64 {
        let nanos = self.elapsed.as_nanos();
        if nanos == 0 {
            return 0;
        }
        u64::try_from(u128::from(self.nodes) * 1_000_000_000 / nanos).unwrap_or(u64::MAX)
    }

    /// The mean cost of thinking about one piece, in microseconds.
    pub fn micros_per_piece(&self) -> u64 {
        if self.pieces == 0 {
            return 0;
        }
        u64::try_from(self.elapsed.as_micros() / u128::from(self.pieces)).unwrap_or(u64::MAX)
    }

    /// **The number P5 exists to produce** (`PILOT-PLAN.md`'s open decisions):
    /// how many nodes fit in one §15.1 tick's worth of frame, and how many fit
    /// in each search of a full §15.2 step 4 catch-up batch.
    ///
    /// The worst case is the one that matters. A pump that has fallen
    /// `MAX_CATCH_UP_TICKS` behind plays that many ticks before it draws, and a
    /// piece could in principle spawn on every one of them — six searches
    /// inside one frame's budget. The second figure is therefore the first
    /// divided by six, and it is the ceiling §P6.4's budget has to sit under.
    pub fn budget(&self) -> Budget {
        let rate = self.nodes_per_second();
        let frame = rate * TICK.as_nanos() as u64 / 1_000_000_000;
        Budget {
            frame,
            per_search: frame / u64::from(MAX_CATCH_UP_TICKS),
        }
    }
}

/// What §P6.4's node budget has to fit inside, measured rather than guessed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Budget {
    /// Nodes one frame at §15.1's tick rate can afford.
    pub frame: u64,
    /// ...divided between a full catch-up batch of searches.
    pub per_search: u64,
}

/// Run the batch, timing it (§P8).
pub fn measure(batch: &Batch) -> (Report, Timing) {
    let at = Instant::now();
    let report = crate::pilot::bench::run(batch);
    let elapsed = at.elapsed();
    let timing = Timing {
        elapsed,
        pieces: report.aggregate.pieces,
        placements: report.aggregate.counted.placements,
        nodes: report.aggregate.counted.nodes,
    };
    (report, timing)
}

/// The timing block, under a heading that says what it is not.
pub fn timing_text(timing: &Timing) -> String {
    let budget = timing.budget();
    let mut out = String::new();
    out.push_str("\ntiming (this machine, not part of the comparable report)\n");
    out.push_str(&format!(
        "  elapsed {:.3}s  {} pieces  {} nodes\n",
        timing.elapsed.as_secs_f64(),
        timing.pieces,
        timing.nodes,
    ));
    out.push_str(&format!(
        "  {} nodes/s  {} us/piece\n",
        timing.nodes_per_second(),
        timing.micros_per_piece(),
    ));
    out.push_str(&format!(
        "  a frame affords {} nodes; {} per search in a full {}-tick catch-up batch\n",
        budget.frame, budget.per_search, MAX_CATCH_UP_TICKS,
    ));
    out
}

/// The timing block as JSON, hung off the report's own object.
fn timing_json(timing: &Timing) -> serde_json::Value {
    let budget = timing.budget();
    serde_json::json!({
        "elapsed_micros": timing.elapsed.as_micros() as u64,
        "pieces": timing.pieces,
        "placements": timing.placements,
        "nodes": timing.nodes,
        "nodes_per_second": timing.nodes_per_second(),
        "micros_per_piece": timing.micros_per_piece(),
        "frame_node_budget": budget.frame,
        "catch_up_node_budget": budget.per_search,
    })
}

/// `ftm-pilot`'s whole program: parse, run, print.
///
/// The report goes to stdout and nothing else does, so a `--json` run pipes
/// into `jq` unaccompanied.
pub fn main() -> Result<()> {
    let cli = Cli::parse();
    let batch = cli.batch()?;
    let (report, timing) = measure(&batch);
    if cli.json {
        let mut value = report.json();
        value["timing"] = timing_json(&timing);
        println!("{}", serde_json::to_string_pretty(&value)?);
    } else {
        print!("{}", report.text());
        print!("{}", timing_text(&timing));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cli(args: &[&str]) -> Cli {
        let mut argv = vec!["ftm-pilot"];
        argv.extend_from_slice(args);
        Cli::try_parse_from(argv).expect("parses")
    }

    #[test]
    fn the_command_line_matches_the_spec_synopsis() {
        // §P8.1, flag by flag. `try_parse_from` fails on a name that is not
        // there, so this is a transcription check of the whole table.
        let all = cli(&[
            "--seeds",
            "0..4",
            "--pieces",
            "50",
            "--depth",
            "3",
            "--beam",
            "8",
            "--nodes",
            "1234",
            "--preview",
            "6",
            "--start-level",
            "5",
            "--no-hold",
            "--no-rot180",
            "--lock-down",
            "classic",
            "--json",
        ]);
        let batch = all.batch().expect("a legal batch");
        assert!(all.json);
        assert_eq!(batch.seeds, vec![0, 1, 2, 3]);
        assert_eq!(batch.pieces, 50);
        assert_eq!(batch.settings.depth, 3);
        assert_eq!(batch.settings.beam, 8);
        assert_eq!(batch.settings.nodes, 1_234);
        assert_eq!(batch.rules.preview_count, 6);
        assert_eq!(batch.rules.start_level, 5);
        assert!(!batch.rules.hold_enabled);
        assert!(!batch.rules.allow_180_rotation);
        assert_eq!(batch.rules.lock_down, LockDownRule::Classic);
    }

    #[test]
    fn the_defaults_are_the_specifications_and_not_a_config_files() {
        // §P8.1's whole point: run with no flags and you get §6.3's defaults,
        // whatever is in the player's file. Nothing in this module can even
        // name a config path.
        let batch = cli(&[]).batch().expect("a legal batch");
        assert_eq!(
            batch.rules,
            RulesConfig::from_settings(&GameplaySettings::default(), &TimingSettings::default()),
        );
        assert_eq!(batch.settings, Settings::default());
        assert_eq!(batch.seeds, (0..8).collect::<Vec<_>>());
        assert_eq!(batch.pieces, 1_000);
    }

    #[test]
    fn both_halves_of_a_paired_flag_are_reachable() {
        assert!(cli(&["--hold"]).batch().unwrap().rules.hold_enabled);
        assert!(!cli(&["--no-hold"]).batch().unwrap().rules.hold_enabled);
        assert!(cli(&["--rot180"]).batch().unwrap().rules.allow_180_rotation);
        assert!(
            !cli(&["--no-rot180"])
                .batch()
                .unwrap()
                .rules
                .allow_180_rotation
        );
        // ...and the later of the two wins, as §6.4's pairs do.
        assert!(
            cli(&["--no-hold", "--hold"])
                .batch()
                .unwrap()
                .rules
                .hold_enabled
        );
    }

    #[test]
    fn a_rule_out_of_range_is_clamped_the_way_the_file_would_be() {
        // §6.3's clamping, silently, because there is nobody to warn (§6.2's
        // loud half is the loader's). A benchmark that refused the flag would
        // be stricter than the game.
        assert_eq!(
            cli(&["--preview", "9"])
                .batch()
                .unwrap()
                .rules
                .preview_count,
            6
        );
        assert_eq!(
            cli(&["--start-level", "99"])
                .batch()
                .unwrap()
                .rules
                .start_level,
            15
        );
    }

    #[test]
    fn the_seed_spec_has_three_spellings() {
        assert_eq!(seeds("4").unwrap(), vec![0, 1, 2, 3]);
        assert_eq!(seeds("10..13").unwrap(), vec![10, 11, 12]);
        assert_eq!(seeds("10..=13").unwrap(), vec![10, 11, 12, 13]);
        assert_eq!(seeds("0").unwrap(), Vec::<u64>::new());
        assert_eq!(seeds("7..7").unwrap(), Vec::<u64>::new());
        assert!(seeds("5..1").is_err(), "a range that runs backwards");
        assert!(seeds("nine").is_err());
        assert!(seeds("1..x").is_err());
    }

    #[test]
    fn the_budget_is_nodes_a_frame_and_a_sixth_of_it_per_search() {
        // The arithmetic P5 exists to do, on numbers chosen so it can be
        // checked by hand: 60,000 nodes a second is a thousand in a 1/60 s
        // frame — 999, because §15.1's `TICK` is a whole number of nanoseconds
        // and a sixtieth of a second is not — and a full catch-up batch divides
        // that between six searches.
        let timing = Timing {
            elapsed: Duration::from_secs(1),
            pieces: 100,
            placements: 60_000,
            nodes: 60_000,
        };
        assert_eq!(timing.nodes_per_second(), 60_000);
        assert_eq!(timing.micros_per_piece(), 10_000);
        assert_eq!(
            timing.budget(),
            Budget {
                frame: 999,
                per_search: 999 / u64::from(MAX_CATCH_UP_TICKS),
            },
        );
    }

    #[test]
    fn an_instant_batch_reports_no_rate_rather_than_dividing_by_zero() {
        let timing = Timing {
            elapsed: Duration::ZERO,
            pieces: 0,
            placements: 0,
            nodes: 0,
        };
        assert_eq!(timing.nodes_per_second(), 0);
        assert_eq!(timing.micros_per_piece(), 0);
        assert_eq!(
            timing.budget(),
            Budget {
                frame: 0,
                per_search: 0
            }
        );
    }

    /// §P8.3's checked-in batch, and the one `cargo test` runs.
    ///
    /// Deliberately small: a debug build replays every plan a second time for
    /// §P3.1's divergence assertion, so this is seconds rather than minutes.
    /// What it is for is not the figures themselves but that they do not move
    /// without a reason — a weight change, a generator change or a tie-break
    /// change moves them *by design*, which is why this snapshot lives apart
    /// from §17.2's I1 and is expected to churn where I1 must not.
    fn smoke() -> Batch {
        Batch {
            rules: RulesConfig::default(),
            settings: Settings::default(),
            seeds: vec![0, 1, 42],
            pieces: 60,
        }
    }

    #[test]
    fn the_smoke_batch_matches_its_checked_in_snapshot() {
        let rendered = crate::pilot::bench::run(&smoke()).text();

        if std::env::var_os("UPDATE_SNAPSHOT").is_some() {
            std::fs::write("tests/snapshots/pilot_bench.txt", &rendered)
                .expect("the snapshot is writable");
            return;
        }

        assert_eq!(
            rendered,
            include_str!("../tests/snapshots/pilot_bench.txt"),
            "\nthe benchmark diverged from its snapshot; re-run with \
             UPDATE_SNAPSHOT=1 once you know why. Unlike §17.2's I1 this one is \
             *expected* to move when the weights, the generator or the \
             tie-breaks change (§P8.3)",
        );
    }

    #[test]
    fn the_smoke_batch_is_worth_snapshotting() {
        // A batch that topped out in ten pieces would produce a stable snapshot
        // and would say nothing about how the planner plays. This is the guard
        // §17.2's high-gravity fixture gets, for the same reason.
        let report = crate::pilot::bench::run(&smoke());
        assert_eq!(report.aggregate.topped_out, 0);
        assert!(report.aggregate.lines > 0, "it cleared something");
        assert_eq!(report.aggregate.pieces, 180);
    }

    #[test]
    fn a_timed_batch_reports_the_figures_the_untimed_one_does() {
        // `measure` must not change what is measured: the report it returns is
        // the report `pilot::bench::run` would have produced on its own.
        let batch = Batch {
            seeds: vec![5],
            pieces: 20,
            ..smoke()
        };
        let (report, timing) = measure(&batch);
        assert_eq!(report, crate::pilot::bench::run(&batch));
        assert_eq!(timing.pieces, report.aggregate.pieces);
        assert_eq!(timing.nodes, report.aggregate.counted.nodes);
        assert!(timing_text(&timing).contains("nodes/s"));
    }

    #[test]
    fn the_json_report_carries_the_timing_beside_the_figures() {
        let (report, timing) = measure(&Batch {
            seeds: vec![1],
            pieces: 15,
            ..smoke()
        });
        let mut value = report.json();
        value["timing"] = timing_json(&timing);
        assert_eq!(value["aggregate"]["pieces"], 15);
        assert!(value["timing"]["nodes_per_second"].is_number());
        assert_eq!(
            value["timing"]["catch_up_node_budget"],
            serde_json::json!(timing.budget().per_search),
        );
    }
}
