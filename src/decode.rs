//! Decoder: CL-8 bytes to UTF-8.
//!
//! Writes into a caller-provided slice and returns the byte count.
//! [`crate::alloc_api`] adds a `String`-returning wrapper under `feature = "alloc"`.
//!
//! A state machine over five pieces of state: position, modifier stack, CAPS mode,
//! Unicode mode, and a terminated flag. Modifiers emit nothing until a base byte
//! arrives; base bytes resolve the stack, then the CAPS mode, then write.

use crate::caps::CapsState;
use crate::consts::{
    is_unicode_terminator, BACKSPACE, CANCEL, CAPS_OFF, CAPS_ON, CASE_SHIFT, ENTER, EOM,
    MODIFIER_FIRST, MODIFIER_LAST, SPACE, TAB, UNI_ON,
};
use crate::error::DecodeError;
use crate::modifier::ModifierStack;
use crate::tables::{to_upper, Modifier};
use crate::unicode_fallback::UnicodeMode;

/// Decodes into `output`. Returns UTF-8 bytes written.
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

/// Streaming decoder.
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
    /// Creates a decoder.
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

    /// Decodes the whole input into `output`. Errors: [`DecodeError`] on invalid sequence or full buffer.
    pub fn decode(&mut self, input: &[u8], output: &mut [u8]) -> Result<usize, DecodeError> {
        while self.position < input.len() && !self.terminated {
            let byte = input[self.position];
            self.process_byte(byte, output)?;
            self.position += 1;
        }

        if self.unicode_mode.is_active() {
            self.unicode_mode.finish()?;
        }

        if !self.modifier_stack.is_empty() {
            return Err(DecodeError::DanglingModifier {
                position: self.position.saturating_sub(1),
            });
        }

        Ok(self.output_offset)
    }

    /// Dispatches one input byte.
    fn process_byte(&mut self, byte: u8, output: &mut [u8]) -> Result<(), DecodeError> {
        if self.unicode_mode.is_active() {
            return self.handle_unicode_mode(byte, output);
        }

        // Base chars are below SPACE and dominate streams; test first.
        if byte < SPACE {
            return self.write_base(byte, output);
        }

        match byte {
            SPACE => self.write_char(' ', output),
            ENTER => self.write_char('\n', output),
            TAB => self.write_char('\t', output),
            BACKSPACE => self.handle_backspace(output),
            UNI_ON => {
                self.unicode_mode.open(self.position, self.output_offset);
                Ok(())
            }
            CASE_SHIFT => {
                self.modifier_stack.push_case_shift(self.position);
                Ok(())
            }
            MODIFIER_FIRST..=MODIFIER_LAST => {
                let modifier = Modifier::from_byte(byte).ok_or(DecodeError::UnknownByte {
                    position: self.position,
                    byte,
                })?;
                self.modifier_stack.push_diacritic(modifier, self.position)
            }
            CAPS_ON => {
                self.caps_state.turn_on();
                Ok(())
            }
            CAPS_OFF => {
                self.caps_state.turn_off();
                Ok(())
            }
            EOM => {
                self.terminated = true;
                Ok(())
            }
            CANCEL => {
                self.output_offset = 0;
                Ok(())
            }
            _ => Err(DecodeError::UnknownByte {
                position: self.position,
                byte,
            }),
        }
    }

    /// Resolves base byte through modifier stack and CAPS, then writes it.
    fn write_base(&mut self, byte: u8, output: &mut [u8]) -> Result<(), DecodeError> {
        let mut codepoint = self.modifier_stack.resolve(byte, self.position)?;
        if self.caps_state.applies() {
            if let Some(upper) = to_upper(codepoint) {
                codepoint = upper;
            }
        }
        self.write_char(
            char::from_u32(codepoint).unwrap_or(char::REPLACEMENT_CHARACTER),
            output,
        )?;
        self.modifier_stack.clear();
        Ok(())
    }

    /// Streams a byte inside a Unicode block, or closes the block on a terminator.
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

    /// Erases the whole preceding codepoint, not a single byte.
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

    /// Writes one `char` into the output buffer as UTF-8.
    fn write_char(&mut self, ch: char, output: &mut [u8]) -> Result<(), DecodeError> {
        let len = ch.len_utf8();
        if self.output_offset + len > output.len() {
            return Err(DecodeError::OutputTooSmall {
                needed: (self.output_offset + len).max(3),
                available: output.len(),
            });
        }
        ch.encode_utf8(&mut output[self.output_offset..]);
        self.output_offset += len;
        Ok(())
    }

    /// Writes one raw byte.
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

/// Length in bytes of the last UTF-8 codepoint in `output`.
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

/// Output size that always suffices for decoding `input_len` bytes.
#[must_use]
#[inline]
pub const fn max_decoded_len(input_len: usize) -> usize {
    input_len * 4
}
