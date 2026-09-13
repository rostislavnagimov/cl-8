//! Unicode fallback mode: raw UTF-8 passthrough.
//!
//! `UNI_ON` (236) opens a block in which nothing is interpreted — ZWJ emoji sequences,
//! skin tones, CJK and unsupported scripts pass through byte for byte. Any byte from
//! `{0xC0, 0xC1, 0xF5..=0xFF}` closes it.
//!
//! The block costs two framing bytes, so it is cheap on solid runs of foreign text
//! and expensive on single characters scattered through table text.

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

    /// Opens the mode, remembering where it started in both streams.
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

/// Validates strict canonical UTF-8.
///
/// Stricter than [`core::str::from_utf8`]: also rejects overlong forms and unpaired
/// surrogates (CESU-8, WTF-8), which would otherwise trip the terminator early.
///
/// # Errors
///
/// [`DecodeError::InvalidUtf8InUnicodeMode`] with the offset of the bad sequence.
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
