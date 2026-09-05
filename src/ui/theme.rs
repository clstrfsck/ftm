//! The §9.2 palette across the four colour depths of §12.3.
//!
//! The core names a piece's colour and gives its truecolor value (§9.2); how
//! that lands on a terminal is presentation, and lives here. A [`Theme`] is
//! resolved once at start-up and then answers two questions: what glyph a cell
//! is drawn with, and what style it is drawn in.
//!
//! Brightness is a **percentage**, because §12.3 and §12.4 between them ask for
//! four of them — full, the two dimmer preview slots, and the ghost — and only
//! truecolor and 256 colours can express more than one. Below those depths the
//! percentages collapse onto the `DIM` attribute, which is exactly what §12.3
//! says to do.

use std::env;
use std::io::IsTerminal;

use ratatui::style::{Color, Modifier, Style};

use crate::config::{CELL_COLUMNS, ColorDepth, DisplaySettings, display_columns};
use crate::core::{Colour, PieceKind};

/// Full brightness: the §9.2 colour as written.
pub const FULL: u8 = 100;
/// Preview slot 1 (§12.4).
pub const SLOT_NEAR: u8 = 75;
/// Preview slots 2 and beyond (§12.4).
pub const SLOT_FAR: u8 = 55;
/// The ghost piece and inactive UI (§12.3).
pub const GHOST: u8 = 45;

/// The colour depth actually in force — `auto` already resolved (§12.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Depth {
    Truecolor,
    Ansi256,
    Ansi16,
    Mono,
}

/// The three configured cell glyphs of §12.2, each exactly
/// [`CELL_COLUMNS`] display columns wide.
///
/// `&'static str` rather than `String` so that [`Theme`] stays `Copy`, which
/// every caller in `ui` relies on. The strings come from the config file, so
/// [`Glyphs::configured`] leaks them — three allocations that live as long as
/// the process, made once at start-up. The alternative is a lifetime parameter
/// on `Theme` threaded through every screen, to save three strings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Glyphs {
    filled: &'static str,
    empty: &'static str,
    ghost: &'static str,
}

impl Glyphs {
    /// The §6.3 defaults.
    pub const DEFAULT: Self = Self {
        filled: "\u{2588}\u{2588}",
        empty: "  ",
        ghost: "\u{2592}\u{2592}",
    };

    /// The configured glyphs (§12.2). **Call this once**, at start-up: it
    /// leaks whatever the player configured.
    ///
    /// The width rule is the loader's (§6.2), which has already replaced
    /// anything that is not two columns wide with the default and warned about
    /// it. The check here is the belt to that braces: a glyph that reached this
    /// far without being validated would make the playfield ragged, and there
    /// is no way to notice that from inside `ui`.
    pub fn configured(display: &DisplaySettings) -> Self {
        let intern = |text: &str, default: &'static str| -> &'static str {
            if text == default || display_columns(text) != Some(CELL_COLUMNS) {
                default
            } else {
                Box::leak(text.to_string().into_boxed_str())
            }
        };
        Self {
            filled: intern(&display.cell_filled, Self::DEFAULT.filled),
            empty: intern(&display.cell_empty, Self::DEFAULT.empty),
            ghost: intern(&display.cell_ghost, Self::DEFAULT.ghost),
        }
    }
}

/// The palette, at one colour depth.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Theme {
    depth: Depth,
    glyphs: Glyphs,
}

impl Theme {
    /// A theme with the §6.3 default glyphs.
    pub const fn new(depth: Depth) -> Self {
        Self::with_glyphs(depth, Glyphs::DEFAULT)
    }

    /// A theme at a settled depth, with the configured glyphs (§12.2).
    ///
    /// [`Theme::resolve`] is this with §12.3's environment test in front; a
    /// caller that already knows the depth — a test, or a screen drawn at a
    /// depth the environment did not choose — asks for it directly.
    pub const fn with_glyphs(depth: Depth, glyphs: Glyphs) -> Self {
        Self { depth, glyphs }
    }

