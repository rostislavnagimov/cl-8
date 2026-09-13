#![allow(clippy::cast_possible_truncation)]

//! Encoder: UTF-8 to CL-8.
//!
//! Writes into a caller-provided slice and returns the byte count.
//! [`crate::alloc_api`] adds a `Vec<u8>`-returning wrapper under `feature = "alloc"`.
//!
//! Input must be NFC-normalized. The encoder cannot normalize — that needs the Unicode
//! tables a `no_std` core exists to avoid — so standalone combining marks are rejected
//! with [`EncodeError::NotNfcNormalized`].
//!
//! Every character resolves to one [`CharPlan`], which the encoder then writes.
//! Before writing, it may open a CAPS run: see [`crate::caps`] for that arithmetic.

pub use crate::caps::{CapsRun, CapsState, CapsStrategy};
use crate::consts::{
    CAPS_OFF, CAPS_ON, CAPS_TOGGLE_THRESHOLD, CASE_SHIFT, ENTER, SPACE, TAB, UNI_ON,
};
use crate::error::EncodeError;
use crate::tables::{code_of, find_modifier_pair_for_codepoint, from_upper_to_lower, Modifier};

/// Encodes `input` into `output`, returning the number of bytes written.
///
/// # Errors
///
/// * [`EncodeError::OutputTooSmall`] — see [`max_encoded_len`] for a sufficient size.
/// * [`EncodeError::NotNfcNormalized`] — input carries standalone combining marks.
///
/// # Examples
///
/// ```
/// # use cl8::encode::encode_into;
/// let mut buf = [0u8; 8];
/// // 'A' encodes directly into 1 byte (code 161)
/// let n = encode_into("A", &mut buf)?;
/// assert_eq!(&buf[..n], &[161]);
/// # Ok::<(), cl8::EncodeError>(())
/// ```
pub fn encode_into(input: &str, output: &mut [u8]) -> Result<usize, EncodeError> {
    let mut encoder = Encoder::new();
    encoder.encode(input, output)
}

/// What the encoder will do with one character.
///
/// Resolved once per character. Carrying the answer keeps CAPS-run scanning and byte
/// emission from re-deriving the same facts through separate table searches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CharPlan {
    /// Whitespace with a dedicated control byte: space, newline, tab.
    Control(u8),
    /// Present in the base table — one byte.
    Base(u8),
    /// Composite diacritic — `modifier + base`, two bytes.
    Mod(Modifier, u8),
    /// Turkish capital `İ` — `DotAbove + CASE_SHIFT + 'i'`, three bytes.
    TurkishI,
    /// Uppercase whose lowercase is a base character — `CASE_SHIFT + base`.
    CsBase(u8),
    /// Uppercase whose lowercase is a composite — `CASE_SHIFT + modifier + base`.
    CsMod(Modifier, u8),
    /// Outside the tables — streams through Unicode fallback.
    Fallback,
}

impl CharPlan {
    /// Resolves how `ch` will be encoded, first matching branch wins.
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

    /// Returns `true` if the character costs an extra `CASE_SHIFT` byte outside a run.
    ///
    /// Direct uppercase (`A-Z`, `А-Я`, `Ё`) is [`CharPlan::Base`] and gains nothing from
    /// the mode; Turkish `İ` keeps its three-byte form either way.
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
    /// Number of characters left in the currently open `CAPS_ON` run.
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

    /// Encodes the whole string into `output`.
    ///
    /// # Errors
    ///
    /// [`EncodeError`] if input is not NFC-normalized or `output` is too small.
    pub fn encode(&mut self, input: &str, output: &mut [u8]) -> Result<usize, EncodeError> {
        Self::validate_nfc(input)?;

        for (byte_idx, ch) in input.char_indices() {
            let plan = CharPlan::of(ch);

            // Open a run only when the stretch ahead carries enough shift-requiring
            // characters to pay for the two toggle bytes.
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

    /// Rejects input carrying standalone combining marks (U+0300..=U+036F).
    fn validate_nfc(input: &str) -> Result<(), EncodeError> {
        // Every combining mark in that block starts with 0xCC or 0xCD in UTF-8, so a raw
        // byte scan clears the overwhelmingly common case without decoding characters.
        // Only a hit pays for the precise pass that locates the offending character.
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

    /// Closes an open Unicode block.
    fn ensure_unicode_closed(&mut self, output: &mut [u8]) -> Result<(), EncodeError> {
        if self.in_unicode_mode {
            self.write_byte(0xC0, output)?;
            self.in_unicode_mode = false;
        }
        Ok(())
    }

    /// Emits the bytes for an already-resolved [`CharPlan`].
    fn write_plan(
        &mut self,
        plan: CharPlan,
        ch: char,
        output: &mut [u8],
    ) -> Result<(), EncodeError> {
        // Fallback is the only plan that continues an open Unicode block; every other
        // one closes it first. The single-byte arm comes first and stays branch-free:
        // it carries over 95% of real text.
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
            // Inside a run the mode capitalizes for us — no prefix needed.
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

    /// Streams an unsupported character through Unicode fallback mode.
    fn encode_unicode_fallback(&mut self, ch: char, output: &mut [u8]) -> Result<(), EncodeError> {
        if !self.in_unicode_mode {
            self.write_byte(UNI_ON, output)?;
            self.in_unicode_mode = true;
        }
        // One bounds check and a direct encode, instead of one check per UTF-8 byte.
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

    /// Writes one byte.
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

/// Output size that always suffices for encoding `input_len` bytes.
#[must_use]
#[inline]
pub const fn max_encoded_len(input_len: usize) -> usize {
    input_len * 3 + 2
}
