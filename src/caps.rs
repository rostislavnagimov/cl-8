//! Uppercase run segmentation and CAPS-toggle logic — Section 9.1 of the specification.
//!
//! # Compression Economics
//!
//! | Run Length | Prefixes (`case-shift` × N) | Toggle (`CAPS_ON` ... `CAPS_OFF`) | Optimal Strategy |
//! |---|---|---|---|
//! | 1 | +1 byte | +2 bytes | Prefix |
//! | 2 | +2 bytes | +2 bytes | Equivalent (Prefix preferred) |
//! | 3 | +3 bytes | +2 bytes | Toggle |
//! | N | +N bytes | +2 bytes | Toggle |
//!
//! The threshold is defined by [`crate::consts::CAPS_TOGGLE_THRESHOLD`]. When equivalent (length 2),
//! prefixes are chosen because they leave no open state, increasing stream resilience.

/// Strategy for encoding a specific uppercase run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapsStrategy {
    /// Use `CASE_SHIFT` before each letter (for runs of length 1–2).
    Prefix,
    /// Wrap the run with `CAPS_ON` … `CAPS_OFF` (for runs of length 3 or more).
    Toggle,
}

/// A contiguous run of uppercase characters in the input text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapsRun {
    /// Character offset (not byte offset) in the input where the run starts.
    pub start: usize,
    /// Length of the run in characters.
    pub len: usize,
    /// Selected encoding strategy for this run.
    pub strategy: CapsStrategy,
}

/// Decoder state for tracking active uppercase mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CapsState {
    active: bool,
}

impl CapsState {
    /// Creates a default disabled caps state.
    #[must_use]
    pub const fn new() -> Self {
        Self { active: false }
    }

    /// Handles `CAPS_ON` control byte.
    pub fn turn_on(&mut self) {
        self.active = true;
    }

    /// Handles `CAPS_OFF` control byte.
    pub fn turn_off(&mut self) {
        self.active = false;
    }

    /// Returns `true` if uppercase mode is currently active.
    #[must_use]
    pub const fn applies(&self) -> bool {
        self.active
    }
}

/// Selects the optimal encoding strategy for an uppercase run of known length.
#[must_use]
#[inline]
pub const fn strategy_for(run_len: usize) -> CapsStrategy {
    if run_len >= crate::consts::CAPS_TOGGLE_THRESHOLD {
        CapsStrategy::Toggle
    } else {
        CapsStrategy::Prefix
    }
}

/// Identifies all uppercase runs in `text` and determines the optimal strategy for each.
///
/// Writes results into the pre-allocated slice `out` and returns the number of runs found.
///
/// # Errors
///
/// Returns `None` if the number of runs exceeds `out.len()`.
pub fn segment(text: &str, out: &mut [CapsRun]) -> Option<usize> {
    let mut out_idx = 0usize;
    let mut current_start = 0usize;
    let mut current_len = 0usize;

    for (char_idx, ch) in text.chars().enumerate() {
        if crate::tables::from_upper_to_lower(ch as u32).is_some() {
            if current_len == 0 {
                current_start = char_idx;
                current_len = 1;
            } else {
                current_len += 1;
            }
        } else if current_len > 0 {
            if out_idx >= out.len() {
                return None;
            }
            out[out_idx] = CapsRun {
                start: current_start,
                len: current_len,
                strategy: strategy_for(current_len),
            };
            out_idx += 1;
            current_len = 0;
        }
    }

    if current_len > 0 {
        if out_idx >= out.len() {
            return None;
        }
        out[out_idx] = CapsRun {
            start: current_start,
            len: current_len,
            strategy: strategy_for(current_len),
        };
        out_idx += 1;
    }

    Some(out_idx)
}
