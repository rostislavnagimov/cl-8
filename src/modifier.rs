//! Modifier buffering between base characters.
//!
//! Modifiers emit nothing on their own: they accumulate until a base character
//! arrives, then apply diacritic first and capitalization second.
//! `237, 238, 14` buffers `[case-shift, acute]`, takes base `e`, yields `é`, then `É`.
//!
//! Two diacritics before one base character are undefined, so the second returns
//! [`DecodeError::StackedDiacritics`].

use crate::error::DecodeError;
pub use crate::tables::Modifier;
use crate::tables::{apply_modifier, codepoint_of, to_upper};

/// Modifiers allowed before one base character: a case-shift plus a diacritic.
pub const MAX_STACK: usize = 2;

/// Modifier accumulator, reset after every resolved base character.
///
/// A `Copy` value small enough to live in registers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ModifierStack {
    case_shift: bool,
    diacritic: Option<Modifier>,
    /// Stream offset of the first modifier in the stack (for error reporting).
    origin: usize,
}

impl ModifierStack {
    /// Creates an empty modifier stack.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            case_shift: false,
            diacritic: None,
            origin: 0,
        }
    }

    /// Returns `true` if no modifiers are currently buffered.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        !self.case_shift && self.diacritic.is_none()
    }

    /// Returns the byte position of the first modifier in this stack.
    #[must_use]
    pub const fn origin(&self) -> usize {
        self.origin
    }

    /// Flags the next resolved character for capitalization.
    ///
    /// Multiple `case-shift` bytes within a single stack are idempotent (no-op).
    pub fn push_case_shift(&mut self, position: usize) {
        if self.is_empty() {
            self.origin = position;
        }
        self.case_shift = true;
    }

    /// Adds a diacritic modifier to the stack.
    ///
    /// # Errors
    ///
    /// Returns [`DecodeError::StackedDiacritics`] if a diacritic is already present.
    pub fn push_diacritic(
        &mut self,
        modifier: Modifier,
        position: usize,
    ) -> Result<(), DecodeError> {
        if self.diacritic.is_some() {
            return Err(DecodeError::StackedDiacritics { position });
        }
        if self.is_empty() {
            self.origin = position;
        }
        self.diacritic = Some(modifier);
        Ok(())
    }

    /// Applies the buffered modifiers to a base code: diacritic first, then capitalization.
    ///
    /// # Errors
    ///
    /// [`DecodeError::UndefinedCombination`] if the pair is not in the table.
    pub fn resolve(&mut self, base: u8, position: usize) -> Result<u32, DecodeError> {
        let mut codepoint = codepoint_of(base).ok_or(DecodeError::UnknownByte {
            position,
            byte: base,
        })?;

        // Turkish İ is the one composite that capitalization cannot reach from its base.
        if self.diacritic == Some(Modifier::DotAbove) && self.case_shift && base == 18 {
            return Ok(0x0130);
        }

        if let Some(diacritic) = self.diacritic {
            if let Some(modified) = apply_modifier(diacritic, base) {
                codepoint = modified;
            } else {
                return Err(DecodeError::UndefinedCombination {
                    position,
                    modifier: diacritic as u8,
                    base,
                });
            }
        }

        if self.case_shift {
            if let Some(upper) = to_upper(codepoint) {
                codepoint = upper;
            }
        }

        Ok(codepoint)
    }

    /// Clears the stack without applying it.
    pub fn clear(&mut self) {
        *self = Self::new();
    }
}
