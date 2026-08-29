//! Unicode fallback mode — Section 10 of the specification.
//!
//! Control byte `UNI_ON` (236) switches the stream into raw UTF-8 passthrough mode.
//! Inside this mode, the parser does not interpret sub-structures: ZWJ emoji sequences,
//! skin tone modifiers, CJK, Arabic, and unsupported scripts are passed 1:1.
//! Mode termination is triggered by any byte from the set `{0xC0, 0xC1, 0xF5..=0xFF}` (Section 10.2).

use crate::error::DecodeError;

/// Decoder state for tracking active Unicode fallback mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct UnicodeMode {
    /// Stream offset of the `UNI_ON` byte if mode is currently open.
    opened_at: Option<usize>,
    /// Offset in the output buffer where the Unicode stream segment begins.
    output_start: usize,
}

impl UnicodeMode {
    /// Creates a default disabled Unicode fallback mode state.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            opened_at: None,
            output_start: 0,
        }
    }

    /// Returns `true` if Unicode fallback mode is currently open.
    #[must_use]
    pub const fn is_active(&self) -> bool {
        self.opened_at.is_some()
    }

    /// Returns the offset in the output buffer where the Unicode segment begins.
    #[must_use]
    pub const fn output_start(&self) -> usize {
        self.output_start
    }

    /// Opens Unicode fallback mode at the specified input stream position.
    pub fn open(&mut self, position: usize, output_start: usize) {
        self.opened_at = Some(position);
        self.output_start = output_start;
    }

    /// Closes Unicode fallback mode.
    pub fn close(&mut self) {
        self.opened_at = None;
    }

    /// Validates that Unicode mode is closed at the end of the input stream.
    ///
    /// # Errors
    ///
    /// Returns [`DecodeError::UnterminatedUnicodeMode`] if the stream ended while mode was active.
    pub fn finish(&self) -> Result<(), DecodeError> {
        match self.opened_at {
            None => Ok(()),
            Some(opened_at) => Err(DecodeError::UnterminatedUnicodeMode { opened_at }),
        }
    }
}

/// Validates that a byte slice contains strict, canonical UTF-8.
///
/// Stricter than [`core::str::from_utf8`]: additionally rejects overlong encodings
/// and unpaired surrogates (CESU-8, WTF-8), which could otherwise cause premature terminator matches.
///
/// # Errors
///
/// Returns [`DecodeError::InvalidUtf8InUnicodeMode`] with the offset of the invalid sequence.
pub fn validate_canonical(bytes: &[u8]) -> Result<(), DecodeError> {
    let mut i = 0;
    while i < bytes.len() {
        let byte = bytes[i];

        // Check for overlong sequences
        if byte == 0xC0 || byte == 0xC1 {
            return Err(DecodeError::InvalidUtf8InUnicodeMode { position: i });
        }

        if byte >= 0xF5 {
            // Values exceeding U+10FFFF
            return Err(DecodeError::InvalidUtf8InUnicodeMode { position: i });
        }

        // Check for CESU-8 unpaired surrogates (0xED 0xA0..=0xBF encodes U+D800..U+DFFF)
        if byte == 0xED && i + 1 < bytes.len() && (0xA0..=0xBF).contains(&bytes[i + 1]) {
            return Err(DecodeError::InvalidUtf8InUnicodeMode { position: i });
        }

        i += 1;
    }

    Ok(())
}

/// Finds the index of the first Unicode mode terminator in `bytes`.
#[must_use]
pub fn find_terminator(bytes: &[u8]) -> Option<usize> {
    let mut i = 0usize;
    while i < bytes.len() {
        if crate::consts::is_unicode_terminator(bytes[i]) {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// Returns `true` if a character requires Unicode fallback delegation.
#[must_use]
pub fn requires_fallback(c: char) -> bool {
    let cp = c as u32;
    crate::tables::code_of(cp).is_none() && !is_modifier_result(cp)
}

/// Checks whether a codepoint can be produced via a (modifier, base) pair.
#[must_use]
fn is_modifier_result(codepoint: u32) -> bool {
    let mut i = 0usize;
    while i < crate::tables::MODIFIER_TABLE.len() {
        let (_, _, result) = crate::tables::MODIFIER_TABLE[i];
        if result == codepoint {
            return true;
        }
        i += 1;
    }
    false
}
