#![allow(clippy::cast_possible_truncation)]

//! UTF-8 to CL-8 encoder.

pub use crate::caps::{CapsRun, CapsState, CapsStrategy};
use crate::consts::{
    CAPS_OFF, CAPS_ON, CAPS_TOGGLE_THRESHOLD, CASE_SHIFT, ENTER, SPACE, TAB, UNI_ON,
};
use crate::error::EncodeError;
use crate::tables::{code_of, find_modifier_pair_for_codepoint, from_upper_to_lower, Modifier};

/// Encodes `input` into `output`. Returns bytes written.
///
/// ```
/// # use cl8::encode::encode_into;
/// let mut buf = [0u8; 8];
/// let n = encode_into("A", &mut buf)?;
/// assert_eq!(&buf[..n], &[161]);
/// # Ok::<(), cl8::EncodeError>(())
/// ```
pub fn encode_into(input: &str, output: &mut [u8]) -> Result<usize, EncodeError> {
    let mut encoder = Encoder::new();
    encoder.encode(input, output)
}

/// Character encoding plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CharPlan {
    /// Control whitespace byte.
    Control(u8),
    /// Base table byte.
    Base(u8),
    /// Modifier and base byte.
    Mod(Modifier, u8),
    /// Turkish capital `İ`.
    TurkishI,
    /// Case shift and base byte.
    CsBase(u8),
    /// Case shift, modifier and base byte.
    CsMod(Modifier, u8),
    /// Unicode fallback character.
    Fallback,
}

impl CharPlan {
    /// Resolves encoding plan for `ch`.
    #[must_use]
    #[inline]
    pub fn of(ch: char) -> Self {
        let codepoint = ch as u32;

        match codepoint {
            0x0020 => return Self::Control(SPACE),
            0x000A => return Self::Control(ENTER),
            0x0009 => return Self::Control(TAB),
            _ => {}
        }

        if let Some(code) = code_of(codepoint) {
            return Self::Base(code);
        }

        if let Some((modifier, base)) = find_modifier_pair_for_codepoint(codepoint) {
            return Self::Mod(modifier, base);
        }

        if codepoint == 0x0130 {
            return Self::TurkishI;
        }

        if let Some(lower) = from_upper_to_lower(codepoint) {
            if let Some((modifier, base)) = find_modifier_pair_for_codepoint(lower) {
                return Self::CsMod(modifier, base);
            }
            if let Some(code) = code_of(lower) {
                return Self::CsBase(code);
            }
        }

        Self::Fallback
    }

    /// Returns `true` if plan requires a case shift prefix.
    #[must_use]
    pub const fn needs_case_shift(self) -> bool {
        matches!(self, Self::CsBase(_) | Self::CsMod(..))
    }
}

/// Streaming encoder.
#[derive(Debug, Clone, Default)]
pub struct Encoder {
    output_offset: usize,
    caps_state: CapsState,
    caps_run_remaining: usize,
    in_unicode_mode: bool,
}

impl Encoder {
    /// Creates an encoder.
    #[must_use]
    pub fn new() -> Self {
        Self {
            output_offset: 0,
            caps_state: CapsState::new(),
            caps_run_remaining: 0,
            in_unicode_mode: false,
        }
    }

    /// Encodes `input` into `output`. Errors: [`EncodeError`].
    pub fn encode(&mut self, input: &str, output: &mut [u8]) -> Result<usize, EncodeError> {
        Self::validate_nfc(input)?;

        for (byte_idx, ch) in input.char_indices() {
            let plan = CharPlan::of(ch);

            if !self.caps_state.applies() && plan.needs_case_shift() {
                let (run_len, shifts) =
                    crate::caps::scan_run(&input[byte_idx + ch.len_utf8()..]);
                if shifts >= CAPS_TOGGLE_THRESHOLD {
                    self.ensure_unicode_closed(output)?;
                    self.write_byte(CAPS_ON, output)?;
                    self.caps_state.turn_on();
                    self.caps_run_remaining = run_len;
                }
            }

            self.write_plan(plan, ch, output)?;

            if self.caps_state.applies() {
                self.caps_run_remaining = self.caps_run_remaining.saturating_sub(1);
                if self.caps_run_remaining == 0 {
                    self.write_byte(CAPS_OFF, output)?;
                    self.caps_state.turn_off();
                }
            }
        }

        if self.in_unicode_mode {
            self.write_byte(0xC0, output)?;
            self.in_unicode_mode = false;
        }

        Ok(self.output_offset)
    }

