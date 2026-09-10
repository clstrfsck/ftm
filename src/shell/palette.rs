//! §9.2's colours as any front-end draws them, and the brightness steps of
//! §12.3 and §12.4.
//!
//! The core names a piece's colour and gives its truecolor value (§9.2). Two
//! things sit between that table and what a player sees, and neither of them
//! is a terminal technique:
//!
//! * **The levelled lift.** §9.2's seven are equally saturated but not equally
//!   bright, and the three dark ones have to be lifted before they can be
//!   read. The luma problem is a property of the colours, not of the display —
//!   blue at luma 17 is as hard to read on a monitor as it is in a terminal —
//!   so [`levelled`] is shared presentation and lives here.
//! * **The brightness percentages.** Full, the two dimmer preview slots and
//!   the ghost: four numbers §12.3 and §12.4 ask every front-end for.
//!
//! How a percentage lands — an RGB scale, a nearer palette entry, a `DIM`
//! attribute, an alpha — is the front-end's, and stays there.
//!
//! `Colour::rgb` is still §9.2 exactly, and is still what a §19 client is
//! handed.

use crate::core::Colour;

/// Full brightness: the §12.3 colour as written.
pub const FULL: u8 = 100;
/// Preview slot 1 (§12.4).
pub const SLOT_NEAR: u8 = 75;
/// Preview slots 2 and beyond (§12.4).
pub const SLOT_FAR: u8 = 55;
/// The ghost piece and inactive UI (§12.3).
pub const GHOST: u8 = 45;

/// §12.3's levelled palette: §9.2's colour, lifted if it is too dark to draw.
///
/// Rec.709 luma of §9.2's seven runs from blue's 17 to yellow's 223, so three
/// of them — purple, red and blue — are far dimmer than the rest. On a dark
/// background that makes a `J` piece hard to pick out at all, and it makes the
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
pub const fn levelled(colour: Colour) -> (u8, u8, u8) {
    match colour {
        Colour::Purple => (0xD5, 0x8F, 0xF8),
        Colour::Red => (0xF4, 0x40, 0x40),
        Colour::Blue => (0x48, 0x48, 0xF4),
        other => other.rgb(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
