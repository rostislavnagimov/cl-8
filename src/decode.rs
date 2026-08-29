//! Decoder: converts CL-8 byte sequences back into UTF-8.
//!
//! Zero-allocation core: writes directly into a caller-provided byte slice and returns
//! the number of written bytes. Allocating convenience wrapper returning `String` is
//! available in [`crate::alloc_api`] under `feature = "alloc"`.
//!
//! # Byte Dispatch Order
//!
//! 1. Unicode fallback mode active -> stream raw bytes and check for terminator (Section 10).
//! 2. Control codes 232..=236 -> space, newline, tab, backspace, `UNI_ON`.
//! 3. Control codes 252..=255 -> `CAPS_ON`, `CAPS_OFF`, `EOM`, `CANCEL` (Section 9).
//! 4. 237 -> push `CASE_SHIFT` onto modifier stack (Section 8.2).
//! 5. 238..=251 -> push diacritic modifier onto modifier stack (Section 8.3).
//! 6. Otherwise -> base character: resolve modifier stack, then apply active CAPS mode (Sections 8.4, 9.1).

use crate::caps::CapsState;
use crate::consts::{
    is_unicode_terminator, BACKSPACE, CANCEL, CAPS_OFF, CAPS_ON, CASE_SHIFT, ENTER, EOM,
    MODIFIER_FIRST, MODIFIER_LAST, SPACE, TAB, UNI_ON,
};
use crate::error::DecodeError;
use crate::modifier::ModifierStack;
use crate::tables::{codepoint_of, to_upper, Modifier};
use crate::unicode_fallback::UnicodeMode;

/// Decodes a CL-8 byte stream into `output`, returning the number of UTF-8 bytes written.
///
/// # Errors
///
/// Returns [`DecodeError`] if the input sequence is malformed or the output buffer is too small.
///
/// # Examples
///
/// ```
/// # use cl8::decode::decode_into;
/// // 237 = case-shift, 10 = 'a' -> "A"
/// let mut buf = [0u8; 8];
/// let n = decode_into(&[237, 10], &mut buf)?;
/// assert_eq!(&buf[..n], b"A");
/// # Ok::<(), cl8::DecodeError>(())
/// ```
pub fn decode_into(input: &[u8], output: &mut [u8]) -> Result<usize, DecodeError> {
    let mut decoder = Decoder::new();
    decoder.decode(input, output)
}

/// State-machine decoder for CL-8.
#[derive(Debug, Clone)]
pub struct Decoder {
    position: usize,
    output_offset: usize,
    modifier_stack: ModifierStack,
    caps_state: CapsState,
    unicode_mode: UnicodeMode,
    terminated: bool,
}

impl Decoder {
    /// Creates a new decoder instance.
    #[must_use]
    pub fn new() -> Self {
        Self {
            position: 0,
            output_offset: 0,
            modifier_stack: ModifierStack::new(),
            caps_state: CapsState::new(),
            unicode_mode: UnicodeMode::new(),
            terminated: false,
        }
    }

    /// Decodes the entire input slice into the output buffer.
    ///
    /// # Errors
    ///
    /// Returns [`DecodeError`] on invalid input sequences or output buffer overflow.
    pub fn decode(&mut self, input: &[u8], output: &mut [u8]) -> Result<usize, DecodeError> {
        while self.position < input.len() && !self.terminated {
            let byte = input[self.position];
            self.process_byte(byte, output)?;
            self.position += 1;
        }

        // Validate that Unicode mode was properly terminated
        if self.unicode_mode.is_active() {
            self.unicode_mode.finish()?;
        }

        // Validate that modifier stack has no dangling modifiers
        if !self.modifier_stack.is_empty() {
            return Err(DecodeError::DanglingModifier {
                position: self.position.saturating_sub(1),
            });
        }

        Ok(self.output_offset)
    }