    /// Rejects input with combining marks.
    fn validate_nfc(input: &str) -> Result<(), EncodeError> {
        if !input.as_bytes().iter().any(|&b| b == 0xCC || b == 0xCD) {
            return Ok(());
        }
        for (i, ch) in input.char_indices() {
            if (0x0300..=0x036F).contains(&(ch as u32)) {
                return Err(EncodeError::NotNfcNormalized { position: i });
            }
        }
        Ok(())
    }

    /// Closes active Unicode fallback mode.
    fn ensure_unicode_closed(&mut self, output: &mut [u8]) -> Result<(), EncodeError> {
        if self.in_unicode_mode {
            self.write_byte(0xC0, output)?;
            self.in_unicode_mode = false;
        }
        Ok(())
    }

    /// Emits bytes for `plan`.
    fn write_plan(
        &mut self,
        plan: CharPlan,
        ch: char,
        output: &mut [u8],
    ) -> Result<(), EncodeError> {
        match plan {
            CharPlan::Control(byte) | CharPlan::Base(byte) => {
                self.ensure_unicode_closed(output)?;
                self.write_byte(byte, output)
            }
            CharPlan::Mod(modifier, base) => {
                self.ensure_unicode_closed(output)?;
                self.write_byte(modifier as u8, output)?;
                self.write_byte(base, output)
            }
            CharPlan::TurkishI => {
                self.ensure_unicode_closed(output)?;
                self.write_byte(Modifier::DotAbove as u8, output)?;
                self.write_byte(CASE_SHIFT, output)?;
                self.write_byte(18, output)
            }
            CharPlan::CsBase(code) => {
                self.ensure_unicode_closed(output)?;
                if !self.caps_state.applies() {
                    self.write_byte(CASE_SHIFT, output)?;
                }
                self.write_byte(code, output)
            }
            CharPlan::CsMod(modifier, base) => {
                self.ensure_unicode_closed(output)?;
                if !self.caps_state.applies() {
                    self.write_byte(CASE_SHIFT, output)?;
                }
                self.write_byte(modifier as u8, output)?;
                self.write_byte(base, output)
            }
            CharPlan::Fallback => self.encode_unicode_fallback(ch, output),
        }
    }

    /// Streams character via Unicode fallback mode.
    fn encode_unicode_fallback(&mut self, ch: char, output: &mut [u8]) -> Result<(), EncodeError> {
        if !self.in_unicode_mode {
            self.write_byte(UNI_ON, output)?;
            self.in_unicode_mode = true;
        }
        let len = ch.len_utf8();
        if self.output_offset + len > output.len() {
            return Err(EncodeError::OutputTooSmall {
                needed: (self.output_offset + len).max(3),
                available: output.len(),
            });
        }
        ch.encode_utf8(&mut output[self.output_offset..]);
        self.output_offset += len;
        Ok(())
    }

    /// Writes single byte to output.
    fn write_byte(&mut self, byte: u8, output: &mut [u8]) -> Result<(), EncodeError> {
        if self.output_offset + 1 > output.len() {
            return Err(EncodeError::OutputTooSmall {
                needed: (self.output_offset + 1).max(3),
                available: output.len(),
            });
        }

        output[self.output_offset] = byte;
        self.output_offset += 1;
        Ok(())
    }
}

/// Maximum buffer size needed to encode `input_len` UTF-8 bytes.
#[must_use]
#[inline]
pub const fn max_encoded_len(input_len: usize) -> usize {
    input_len * 3 + 2
}
