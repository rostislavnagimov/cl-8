//! Uppercase run tracking and strategies.

use crate::encode::CharPlan;

/// Run encoding strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapsStrategy {
    /// Prefix each character with `CASE_SHIFT`.
    Prefix,
    /// Wrap run with `CAPS_ON` and `CAPS_OFF`.
    Toggle,
}

/// Uppercase run metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapsRun {
    /// Character offset in input where the run starts.
    pub start: usize,
    /// Length of the run in characters.
    pub len: usize,
    /// Encoding strategy for this run.
    pub strategy: CapsStrategy,
}

/// Uppercase mode state tracker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CapsState {
    active: bool,
}

impl CapsState {
    /// Creates disabled state.
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

    /// Returns `true` if uppercase mode is active.
    #[must_use]
    pub const fn applies(&self) -> bool {
        self.active
    }
}

/// Selects strategy for a run with `shifts` case-shifted characters.
#[must_use]
#[inline]
pub const fn strategy_for(shifts: usize) -> CapsStrategy {
    if shifts >= crate::consts::CAPS_TOGGLE_THRESHOLD {
        CapsStrategy::Toggle
    } else {
        CapsStrategy::Prefix
    }
}

/// Returns `true` if `ch` is unaffected by uppercase mode.
fn is_caps_neutral(ch: char, plan: CharPlan) -> bool {
    match plan {
        CharPlan::Control(_) => true,
        CharPlan::Base(_) => crate::tables::to_upper(ch as u32).is_none(),
        _ => false,
    }
}

/// Scans the remaining characters to determine run length and case shift count.
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

/// Identifies uppercase runs in `text`. Returns `None` if `out` is too small.
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