    /// Resolve `color_depth` against the environment (§12.3).
    ///
    /// The `NO_COLOR` / not-a-TTY test comes **first**, not last: it overrides
    /// a positive answer from `$COLORTERM` rather than being reached only when
    /// every other test has failed, which is the only reading under which it
    /// can ever apply.
    pub fn resolve(requested: ColorDepth, glyphs: Glyphs) -> Self {
        let depth = if !std::io::stdout().is_terminal() || env::var_os("NO_COLOR").is_some() {
            Depth::Mono
        } else {
            match requested {
                ColorDepth::Truecolor => Depth::Truecolor,
                ColorDepth::Ansi256 => Depth::Ansi256,
                ColorDepth::Ansi16 => Depth::Ansi16,
                ColorDepth::Mono => Depth::Mono,
                ColorDepth::Auto => detect(),
            }
        };
        Self::with_glyphs(depth, glyphs)
    }

    pub const fn depth(self) -> Depth {
        self.depth
    }

    /// A piece's colour at `percent` of full brightness.
    ///
    /// The base is §12.3's levelled palette, not §9.2's table as written: see
    /// [`levelled`], which is where the two differ and why.
    pub fn piece(self, kind: PieceKind, percent: u8) -> Style {
        let colour = kind.colour();
        let base = levelled(colour);
        let dim = percent < FULL;
        match self.depth {
            Depth::Truecolor => {
                let (r, g, b) = scale(base, percent);
                Style::new().fg(Color::Rgb(r, g, b))
            }
            // The §9.2 table is authoritative wherever §12.3 leaves it alone --
            // its orange is a deliberate choice, not the nearest cube entry --
            // so a lifted colour is the only one at full brightness that is
            // computed, and the dimmed steps always are.
            Depth::Ansi256 if !dim && base == colour.rgb() => {
                Style::new().fg(Color::Indexed(ansi256(colour)))
            }
            Depth::Ansi256 => {
                let (r, g, b) = scale(base, percent);
                Style::new().fg(Color::Indexed(cube(r, g, b)))
            }
            Depth::Ansi16 => attribute(Style::new().fg(ansi16(colour)), dim),
            Depth::Mono => attribute(Style::new(), dim),
        }
    }

    /// White: the line-clear and lock flashes of §12.5.
    pub fn flash(self) -> Style {
        match self.depth {
            Depth::Mono => Style::new().add_modifier(Modifier::REVERSED),
            _ => Style::new().fg(Color::White).add_modifier(Modifier::BOLD),
        }
    }

    /// The grey of the game-over wipe (§12.5).
    pub fn greyed(self) -> Style {
        match self.depth {
            Depth::Truecolor => Style::new().fg(Color::Rgb(0x60, 0x60, 0x60)),
            Depth::Ansi256 => Style::new().fg(Color::Indexed(240)),
            _ => Style::new().fg(Color::DarkGray).add_modifier(Modifier::DIM),
        }
    }

    /// Ordinary text: labels, numbers, borders.
    pub fn plain(self) -> Style {
        Style::new()
    }

    /// Text that is present but not in force: a dimmed label, an unselected
    /// menu item, the grid dots of §12.2.
    pub fn faint(self) -> Style {
        match self.depth {
            Depth::Truecolor => Style::new().fg(Color::Rgb(0x6c, 0x6c, 0x6c)),
            Depth::Ansi256 => Style::new().fg(Color::Indexed(243)),
            _ => Style::new().add_modifier(Modifier::DIM),
        }
    }

    /// Emphasis. In `mono` this is all §12.3 allows, and it is enough.
    pub fn bold(self) -> Style {
        Style::new().add_modifier(Modifier::BOLD)
    }

    /// White text at `percent` of full brightness — the fade of the level-up
    /// banner (§12.5).
    pub fn faded(self, percent: u8) -> Style {
        match self.depth {
            Depth::Truecolor => {
                let (r, g, b) = scale((0xF0, 0xF0, 0xF0), percent);
                Style::new()
                    .fg(Color::Rgb(r, g, b))
                    .add_modifier(Modifier::BOLD)
            }
            _ if percent < FULL => self.faint().add_modifier(Modifier::BOLD),
            _ => self.bold(),
        }
    }

