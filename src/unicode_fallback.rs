//! Unicode fallback mode.

use crate::error::DecodeError;

/// Decoder Unicode fallback state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct UnicodeMode {
    /// Stream offset of `UNI_ON` if active.
    opened_at: Option<usize>,
    /// Output buffer start offset.
    output_start: usize,
}

impl UnicodeMode {
    /// Creates disabled state.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            opened_at: None,
            output_start: 0,
        }
    }

    /// Returns `true` if mode is active.
    #[must_use]
    pub const fn is_active(&self) -> bool {
        self.opened_at.is_some()
    }

    /// Returns output buffer start offset.
    #[must_use]
    pub const fn output_start(&self) -> usize {
        self.output_start
    }

    /// Opens fallback mode.
    pub fn open(&mut self, position: usize, output_start: usize) {
        self.opened_at = Some(position);
        self.output_start = output_start;
    }

    /// Closes fallback mode.
    pub fn close(&mut self) {
        self.opened_at = None;
    }

    /// Validates mode is closed at end. Errors: [`DecodeError::UnterminatedUnicodeMode`] if active.
    pub fn finish(&self) -> Result<(), DecodeError> {
        match self.opened_at {
            None => Ok(()),
            Some(opened_at) => Err(DecodeError::UnterminatedUnicodeMode { opened_at }),
        }
    }
}

/// Validates canonical UTF-8 bytes. Errors: [`DecodeError::InvalidUtf8InUnicodeMode`].
pub fn validate_canonical(bytes: &[u8]) -> Result<(), DecodeError> {
    let mut i = 0;
    while i < bytes.len() {
        let byte = bytes[i];

        if byte == 0xC0 || byte == 0xC1 {
            return Err(DecodeError::InvalidUtf8InUnicodeMode { position: i });
        }

        if byte >= 0xF5 {
            return Err(DecodeError::InvalidUtf8InUnicodeMode { position: i });
        }

        if byte == 0xED && i + 1 < bytes.len() && (0xA0..=0xBF).contains(&bytes[i + 1]) {
            return Err(DecodeError::InvalidUtf8InUnicodeMode { position: i });
        }

        i += 1;
    }

    Ok(())
}
