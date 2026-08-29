//! Error types for encoding and decoding.
//!
//! Both enums are zero-allocation and `Copy`, designed to operate in `no_std` environments
//! without an allocator. Error context is conveyed via numeric offsets and bytes.

/// An error that occurred while decoding a CL-8 byte sequence into UTF-8.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum DecodeError {
    /// Encountered a byte that corresponds neither to a table character nor a control code.
    ///
    /// Occurs for reserved codes 220..=231 which are unassigned in the specification.
    UnknownByte {
        /// Byte offset from the start of the input stream.
        position: usize,
        /// The unassigned byte value.
        byte: u8,
    },

    /// Two diacritic modifiers occurred in succession before a single base letter.
    ///
    /// Section 8.4 declares multiple stacked diacritics undefined and requires
    /// treating them as input errors. `CASE_SHIFT` combined with a single diacritic
    /// is valid and does not trigger this error.
    StackedDiacritics {
        /// Position of the second diacritic modifier.
        position: usize,
    },

    /// A modifier occurred at the very end of the stream without a following base character.
    DanglingModifier {
        /// Position of the modifier.
        position: usize,
    },

    /// The (modifier, base) pair is undefined in the specification table (Section 8.3).
    UndefinedCombination {
        /// Position of the base character.
        position: usize,
        /// Modifier control byte.
        modifier: u8,
        /// Base character code.
        base: u8,
    },

    /// The input stream ended while Unicode fallback mode was still open.
    UnterminatedUnicodeMode {
        /// Position of the `UNI_ON` byte that initiated the mode.
        opened_at: usize,
    },

    /// Bytes inside a Unicode fallback block do not form valid canonical UTF-8.
    InvalidUtf8InUnicodeMode {
        /// Position of the start of the invalid sequence.
        position: usize,
    },

    /// Encountered `BACKSPACE` when the output buffer was empty.
    NothingToErase {
        /// Position of the `BACKSPACE` byte.
        position: usize,
    },

    /// The provided output buffer is smaller than required for the decoded text.
    ///
    /// Callers can compute a guaranteed upper bound with [`crate::max_decoded_len`].
    OutputTooSmall {
        /// Required buffer capacity in bytes.
        needed: usize,
        /// Available capacity in bytes.
        available: usize,
    },
}

/// An error that occurred while encoding text into CL-8.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum EncodeError {
    /// The provided output buffer is smaller than required for the encoded data.
    ///
    /// Callers can compute a guaranteed upper bound with [`crate::max_encoded_len`].
    OutputTooSmall {
        /// Required buffer capacity in bytes.
        needed: usize,
        /// Available capacity in bytes.
        available: usize,
    },

    /// Input is not normalized in Unicode Normalization Form C (NFC).
    ///
    /// Section 10.3 requires NFC preprocessing. The encoder detects standalone
    /// combining marks and rejects them to prevent improper Unicode fallback delegation.
    NotNfcNormalized {
        /// Character offset of the standalone combining mark in the input.
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