    /// The glyph for one occupied cell (§12.2, §12.3): two display columns,
    /// always.
    ///
    /// `mono` overrides the configured glyph, because there §12.3 asks for the
    /// piece's own letter and a single block character would leave the seven
    /// pieces indistinguishable — which is the one thing colour was doing.
    pub fn filled_glyph(self, kind: PieceKind) -> &'static str {
        match self.depth {
            Depth::Mono => match kind {
                PieceKind::I => "II",
                PieceKind::O => "OO",
                PieceKind::T => "TT",
                PieceKind::S => "SS",
                PieceKind::Z => "ZZ",
                PieceKind::J => "JJ",
                PieceKind::L => "LL",
            },
            _ => self.glyphs.filled,
        }
    }

    /// The glyph for one ghost cell (§12.2, §12.3).
    pub fn ghost_glyph(self) -> &'static str {
        match self.depth {
            Depth::Mono => "..",
            _ => self.glyphs.ghost,
        }
    }

    /// The glyph for one empty cell — the dotted grid of §12.2 when
    /// `show_grid` is on, which overrides `cell_empty` for the same reason
    /// `mono` overrides `cell_filled`: it is what the setting is for.
    pub fn empty_glyph(self, grid: bool) -> &'static str {
        if grid {
            "\u{b7}\u{b7}"
        } else {
            self.glyphs.empty
        }
    }
}

/// §12.3 steps 1-3, once the `NO_COLOR` test of step 4 has been made.
fn detect() -> Depth {
    let colorterm = env::var("COLORTERM").unwrap_or_default();
    if colorterm == "truecolor" || colorterm == "24bit" {
        return Depth::Truecolor;
    }
    if env::var("TERM").unwrap_or_default().contains("256color") {
        return Depth::Ansi256;
    }
    Depth::Ansi16
}

/// `DIM` is the only dimming a 16-colour or monochrome terminal has (§12.3).
fn attribute(style: Style, dim: bool) -> Style {
    if dim {
        style.add_modifier(Modifier::DIM)
    } else {
        style
    }
}

/// An RGB triple at `percent` of its intensity (§12.3).
fn scale((r, g, b): (u8, u8, u8), percent: u8) -> (u8, u8, u8) {
    // Integer arithmetic, so that a screenshot of the same frame is the same
    // bytes on every platform.
    let dim = |c: u8| ((u16::from(c) * u16::from(percent)) / 100) as u8;
    (dim(r), dim(g), dim(b))
}

/// §12.3's levelled palette: §9.2's colour, lifted if it is too dark to draw.
///
/// Rec.709 luma of §9.2's seven runs from blue's 17 to yellow's 223, so three
/// of them — purple, red and blue — are far dimmer than the rest. On a dark
/// terminal that makes a `J` piece hard to pick out at all, and it makes the
/// wordmark of §13.2, whose letters sit side by side, read as two different
/// weights. Those three are lifted; the other four are §9.2 exactly.
///
/// A hue is lifted by blending it toward white, which is the only way up: a
/// saturated blue or purple *cannot* be as bright as cyan on any display, so
/// the brightness is bought with saturation. How much of that is worth
/// spending differs by hue, so the three do **not** land on one number:
///
/// * Purple already carries two primaries, so it reaches orange's 165 — the
///   dimmest of the four that were already bright — and is still purple.
/// * Red and blue carry one primary each and gray out much faster: at 165 they
///   read as salmon and lavender rather than as red and blue. They are lifted
///   45% of that far instead, to 102 and 84, keeping about three-quarters of
///   their saturation. A saturated hue also *looks* brighter than its luma
///   says (Helmholtz–Kohlrausch), and most so for blue, which closes much of
///   the gap the number still shows.
///
/// This is presentation and lives here: §9.2 stays the guideline table the
/// core names a piece by, and a §19 client is free to draw it its own way.
/// Hardcoded rather than computed, both because the blend is the one place a
/// float would otherwise appear and because these are a designer's numbers
/// now — see §12.3, which records the derivation.
const fn levelled(colour: Colour) -> (u8, u8, u8) {
    match colour {
        Colour::Purple => (0xD5, 0x8F, 0xF8),
        Colour::Red => (0xF4, 0x40, 0x40),
        Colour::Blue => (0x48, 0x48, 0xF4),
        other => other.rgb(),
    }
}

/// The §9.2 256-colour entry.
const fn ansi256(colour: Colour) -> u8 {
    match colour {
        Colour::Cyan => 51,
        Colour::Yellow => 226,
        Colour::Purple => 129,
        Colour::Green => 46,
        Colour::Red => 196,
        Colour::Blue => 21,
        Colour::Orange => 208,
    }
}

