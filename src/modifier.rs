//! Modifier buffering.

use crate::error::DecodeError;
pub use crate::tables::Modifier;
use crate::tables::{apply_modifier, codepoint_of, to_upper};

/// Maximum buffered modifiers per base character.
pub const MAX_STACK: usize = 2;

/// Modifier accumulator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ModifierStack {
    case_shift: bool,
    diacritic: Option<Modifier>,
    /// Stream offset of the first modifier in the stack.
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

    /// Returns `true` if no modifiers are buffered.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        !self.case_shift && self.diacritic.is_none()
    }

    /// Returns the byte position of the first modifier in this stack.
    #[must_use]
    pub const fn origin(&self) -> usize {
        self.origin
    }

    /// Buffers a case shift.
    pub fn push_case_shift(&mut self, position: usize) {
        if self.is_empty() {
            self.origin = position;
        }
        self.case_shift = true;
    }

    /// Buffers a diacritic. Errors: [`DecodeError::StackedDiacritics`] if one is already buffered.
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

    /// Resolves buffered modifiers on a base code. Errors: [`DecodeError`] on invalid code or combination.
    pub fn resolve(&mut self, base: u8, position: usize) -> Result<u32, DecodeError> {
        let mut codepoint = codepoint_of(base).ok_or(DecodeError::UnknownByte {
            position,
            byte: base,
        })?;

        // Turkish capital İ composite exception.
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

    /// Clears the stack.
    pub fn clear(&mut self) {
        *self = Self::new();
    }
}
