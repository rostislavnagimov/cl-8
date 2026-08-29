//! Modifier buffering and stacking — Section 8.4 of the specification.
//!
//! # Rules
//!
//! When `CASE_SHIFT` is followed by another modifier instead of a base letter,
//! it is buffered along with the modifier chain until the first base letter.
//! Capitalization is applied to the resolved modified character.
//!
//! Example: `237, 238, 14` -> buffer `[case-shift, acute]` -> base 'e' -> acute produces
//! 'é' -> case-shift capitalizes to 'É'.
//!
//! Two consecutive diacritic modifiers before a single base letter are undefined
//! by the specification. The stack accepts at most one diacritic; a second one
//! returns [`DecodeError::StackedDiacritics`].

use crate::error::DecodeError;
pub use crate::tables::Modifier;
use crate::tables::{apply_modifier, codepoint_of, to_upper};

/// Maximum number of modifiers allowed before a single base character.
///
/// Exactly two: one `case-shift` plus one diacritic (Section 8.4).
pub const MAX_STACK: usize = 2;

/// Modifier accumulator between base characters.
///
/// Reset occurs after every resolved base character.
/// Zero-allocation, `Copy` value type stored directly in registers.
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

    /// Applies the accumulated modifiers to a base character code and resets the stack.
    ///
    /// Processing order: diacritic modifier is applied first, followed by capitalization.
    /// Special case (Section 8.4): Turkish `İ` (`DotAbove` + `CASE_SHIFT` + `'i'`) resolves to `U+0130`.
    ///
    /// # Errors
    ///
    /// Returns [`DecodeError::UndefinedCombination`] if the (modifier, base) pair is not in the table.
    pub fn resolve(&mut self, base: u8, position: usize) -> Result<u32, DecodeError> {
        let mut codepoint = codepoint_of(base).ok_or(DecodeError::UnknownByte {
            position,
            byte: base,
        })?;

        // Special case for Turkish capital I with dot: DotAbove + CASE_SHIFT + 'i' (code 18) -> U+0130
        if self.diacritic == Some(Modifier::DotAbove) && self.case_shift && base == 18 {
            return Ok(0x0130);
        }

        // 1. Apply diacritic
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

        // 2. Apply case-shift
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