/// The §9.2 16-colour entry.
const fn ansi16(colour: Colour) -> Color {
    match colour {
        Colour::Cyan => Color::LightCyan,
        Colour::Yellow => Color::LightYellow,
        Colour::Purple => Color::Magenta,
        Colour::Green => Color::LightGreen,
        Colour::Red => Color::LightRed,
        Colour::Blue => Color::LightBlue,
        Colour::Orange => Color::Yellow,
    }
}

/// The xterm 6 x 6 x 6 colour cube entry nearest an RGB triple.
fn cube(r: u8, g: u8, b: u8) -> u8 {
    const LEVELS: [u8; 6] = [0, 95, 135, 175, 215, 255];
    let nearest = |c: u8| {
        LEVELS
            .iter()
            .enumerate()
            .min_by_key(|(_, level)| c.abs_diff(**level))
            .map_or(0, |(index, _)| index as u8)
    };
    16 + 36 * nearest(r) + 6 * nearest(g) + nearest(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One row of the §9.2 colour table, and what §12.3 draws it as.
    struct Entry {
        kind: PieceKind,
        /// §9.2's colour, as the core names it.
        rgb: (u8, u8, u8),
        /// §12.3's, which is §9.2's unless the hue was too dark to draw.
        drawn: (u8, u8, u8),
        /// The 256-colour entry at full brightness: §9.2's own where §12.3
        /// leaves the colour alone, and the cube cell nearest `drawn` where
        /// it does not.
        indexed: u8,
        named: Color,
    }

    const fn entry(
        kind: PieceKind,
        rgb: (u8, u8, u8),
        drawn: (u8, u8, u8),
        indexed: u8,
        named: Color,
    ) -> Entry {
        Entry {
            kind,
            rgb,
            drawn,
            indexed,
            named,
        }
    }

    /// The §9.2 and §12.3 tables, transcribed literally rather than recomputed
    /// by the same code that produces them.
    const PALETTE: [Entry; 7] = [
        entry(
            PieceKind::I,
            (0x00, 0xF0, 0xF0),
            (0x00, 0xF0, 0xF0),
            51,
            Color::LightCyan,
        ),
        entry(
            PieceKind::O,
            (0xF0, 0xF0, 0x00),
            (0xF0, 0xF0, 0x00),
            226,
            Color::LightYellow,
        ),
        entry(
            PieceKind::T,
            (0xA0, 0x00, 0xF0),
            (0xD5, 0x8F, 0xF8),
            177,
            Color::Magenta,
        ),
        entry(
            PieceKind::S,
            (0x00, 0xF0, 0x00),
            (0x00, 0xF0, 0x00),
            46,
            Color::LightGreen,
        ),
        entry(
            PieceKind::Z,
            (0xF0, 0x00, 0x00),
            (0xF4, 0x40, 0x40),
            203,
            Color::LightRed,
        ),
        entry(
            PieceKind::J,
            (0x00, 0x00, 0xF0),
            (0x48, 0x48, 0xF4),
            63,
            Color::LightBlue,
        ),
        entry(
            PieceKind::L,
            (0xF0, 0xA0, 0x00),
            (0xF0, 0xA0, 0x00),
            208,
            Color::Yellow,
        ),
    ];

    #[test]
    fn every_depth_reproduces_the_spec_table() {
        for Entry {
            kind,
            rgb,
            drawn,
            indexed,
            named,
        } in PALETTE
        {
            assert_eq!(levelled(kind.colour()), drawn, "{kind:?} in §12.3");
            let (r, g, b) = drawn;
            assert_eq!(
                Theme::new(Depth::Truecolor).piece(kind, FULL).fg,
                Some(Color::Rgb(r, g, b)),
                "{kind:?} in truecolor",
            );
            assert_eq!(
                Theme::new(Depth::Ansi256).piece(kind, FULL).fg,
                Some(Color::Indexed(indexed)),
                "{kind:?} at 256 colours",
            );
            assert_eq!(
                Theme::new(Depth::Ansi16).piece(kind, FULL).fg,
                Some(named),
                "{kind:?} at 16 colours",
            );
            // §9.2 is still what the core names the piece, whatever §12.3 goes
            // on to draw: the lift is presentation and stops at this module.
            assert_eq!(kind.colour().rgb(), rgb, "{kind:?} in §9.2");
        }
    }

    /// Rec.709 luma, the measure §12.3's palette is levelled by.
    fn luma((r, g, b): (u8, u8, u8)) -> u32 {
        (2126 * u32::from(r) + 7152 * u32::from(g) + 722 * u32::from(b)) / 10000
    }

    /// HSV saturation as a percentage: how much hue a lift has left behind.
    fn saturation((r, g, b): (u8, u8, u8)) -> u32 {
        let (top, bottom) = (r.max(g).max(b), r.min(g).min(b));
        if top == 0 {
            return 0;
        }
        100 * u32::from(top - bottom) / u32::from(top)
    }

    #[test]
    fn the_palette_lifts_the_three_dark_hues() {
        // §12.3: §9.2's own spread runs from blue's 17 to yellow's 223, which
        // is what neither the field nor §13.2's wordmark can use as it stands.
        assert_eq!(luma(Colour::Blue.rgb()), 17);
        assert_eq!(luma(Colour::Yellow.rgb()), 222);
        // Purple goes all the way to orange's 165, the dimmest of the four
        // that were already bright, and is still purple at the end of it.
        let floor = luma(Colour::Orange.rgb());
        assert_eq!(floor, 165);
        assert_eq!(luma(levelled(Colour::Purple)), floor);
        // Red and blue gray out far faster, so they stop short of it on
        // purpose and keep about three-quarters of their saturation.
        for colour in [Colour::Red, Colour::Blue] {
            let lifted = levelled(colour);
            assert!(
                luma(lifted) > luma(colour.rgb()) * 3 / 2,
                "{colour:?} is barely lifted at all",
            );
            assert!(
                luma(lifted) < floor,
                "{colour:?} at {} is back to a pastel",
                luma(lifted),
            );
            assert!(
                saturation(lifted) >= 70,
                "{colour:?} keeps only {}% of its hue",
                saturation(lifted),
            );
        }
        // And the other four are §9.2 as written.
        for colour in [Colour::Cyan, Colour::Green, Colour::Orange, Colour::Yellow] {
            assert_eq!(levelled(colour), colour.rgb(), "{colour:?} is §9.2's");
        }
    }

    #[test]
    fn a_lifted_colour_dims_from_where_it_was_lifted_to() {
        // The lift is the base the §12.3 scale runs from, not a step laid over
        // the top of it: a ghost `J` is a dimmed §12.3 blue, not a dimmed §9.2
        // one, or a piece and its ghost would be two different hues.
        let (r, g, b) = scale(levelled(Colour::Blue), GHOST);
        assert_eq!(
            Theme::new(Depth::Truecolor).piece(PieceKind::J, GHOST).fg,
            Some(Color::Rgb(r, g, b)),
        );
        assert_ne!(r, 0, "a §9.2 blue would have had no red left in it");
    }

    #[test]
    fn the_orange_entry_is_the_tables_and_not_the_nearest_cube_cell() {
        // The one entry that would be silently "corrected" by computing it:
        // #F0A000 falls nearest cube cell 214, and §9.2 says 208.
        assert_eq!(cube(0xF0, 0xA0, 0x00), 214);
        assert_eq!(ansi256(Colour::Orange), 208);
    }

    #[test]
    fn dimming_follows_the_depth() {
        // §12.3: an RGB scale in truecolor, a darker palette entry at 256, the
        // DIM attribute at 16 and in mono.
        // 45% of §12.3's red -- #F44040, the lifted one -- and not of §9.2's.
        let truecolor = Theme::new(Depth::Truecolor).piece(PieceKind::Z, GHOST);
        assert_eq!(truecolor.fg, Some(Color::Rgb(0x6D, 0x1C, 0x1C)));

        let ansi256 = Theme::new(Depth::Ansi256).piece(PieceKind::Z, GHOST);
        assert_eq!(ansi256.fg, Some(Color::Indexed(cube(0x6D, 0x1C, 0x1C))));
        assert_ne!(ansi256.fg, Some(Color::Indexed(196)), "a darker entry");

        let ansi16 = Theme::new(Depth::Ansi16).piece(PieceKind::Z, GHOST);
        assert_eq!(ansi16.fg, Some(Color::LightRed), "the same hue");
        assert!(ansi16.add_modifier.contains(Modifier::DIM));

        let mono = Theme::new(Depth::Mono).piece(PieceKind::Z, GHOST);
        assert_eq!(mono.fg, None, "mono has no colour at all");
        assert!(mono.add_modifier.contains(Modifier::DIM));
    }

    #[test]
    fn the_preview_steps_are_ordered() {
        // §12.4: three steps, 100 %, 75 % and 55 %, and they must be visibly
        // different where the depth allows.
        let theme = Theme::new(Depth::Truecolor);
        let green = |percent| match theme.piece(PieceKind::S, percent).fg {
            Some(Color::Rgb(_, g, _)) => g,
            other => panic!("{other:?}"),
        };
        assert!(green(FULL) > green(SLOT_NEAR));
        assert!(green(SLOT_NEAR) > green(SLOT_FAR));
        assert!(green(SLOT_FAR) > green(GHOST));
    }

    #[test]
    fn every_glyph_is_two_display_columns() {
        // §12.2: all three glyph strings are exactly two columns wide, in every
        // depth, or the playfield stops being a rectangle.
        for depth in [Depth::Truecolor, Depth::Ansi256, Depth::Ansi16, Depth::Mono] {
            let theme = Theme::new(depth);
            for kind in PieceKind::ALL {
                assert_eq!(theme.filled_glyph(kind).chars().count(), 2, "{depth:?}");
            }
            assert_eq!(theme.ghost_glyph().chars().count(), 2, "{depth:?}");
            assert_eq!(theme.empty_glyph(false).chars().count(), 2, "{depth:?}");
            assert_eq!(theme.empty_glyph(true).chars().count(), 2, "{depth:?}");
        }
    }

    #[test]
    fn mono_draws_the_piece_letters() {
        // §12.3: the mono glyph of §9.2, doubled.
        let theme = Theme::new(Depth::Mono);
        for kind in PieceKind::ALL {
            let doubled: String = std::iter::repeat_n(kind.glyph(), 2).collect();
            assert_eq!(theme.filled_glyph(kind), doubled);
        }
        assert_eq!(theme.ghost_glyph(), "..");
    }
    #[test]
    fn the_configured_glyphs_replace_the_defaults() {
        // §12.2: `cell_filled`, `cell_empty` and `cell_ghost` are the player's,
        // and every depth but `mono` honours them.
        let theme = Theme::resolve(
            ColorDepth::Truecolor,
            Glyphs::configured(&DisplaySettings {
                cell_filled: "[]".to_string(),
                cell_empty: "..".to_string(),
                cell_ghost: "()".to_string(),
                ..DisplaySettings::default()
            }),
        );
        // Under a test harness stdout is not a terminal, so §12.3 step 1 makes
        // the depth `mono` whatever was asked for -- which is the environment
        // rule working, not a failure. The glyphs travel regardless.
        let theme = Theme::with_glyphs(Depth::Truecolor, theme.glyphs);
        assert_eq!(theme.filled_glyph(PieceKind::T), "[]");
        assert_eq!(theme.empty_glyph(false), "..");
        assert_eq!(theme.ghost_glyph(), "()");
        assert_eq!(
            theme.empty_glyph(true),
            "\u{b7}\u{b7}",
            "show_grid overrides cell_empty, which is what the setting is for",
        );
    }

    #[test]
    fn mono_keeps_its_letters_whatever_the_glyphs_say() {
        // §12.3: in `mono` the piece letter is all that distinguishes the seven
        // pieces, so a configured block character must not replace it.
        let glyphs = Glyphs::configured(&DisplaySettings {
            cell_filled: "[]".to_string(),
            cell_ghost: "()".to_string(),
            ..DisplaySettings::default()
        });
        let theme = Theme::with_glyphs(Depth::Mono, glyphs);
        assert_eq!(theme.filled_glyph(PieceKind::S), "SS");
        assert_eq!(theme.ghost_glyph(), "..");
    }

    #[test]
    fn a_glyph_the_loader_would_have_rejected_falls_back() {
        // Belt to the loader's braces (§6.2, §12.2): `ui` assumes two columns
        // everywhere and has no way to notice a ragged field from inside.
        let glyphs = Glyphs::configured(&DisplaySettings {
            cell_filled: "###".to_string(),
            ..DisplaySettings::default()
        });
        assert_eq!(glyphs.filled, Glyphs::DEFAULT.filled);
    }
}
