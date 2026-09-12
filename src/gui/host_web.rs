//! The four capabilities of `FRONTEND.md` F1-F4, in a browser tab
//! (`GUI.md` §G8).
//!
//! This is the web build's whole platform, and it is the file `EGUI-PLAN.md`
//! G3 was betting on when it called the web build "a fourth capability
//! provider rather than a port": the shell above it is the native build's,
//! unchanged and uncompiled-for-differently, because since G3 it asks the
//! platform for nothing it is not handed.
//!
//! The same names as [`host_native`](super) — `Clock`, a store, `seed`,
//! `today` — so that `gui/app.rs` never learns which of the two it is running
//! on. Two more are here because a tab has no `main` to return to: [`report`],
//! which is where §16's warnings go instead of stderr, and [`query`], which is
//! where §6.4's flags come from instead of argv.

use wasm_bindgen::{JsCast as _, JsValue};

use crate::shell::storage::{Slot, Storage, StorageError};
use crate::shell::time::Stamp;

/// The `localStorage` key for §6.2's document (§G8.3).
pub const CONFIG_KEY: &str = "ftm/config.toml";
/// The `localStorage` key for §14's table (§G8.3).
pub const SCORES_KEY: &str = "ftm/highscores.json";

// ---------------------------------------------------------------------------
// F1: time
// ---------------------------------------------------------------------------

/// `performance.now()`, measured from when the clock was started (F1).
///
/// It is the browser's monotonic clock and is wall-clock-paced, which are
/// F1's two obligations; `Date.now()` is neither. The milliseconds arrive as a
/// float and become a [`Stamp`] once, here, through
/// [`Stamp::from_secs_f64`] — the one place above the core that floating
/// point is allowed (§9.9) — and the origin is subtracted first so that the
/// stamps start near zero as native ones do.
#[derive(Clone, Debug)]
pub struct Clock {
    performance: web_sys::Performance,
    origin: f64,
}

impl Clock {
    /// Start the clock.
    ///
    /// # Panics
    ///
    /// Without `window.performance`, which every browser that can run WebGL2
    /// has had for a decade. The panic is legible: `eframe::WebRunner` has
    /// installed its hook by the time this is called (§G8.5).
    pub fn new() -> Self {
        let performance = web_sys::window()
            .and_then(|window| window.performance())
            .expect("window.performance is the web build's clock (FRONTEND.md F1)");
        let origin = performance.now();
        Self {
            performance,
            origin,
        }
    }

    /// Now, as the shell measures it.
    pub fn now(&self) -> Stamp {
        Stamp::from_secs_f64((self.performance.now() - self.origin) / 1000.0)
    }
}

impl Default for Clock {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// F3, F4: entropy and the calendar
// ---------------------------------------------------------------------------

/// One seed, from two calls to `Math.random()` (F3, §9.6).
///
/// Thirty-two bits from each, which is well inside the fifty-odd that every
/// engine's generator produces per call. A seed is the program's whole entropy
/// budget and it only has to make each game a different game; it does not
/// have to be a secret, which is why `crypto.getRandomValues` — and the
/// `getrandom` backend and build-time `--cfg` that would come with it — is not
/// needed here.
pub fn seed() -> u64 {
    let half = || (js_sys::Math::random() * 4_294_967_296.0) as u64;
    (half() << 32) | half()
}

/// Today, as §14 stamps it: `YYYY-MM-DD`, in the browser's local time (F4).
pub fn today() -> String {
    let now = js_sys::Date::new_0();
    format!(
        "{:04}-{:02}-{:02}",
        now.get_full_year(),
        // JavaScript's months count from zero; its days of the month do not.
        now.get_month() + 1,
        now.get_date(),
    )
}

// ---------------------------------------------------------------------------
// F2: storage
// ---------------------------------------------------------------------------

/// §6.2's and §14's two slots, as two `localStorage` keys (F2, §G8.3).
///
/// **Per origin, and not the native files.** A player's scores in a tab and on
/// their desktop are separate tables by construction, and so are their
/// settings; §6.2 and §14 say so.
///
/// No temp file and no rename: §14 asks for a table that is either the old one
/// or the new one after a crash, and a single `setItem` already is. What is
/// left to go wrong is the quota, which is [`StorageError::Failed`] and a
/// warning (§16), and a browser that has storage switched off for the page —
/// cookies blocked, a sandboxed frame — which is
/// [`StorageError::Unavailable`], said once on the read and never again.
#[derive(Clone, Debug)]
pub struct Local {
    store: Option<web_sys::Storage>,
}

impl Local {
    /// The page's `localStorage`, if it is allowed one.
    ///
    /// Asking can *throw* — a `SecurityError` where storage is blocked — as
    /// well as answer `null`, and the two mean the same thing here.
    pub fn new() -> Self {
        Self {
            store: web_sys::window().and_then(|window| window.local_storage().ok().flatten()),
        }
    }

    fn store(&self) -> Result<&web_sys::Storage, StorageError> {
        self.store.as_ref().ok_or(StorageError::Unavailable)
    }
}

impl Default for Local {
    fn default() -> Self {
        Self::new()
    }
}

impl Storage for Local {
    fn read(&self, slot: Slot) -> Result<Option<String>, StorageError> {
        // `null` for a key never set is the ordinary first run, and arrives
        // here as `Ok(None)` without any help.
        self.store()?
            .get_item(key(slot))
            .map_err(|error| failed(slot, &error))
    }

    fn write(&mut self, slot: Slot, contents: &str) -> Result<(), StorageError> {
        self.store()?
            .set_item(key(slot), contents)
            .map_err(|error| failed(slot, &error))
    }
}

fn key(slot: Slot) -> &'static str {
    match slot {
        Slot::Config => CONFIG_KEY,
        Slot::HighScores => SCORES_KEY,
    }
}

/// §16's line: where, and then what the browser said — a `QuotaExceededError`
/// is the one a player will actually see.
fn failed(slot: Slot, error: &JsValue) -> StorageError {
    StorageError::Failed(format!(
        "localStorage \"{}\": {}",
        key(slot),
        describe(error)
    ))
}

/// A thrown JavaScript value, as words.
pub fn describe(value: &JsValue) -> String {
    if let Some(error) = value.dyn_ref::<js_sys::Error>() {
        format!(
            "{}: {}",
            String::from(error.name()),
            String::from(error.message())
        )
    } else if let Some(text) = value.as_string() {
        text
    } else {
        format!("{value:?}")
    }
}

// ---------------------------------------------------------------------------
// §16 and §6.4: what a tab has instead of stderr and argv
// ---------------------------------------------------------------------------

/// §16's warnings, on the browser console (§G8.6).
///
/// Natively they wait for the window to close and go to stderr. A tab is
/// closed rather than quit and has no "after" to print them in, so they are
/// reported as they arise, each once.
pub fn report(warnings: &[String]) {
    for warning in warnings {
        web_sys::console::warn_1(&JsValue::from_str(&format!("ftm-gui: {warning}")));
    }
}

/// The page's query string, `?` and all — what [`query::overrides`] reads
/// §6.4's flags from (§G8.4).
///
/// [`query::overrides`]: crate::gui::query::overrides
pub fn query() -> String {
    web_sys::window()
        .and_then(|window| window.location().search().ok())
        .unwrap_or_default()
}
