//! The four capabilities of `FRONTEND.md` F1-F4, for the window on a desktop
//! (`GUI.md` §G1).
//!
//! There is almost nothing here, and that is the point. A desktop is a desktop
//! whichever front-end is standing on it, and §6.2 and §14 say `ftm` and
//! `ftm-gui` share one config file and one high-score table — so the clock,
//! the files, the seed and the date all come from [`crate::native`], which the
//! terminal front-end uses unchanged. What this module is, is the *native* arm
//! of this front-end's own split: `host_web.rs` beside it answers the same
//! names over `web_sys`, and `gui/app.rs` never learns which of the two it is
//! running on.

pub use crate::native::{Clock, Files, seed, today};

use crate::shell::config::{GuiSettings, Startup};

/// §16's warnings as they arise: natively, they wait.
///
/// The window's run has an end — `eframe::run_native` returns when it closes —
/// and §G1.2 prints them on stderr then, from `main`, where the terminal
/// front-end prints them after teardown. So there is nothing to do here; it
/// exists so that `gui/app.rs` says the same thing on both hosts, and a tab,
/// which has no end, can answer differently (§G8.6).
pub fn report(_warnings: &[String]) {}

/// `GUI.md` §G8.10: where the window is, for the config to remember.
///
/// Read every pump rather than at the end, because there is no "at the end"
/// inside the application — `eframe` hands control back to `main`, and by then
/// the window is gone and so is its geometry. Whether any of it is *written*
/// is `main`'s question and §6.2's: a run that never moved the window changes
/// nothing, and `remember_window = false` changes nothing either.
///
/// Size and position only. `fullscreen` is a setting the player chooses — in
/// the file or in the §13.5 panel — rather than a state observed off the
/// window, because `--fullscreen` is a flag and §6.1 never writes a flag back.
/// A full-screen or maximised window is skipped for the same reason in
/// reverse: the size it reports is the display's, and restoring it later would
/// leave the player a window they never chose.
///
/// **The two ends of this speak different points, and that is a real trap.**
/// `ViewportInfo`'s rects are in *`egui`* points — physical pixels over
/// `native_ppp × zoom_factor` — while `ViewportBuilder::with_inner_size` and
/// `with_position`, which is what `gui::run` hands them back to, take the
/// platform's logical points, with no zoom in them. Reading one and writing
/// the other unconverted divides the window by the scale on every run: at
/// `scale_percent = 125` a 900-point window reopens at 720, then at 576. So
/// the zoom is multiplied back out here, once, at the boundary between the two
/// units. Nothing else in this front-end has to care, because §G3's metric
/// works in `egui` points throughout (`GUI.md` §G3.1).
pub fn remember(gui: &mut GuiSettings, ctx: &egui::Context) {
    if !gui.remember_window {
        return;
    }
    // `egui` points to the platform's logical points: see above.
    let logical = ctx.zoom_factor();
    ctx.input(|input| {
        let viewport = input.viewport();
        if viewport.fullscreen.unwrap_or(false) || viewport.maximized.unwrap_or(false) {
            return;
        }
        if let Some(inner) = viewport.inner_rect {
            let (width, height) = (
                (inner.width() * logical).round(),
                (inner.height() * logical).round(),
            );
            // A window mid-resize reports zero on some platforms, and a
            // minimised one reports nothing worth keeping.
            if width >= 1.0 && height >= 1.0 {
                gui.window_width = width as u32;
                gui.window_height = height as u32;
            }
        }
        if let Some(outer) = viewport.outer_rect {
            gui.window_x = Some((outer.min.x * logical).round() as i32);
            gui.window_y = Some((outer.min.y * logical).round() as i32);
        }
    });
}

/// Carry the window's size and place onto the document that will be saved,
/// saying whether any of it changed (`GUI.md` §G8.10).
///
/// [`remember`] above put what the window reported into the *session's* config
/// as the run went on, and `Session::finish` has handed that back on
/// [`Startup::file`]. What §6.2 writes is [`Startup::on_disk`] — the file
/// without the command line applied, because a flag is for one run — so the
/// four numbers cross over here and nothing else does.
///
/// **It answers `true` only when something actually moved.** Otherwise every
/// run would rewrite the player's config for nothing, and every run over a
/// read-only one would add §16's warning to the pile.
pub fn keep_window(startup: &mut Startup) -> bool {
    // The player's own answer, taken from the file rather than from the run: a
    // panel that switched it off part-way through has switched it off.
    if !startup.on_disk.gui.remember_window {
        return false;
    }
    let seen = &startup.file.gui;
    let place = (
        seen.window_width,
        seen.window_height,
        seen.window_x,
        seen.window_y,
    );
    let kept = &mut startup.on_disk.gui;
    if (
        kept.window_width,
        kept.window_height,
        kept.window_x,
        kept.window_y,
    ) == place
    {
        return false;
    }
    (
        kept.window_width,
        kept.window_height,
        kept.window_x,
        kept.window_y,
    ) = place;
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shell::config::ConfigFile;

    fn startup(on_disk: ConfigFile, file: ConfigFile) -> Startup {
        Startup {
            file,
            on_disk,
            existed: true,
            wrote_config: false,
            seed: 1,
            seeded: false,
            warnings: Vec::new(),
        }
    }

    #[test]
    fn a_window_that_moved_is_written_back_and_one_that_did_not_is_not() {
        // `GUI.md` §G8.10, and the reason it is a decision rather than an
        // unconditional save: an unmoved window must not rewrite the file.
        let mut unmoved = startup(ConfigFile::default(), ConfigFile::default());
        assert!(!keep_window(&mut unmoved));

        let mut moved = ConfigFile::default();
        moved.gui.window_width = 1000;
        moved.gui.window_x = Some(-40);
        let mut run = startup(ConfigFile::default(), moved);
        assert!(keep_window(&mut run));
        assert_eq!(run.on_disk.gui.window_width, 1000);
        assert_eq!(run.on_disk.gui.window_x, Some(-40));
        // ...and it is idempotent: the second time nothing has moved.
        assert!(!keep_window(&mut run));
    }

    #[test]
    fn remember_window_off_keeps_the_file_exactly_as_it_was() {
        let mut off = ConfigFile::default();
        off.gui.remember_window = false;
        let mut moved = off.clone();
        moved.gui.window_width = 1000;
        let mut run = startup(off.clone(), moved);
        assert!(!keep_window(&mut run));
        assert_eq!(run.on_disk, off);
    }

    #[test]
    fn a_flag_is_never_written_back_with_the_geometry() {
        // §6.1: `--scale` and `--fullscreen` reach `Startup::file` and never
        // `on_disk`, and a window that moved must not smuggle them across.
        use crate::gui::cli::Cli;
        use crate::shell::config::Startup;
        use crate::shell::storage::Memory;
        use clap::Parser as _;
        let overrides = Cli::parse_from(["ftm-gui", "--scale", "150", "--fullscreen"]).overrides();
        let storage = Memory::new();
        let mut run = Startup::resolve(&overrides, &storage, || 1);
        // The window then moved, as `remember` would have recorded it.
        run.file.gui.window_width = 1000;
        assert!(keep_window(&mut run));
        assert_eq!(run.on_disk.gui.window_width, 1000, "the geometry crosses");
        let defaults = ConfigFile::default();
        assert_eq!(run.on_disk.gui.scale_percent, defaults.gui.scale_percent);
        assert_eq!(run.on_disk.gui.fullscreen, defaults.gui.fullscreen);
        assert_eq!(run.file.gui.scale_percent, 150, "and the run still has it");
        assert!(run.file.gui.fullscreen);
    }
}