    /// Processes a single input byte according to the specification dispatch rules.
    fn process_byte(&mut self, byte: u8, output: &mut [u8]) -> Result<(), DecodeError> {
        // 1. Unicode mode active -> stream byte or handle terminator
        if self.unicode_mode.is_active() {
            return self.handle_unicode_mode(byte, output);
        }

        // 2. Control codes 232..=236 -> space, newline, tab, backspace, UNI_ON
        if byte == SPACE {
            self.write_char(' ', output)?;
            return Ok(());
        }
        if byte == ENTER {
            self.write_char('\n', output)?;
            return Ok(());
        }
        if byte == TAB {
            self.write_char('\t', output)?;
            return Ok(());
        }
        if byte == BACKSPACE {
            return self.handle_backspace(output);
        }
        if byte == UNI_ON {
            self.unicode_mode.open(self.position, self.output_offset);
            return Ok(());
        }

        // 3. Control codes 252..=255 -> CAPS_ON, CAPS_OFF, EOM, CANCEL
        if byte == CAPS_ON {
            self.caps_state.turn_on();
            return Ok(());
        }
        if byte == CAPS_OFF {
            self.caps_state.turn_off();
            return Ok(());
        }
        if byte == EOM {
            self.terminated = true;
            return Ok(());
        }
        if byte == CANCEL {
            self.output_offset = 0;
            return Ok(());
        }

        // 4. 237 -> case-shift into modifier stack
        if byte == CASE_SHIFT {
            self.modifier_stack.push_case_shift(self.position);
            return Ok(());
        }

        // 5. 238..=251 -> diacritic into modifier stack
        if (MODIFIER_FIRST..=MODIFIER_LAST).contains(&byte) {
            let modifier = Modifier::from_byte(byte).ok_or(DecodeError::UnknownByte {
                position: self.position,
                byte,
            })?;
            self.modifier_stack
                .push_diacritic(modifier, self.position)?;
            return Ok(());
        }

        // 6. Otherwise -> base character: apply stack and CAPS state
        if byte < 232 {
            codepoint_of(byte).ok_or(DecodeError::UnknownByte {
                position: self.position,
                byte,
            })?;

            // Resolve modifiers on base character
            let final_codepoint = self.modifier_stack.resolve(byte, self.position)?;

            // Apply active CAPS mode
            let mut char_to_write = final_codepoint;
            if self.caps_state.applies() {
                if let Some(upper) = to_upper(final_codepoint) {
                    char_to_write = upper;
                }
            }

            self.write_char(
                char::from_u32(char_to_write).unwrap_or(char::REPLACEMENT_CHARACTER),
                output,
            )?;

            self.modifier_stack.clear();
            return Ok(());
        }

        Err(DecodeError::UnknownByte {
            position: self.position,
            byte,
        })
    }

    /// Handles bytes when Unicode fallback mode is active.
    fn handle_unicode_mode(&mut self, byte: u8, output: &mut [u8]) -> Result<(), DecodeError> {
        if is_unicode_terminator(byte) {
            let start = self.unicode_mode.output_start();
            let bytes = &output[start..self.output_offset];
            crate::unicode_fallback::validate_canonical(bytes)?;
            if core::str::from_utf8(bytes).is_err() {
                return Err(DecodeError::InvalidUtf8InUnicodeMode { position: start });
            }
            self.unicode_mode.close();
            return Ok(());
        }

        self.write_byte(byte, output)?;
        Ok(())
    }

    /// Handles the `BACKSPACE` control byte by erasing the entire preceding UTF-8 codepoint.
    fn handle_backspace(&mut self, output: &mut [u8]) -> Result<(), DecodeError> {
        if self.output_offset == 0 {
            return Err(DecodeError::NothingToErase {
                position: self.position,
            });
        }

        let char_len = utf8_char_len(output, self.output_offset);
        self.output_offset = self.output_offset.saturating_sub(char_len);
        Ok(())
    }

    /// Encodes and writes a single `char` into the output buffer as UTF-8.
    fn write_char(&mut self, ch: char, output: &mut [u8]) -> Result<(), DecodeError> {
        let mut buf = [0u8; 4];
        let bytes = ch.encode_utf8(&mut buf).as_bytes();
        let len = bytes.len();

        if self.output_offset + len > output.len() {
            return Err(DecodeError::OutputTooSmall {
                needed: (self.output_offset + len).max(3),
                available: output.len(),
            });
        }

        output[self.output_offset..self.output_offset + len].copy_from_slice(bytes);
        self.output_offset += len;
        Ok(())
    }

    /// Writes a raw byte into the output buffer with bounds checking.
    fn write_byte(&mut self, byte: u8, output: &mut [u8]) -> Result<(), DecodeError> {
        if self.output_offset + 1 > output.len() {
            return Err(DecodeError::OutputTooSmall {
                needed: self.output_offset + 1,
                available: output.len(),
            });
        }

        output[self.output_offset] = byte;
        self.output_offset += 1;
        Ok(())
    }
}

impl Default for Decoder {
    fn default() -> Self {
        Self::new()
    }
}

/// Calculates the byte length of the last UTF-8 codepoint in `output`.
fn utf8_char_len(output: &[u8], offset: usize) -> usize {
    if offset == 0 {
        return 0;
    }
    let mut i = offset;
    while i > 0 {
        i -= 1;
        let byte = output[i];
        if byte < 0x80 {
            return offset - i;
        }
        if (byte & 0xE0) == 0xC0 {
            return offset - i;
        }
        if (byte & 0xF0) == 0xE0 {
            return offset - i;
        }
        if (byte & 0xF8) == 0xF0 {
            return offset - i;
        }
    }
    1
}

/// Guaranteed upper bound for decoded UTF-8 text length in bytes.
#[must_use]
#[inline]
pub const fn max_decoded_len(input_len: usize) -> usize {
    input_len * 4
}
