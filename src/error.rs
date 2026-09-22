//! Error types.

/// Decoding error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum DecodeError {
    /// Unknown or reserved byte value.
    UnknownByte {
        /// Byte offset in input.
        position: usize,
        /// Unassigned byte value.
        byte: u8,
    },

    /// Two consecutive diacritics.
    StackedDiacritics {
        /// Byte offset of the second modifier.
        position: usize,
    },

    /// Modifier without a following base character.
    DanglingModifier {
        /// Byte offset of the modifier.
        position: usize,
    },

    /// Undefined modifier and base combination.
    UndefinedCombination {
        /// Byte offset of the base character.
        position: usize,
        /// Modifier control byte.
        modifier: u8,
        /// Base character code.
        base: u8,
    },

    /// Unterminated Unicode fallback mode.
    UnterminatedUnicodeMode {
        /// Byte offset of `UNI_ON`.
        opened_at: usize,
    },

    /// Invalid UTF-8 in Unicode fallback block.
    InvalidUtf8InUnicodeMode {
        /// Byte offset of invalid sequence.
        position: usize,
    },

    /// `BACKSPACE` on empty output buffer.
    NothingToErase {
        /// Byte offset of `BACKSPACE`.
        position: usize,
    },

    /// Output buffer too small.
    OutputTooSmall {
        /// Required capacity in bytes.
        needed: usize,
        /// Available capacity in bytes.
        available: usize,
    },
}

/// Encoding error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum EncodeError {
    /// Output buffer too small.
    OutputTooSmall {
        /// Required capacity in bytes.
        needed: usize,
        /// Available capacity in bytes.
        available: usize,
    },

    /// Input is not NFC-normalized.
    NotNfcNormalized {
        /// Byte offset of combining mark.
        position: usize,
    },
}

impl core::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::UnknownByte { position, byte } => {
                write!(f, "unknown byte 0x{byte:02X} at position {position}")
            }
            Self::StackedDiacritics { position } => write!(
                f,
                "two consecutive diacritic modifiers at position {position} (undefined combination)"
            ),
            Self::DanglingModifier { position } => {
                write!(
                    f,
                    "dangling modifier at position {position} without a base character"
                )
            }
            Self::UndefinedCombination {
                position,
                modifier,
                base,
            } => write!(
                f,
                "undefined combination of modifier {modifier} with base {base} at position {position}"
            ),
            Self::UnterminatedUnicodeMode { opened_at } => {
                write!(
                    f,
                    "unterminated unicode fallback mode opened at position {opened_at}"
                )
            }
            Self::InvalidUtf8InUnicodeMode { position } => {
                write!(
                    f,
                    "invalid UTF-8 in unicode fallback mode at position {position}"
                )
            }
            Self::NothingToErase { position } => {
                write!(f, "backspace at position {position} with empty output buffer")
            }
            Self::OutputTooSmall { needed, available } => {
                write!(
                    f,
                    "output buffer too small: {available} bytes available, {needed} required"
                )
            }
        }
    }
}

impl core::fmt::Display for EncodeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::OutputTooSmall { needed, available } => {
                write!(
                    f,
                    "output buffer too small: {available} bytes available, {needed} required"
                )
            }
            Self::NotNfcNormalized { position } => write!(
                f,
                "input is not NFC-normalized: combining mark at position {position}"
            ),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for DecodeError {}
#[cfg(feature = "std")]
impl std::error::Error for EncodeError {}
