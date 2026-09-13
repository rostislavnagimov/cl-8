//! Uppercase runs and the CAPS toggle.
//!
//! Prefixes cost one byte per letter; the `CAPS_ON`/`CAPS_OFF` pair costs two for the
//! whole run. So one letter takes a prefix, two are a tie broken toward prefixes
//! (they leave no open state), and three or more take the toggle —
//! see [`crate::consts::CAPS_TOGGLE_THRESHOLD`].
//!
//! Only letters that would actually need a `CASE_SHIFT` count. Direct single-byte
//! uppercase (`A-Z`, `А-Я`, `Ё`) saves nothing from the mode and starts no run.

use crate::encode::CharPlan;

/// How a run is encoded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapsStrategy {
    /// Use `CASE_SHIFT` before each letter (for runs of length 1–2).
    Prefix,
    /// Wrap the run with `CAPS_ON` … `CAPS_OFF` (for runs of length 3 or more).
    Toggle,
}

/// A run of uppercase characters the encoder acts on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapsRun {
    /// Character offset (not byte offset) in the input where the run starts.
    pub start: usize,
    /// Length of the run in characters.
    pub len: usize,
    /// Selected encoding strategy for this run.
    pub strategy: CapsStrategy,
}

/// Whether uppercase mode is currently on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CapsState {
    active: bool,
}

impl CapsState {
    /// Mode off.
    #[must_use]
    pub const fn new() -> Self {
        Self { active: false }
    }

    /// Handles `CAPS_ON`.
    pub fn turn_on(&mut self) {
        self.active = true;
    }

    /// Handles `CAPS_OFF`.
    pub fn turn_off(&mut self) {
        self.active = false;
    }

    /// Returns `true` while the mode is on.
    #[must_use]
    pub const fn applies(&self) -> bool {
        self.active
    }
}

/// Selects the optimal encoding strategy for a run carrying `shifts` characters
/// that would each need a `CASE_SHIFT` prefix.
#[must_use]
#[inline]
pub const fn strategy_for(shifts: usize) -> CapsStrategy {
    if shifts >= crate::consts::CAPS_TOGGLE_THRESHOLD {
        CapsStrategy::Toggle
    } else {
        CapsStrategy::Prefix
    }
}

/// Returns `true` if an active CAPS run leaves the character untouched, so it can sit
/// inside a run at no extra cost: digits, punctuation, whitespace, and uppercase letters
/// that are already direct single-byte codes.
fn is_caps_neutral(ch: char, plan: CharPlan) -> bool {
    match plan {
        // Whitespace becomes its own control byte and never consults CAPS state.
        CharPlan::Control(_) => true,
        // A direct base character is free inside a run only if CAPS would not change it.
        CharPlan::Base(_) => crate::tables::to_upper(ch as u32).is_none(),
        _ => false,
    }
}

/// Measures the CAPS run that has already started on a case-shift-requiring character,
/// given the remainder of the input after it.
///
/// Returns `(run_len, shifts)`: the run spans `run_len` characters and ends on the last
/// one needing a `CASE_SHIFT`, of which there are `shifts` in total. Neutral characters
/// extend the run only when more shift-requiring characters follow, so a run never
/// trails past its last uppercase letter.
///
/// This is the encoder's own measurement: [`Encoder`](crate::encode::Encoder) calls it
/// to decide when to open `CAPS_ON`, and [`segment`] calls it to report the same runs.
#[must_use]
pub fn scan_run(rest: &str) -> (usize, usize) {
    let mut run_len = 1usize;
    let mut shifts = 1usize;
    let mut pending = 0usize;

    for ch in rest.chars() {
        let plan = CharPlan::of(ch);
        if plan.needs_case_shift() {
            run_len += pending + 1;
            pending = 0;
            shifts += 1;
        } else if is_caps_neutral(ch, plan) {
            pending += 1;
        } else {
            break;
        }
    }

    (run_len, shifts)
}

/// Identifies the uppercase runs the encoder would actually act on, and the strategy
/// each one gets.
///
/// A run starts at a character that would cost a `CASE_SHIFT` byte on its own; direct
/// single-byte uppercase (`A-Z`, `А-Я`, `Ё`) starts nothing, because CAPS mode would
/// save it no bytes. This mirrors [`Encoder`](crate::encode::Encoder) exactly: the two
/// share [`scan_run`].
///
/// Writes results into the pre-allocated slice `out` and returns the number of runs found.
///
/// # Errors
///
/// Returns `None` if the number of runs exceeds `out.len()`.
pub fn segment(text: &str, out: &mut [CapsRun]) -> Option<usize> {
    let mut out_idx = 0usize;
    let mut skip = 0usize;

    for (char_idx, ch) in text.chars().enumerate() {
        if skip > 0 {
            skip -= 1;
            continue;
        }
        if !CharPlan::of(ch).needs_case_shift() {
            continue;
        }
        let tail_start = text
            .char_indices()
            .nth(char_idx)
            .map_or(text.len(), |(i, c)| i + c.len_utf8());
        let (len, shifts) = scan_run(&text[tail_start..]);
        if out_idx >= out.len() {
            return None;
        }
        out[out_idx] = CapsRun {
            start: char_idx,
            len,
            strategy: strategy_for(shifts),
        };
        out_idx += 1;
        skip = len - 1;
    }

    Some(out_idx)
}
