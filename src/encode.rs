#![allow(clippy::cast_possible_truncation)]

//! Encoder: converts UTF-8 strings into compact CL-8 byte streams.
//!
//! Zero-allocation core: writes directly into a caller-provided byte slice and returns
//! the number of written bytes. Allocating convenience wrapper returning `Vec<u8>` is
//! available in [`crate::alloc_api`] under `feature = "alloc"`.
//!
//! # Required Preprocessing
//!
//! Section 10.3: Input text must be valid canonical UTF-8 in Normalization Form C (NFC).
//! The encoder does not normalize on the fly (which would require extensive Unicode tables
//! and external dependencies forbidden in the `no_std` core). Instead, it detects un-normalized
//! inputs with standalone combining characters and returns [`EncodeError::NotNfcNormalized`].
//!
//! # Decision Pipeline for Each Character
//!
//! 1. Direct whitespace control byte (space, newline, tab).
//! 2. Character exists in the base table (including direct ASCII uppercase `A-Z` and Cyrillic `А-Я`, `Ё`) -> 1 byte.
//! 3. Character is a known composite diacritic pair -> 2 bytes (`modifier + base`).
//! 4. Special case for Turkish capital `İ` (`U+0130`) -> 3 bytes (`DotAbove + CASE_SHIFT + 'i'`).
//! 5. Character is an uppercase letter whose lowercase form is in the table -> capitalization logic.
//! 6. Otherwise -> Unicode fallback: `UNI_ON` (236), raw UTF-8 bytes, inline terminator (Section 10).
//!
//! # CAPS-Toggle Runs (Section 9.1)
//!
//! Uppercase characters that would each need a `CASE_SHIFT` prefix (uppercase supplementary
//! Cyrillic such as `ЄЇЂ`, uppercase diacritics such as `ČŠŽ`) cost 1 extra byte apiece.
//! Wrapping them in `CAPS_ON` … `CAPS_OFF` costs 2 bytes for the whole run, so a run carrying
//! 3 or more such characters is toggled instead of prefixed.
//!
//! A run is not required to be a solid block of them. Characters that pass through CAPS mode
//! unchanged — digits, punctuation, whitespace, and already-direct uppercase (`A-Z`, `А-Я`, `Ё`) —
//! cost exactly one byte either way, so they extend a run for free and let distant uppercase
//! letters share a single pair of toggle bytes. Lowercase letters would be capitalized by the
//! mode, so they always end a run.

pub use crate::caps::{CapsRun, CapsState, CapsStrategy};
use crate::consts::{
    CAPS_OFF, CAPS_ON, CAPS_TOGGLE_THRESHOLD, CASE_SHIFT, ENTER, SPACE, TAB, UNI_ON,
};
use crate::error::EncodeError;
use crate::tables::{code_of, from_upper_to_lower, to_upper, Modifier, MODIFIER_TABLE};

/// Encodes input UTF-8 text into `output`, returning the number of bytes written.
///
/// # Errors
///
/// * [`EncodeError::OutputTooSmall`] — buffer capacity is insufficient (see [`max_encoded_len`]).
/// * [`EncodeError::NotNfcNormalized`] — input contains standalone combining marks.
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

/// Streaming encoder for CL-8.
#[derive(Debug, Clone, Default)]
pub struct Encoder {
    output_offset: usize,
    caps_state: CapsState,
    /// Number of characters left in the currently open `CAPS_ON` run.
    caps_run_remaining: usize,
    in_unicode_mode: bool,
}

impl Encoder {
    /// Creates a new encoder instance.
    #[must_use]
    pub fn new() -> Self {
        Self {
            output_offset: 0,
            caps_state: CapsState::new(),
            caps_run_remaining: 0,
            in_unicode_mode: false,
        }
    }

    /// Encodes an entire string into the provided output buffer.
    ///
    /// # Errors
    ///
    /// Returns [`EncodeError`] if input is not NFC-normalized or output buffer is too small.
    pub fn encode(&mut self, input: &str, output: &mut [u8]) -> Result<usize, EncodeError> {
        // NFC normalization check
        Self::validate_nfc(input)?;

        for (byte_idx, ch) in input.char_indices() {
            // Section 9.1: open a CAPS_ON run when the upcoming stretch carries enough
            // case-shift-requiring characters to pay for the two toggle bytes.
            if !self.caps_state.applies() && Self::needs_case_shift(ch) {
                let (run_len, shifts) = Self::scan_caps_run(&input[byte_idx + ch.len_utf8()..]);
                if shifts >= CAPS_TOGGLE_THRESHOLD {
                    self.ensure_unicode_closed(output)?;
                    self.write_byte(CAPS_ON, output)?;
                    self.caps_state.turn_on();
                    self.caps_run_remaining = run_len;
                }
            }

            self.encode_char(ch, output)?;

            // Close the run after its last character.
            if self.caps_state.applies() {
                self.caps_run_remaining = self.caps_run_remaining.saturating_sub(1);
                if self.caps_run_remaining == 0 {
                    self.write_byte(CAPS_OFF, output)?;
                    self.caps_state.turn_off();
                }
            }
        }

        // If stream ends while Unicode mode is open, close with a terminator byte
        if self.in_unicode_mode {
            self.write_byte(0xC0, output)?;
            self.in_unicode_mode = false;
        }

        Ok(self.output_offset)
    }

    /// Validates that input contains no standalone combining diacritic marks (U+0300..=U+036F).
    fn validate_nfc(input: &str) -> Result<(), EncodeError> {
        for (i, ch) in input.char_indices() {
            let cp = ch as u32;
            if (0x0300..=0x036F).contains(&cp) {
                return Err(EncodeError::NotNfcNormalized { position: i });
            }
        }
        Ok(())
    }

    /// Closes Unicode fallback mode with a terminator byte if it is currently active.
    fn ensure_unicode_closed(&mut self, output: &mut [u8]) -> Result<(), EncodeError> {
        if self.in_unicode_mode {
            self.write_byte(0xC0, output)?;
            self.in_unicode_mode = false;
        }
        Ok(())
    }

    /// Encodes a single character.
    fn encode_char(&mut self, ch: char, output: &mut [u8]) -> Result<(), EncodeError> {
        let codepoint = ch as u32;

        // 1. Control whitespace characters: space, newline, tab
        if codepoint == 0x0020 {
            self.ensure_unicode_closed(output)?;
            return self.write_byte(SPACE, output);
        }
        if codepoint == 0x000A {
            self.ensure_unicode_closed(output)?;
            return self.write_byte(ENTER, output);
        }
        if codepoint == 0x0009 {
            self.ensure_unicode_closed(output)?;
            return self.write_byte(TAB, output);
        }

        // 2. Character is in base table (including A-Z [161-186], А-Я [187-219], Ё [193]) -> 1 byte
        if let Some(code) = code_of(codepoint) {
            self.ensure_unicode_closed(output)?;
            return self.encode_base_char(code, output);
        }

        // 3. Character is a modifier+base pair -> 2 bytes (Section 8.3)
        if let Some((modifier, base)) = Self::find_modifier_pair(codepoint) {
            self.ensure_unicode_closed(output)?;
            self.write_byte(modifier as u8, output)?;
            self.write_byte(base, output)?;
            return Ok(());
        }

        // 4. Special case for Turkish capital I with dot (U+0130): DotAbove (251) + CASE_SHIFT (237) + 'i' (18)
        if codepoint == 0x0130 {
            self.ensure_unicode_closed(output)?;
            self.write_byte(Modifier::DotAbove as u8, output)?;
            self.write_byte(CASE_SHIFT, output)?;
            self.write_byte(18, output)?;
            return Ok(());
        }

        // 5. Character is uppercase with a lowercase counterpart -> capitalization logic
        if let Some(lower_codepoint) = from_upper_to_lower(codepoint) {
            if let Some((modifier, base)) = Self::find_modifier_pair(lower_codepoint) {
                self.ensure_unicode_closed(output)?;
                // Inside an active CAPS_ON run the mode itself capitalizes the
                // resolved character (Section 9.1) — no CASE_SHIFT prefix needed.
                if !self.caps_state.applies() {
                    self.write_byte(CASE_SHIFT, output)?;
                }
                self.write_byte(modifier as u8, output)?;
                self.write_byte(base, output)?;
                return Ok(());
            } else if let Some(lower_code) = code_of(lower_codepoint) {
                self.ensure_unicode_closed(output)?;
                if self.caps_state.applies() {
                    self.write_byte(lower_code, output)?;
                } else {
                    self.write_byte(CASE_SHIFT, output)?;
                    self.write_byte(lower_code, output)?;
                }
                return Ok(());
            }
        }

        // 6. Otherwise -> Unicode fallback mode: open `UNI_ON` (if not already open) and stream UTF-8 bytes
        self.encode_unicode_fallback(ch, output)?;

        Ok(())
    }

    /// Encodes a base character.
    fn encode_base_char(&mut self, code: u8, output: &mut [u8]) -> Result<(), EncodeError> {
        self.write_byte(code, output)
    }

    /// Returns `true` if the character is an uppercase letter that costs an extra
    /// `CASE_SHIFT` byte when encoded outside a `CAPS_ON` run (Section 9.1 economics).
    ///
    /// Direct-mapped uppercase (`A-Z`, `А-Я`, `Ё`) is already 1 byte and gains nothing
    /// from CAPS mode. Turkish `İ` keeps its dedicated 3-byte form in either mode.
    fn needs_case_shift(ch: char) -> bool {
        let codepoint = ch as u32;
        // Fast rejects keep the hot path free of table scans: ASCII (uppercase
        // `A-Z` is direct-mapped), basic Cyrillic `а-я`/`А-Я`/`ё`/`Ё` (uppercase is
        // direct-mapped), and Turkish `İ` (dedicated 3-byte form).
        if codepoint < 0x80
            || matches!(codepoint, 0x0410..=0x044F)
            || codepoint == 0x0451
            || codepoint == 0x0401
            || codepoint == 0x0130
        {
            return false;
        }
        if code_of(codepoint).is_some() {
            return false;
        }
        match from_upper_to_lower(codepoint) {
            Some(lower) => code_of(lower).is_some() || Self::find_modifier_pair(lower).is_some(),
            None => false,
        }
    }

    /// Returns `true` if an active CAPS run leaves the character untouched, so it can sit
    /// inside a run at no extra cost: digits, punctuation, whitespace, and uppercase letters
    /// that are already direct single-byte codes.
    fn is_caps_neutral(ch: char) -> bool {
        let codepoint = ch as u32;
        // Whitespace becomes its own control byte and never consults CAPS state.
        if matches!(codepoint, 0x0020 | 0x000A | 0x0009) {
            return true;
        }
        // Everything else must be a direct base character that `to_upper` would not change.
        code_of(codepoint).is_some() && to_upper(codepoint).is_none()
    }

    /// Measures the CAPS run that has already started on a case-shift-requiring character,
    /// given the remainder of the input after it.
    ///
    /// Returns `(run_len, shifts)`: the run spans `run_len` characters and ends on the last
    /// one needing a `CASE_SHIFT`, of which there are `shifts` in total. Neutral characters
    /// extend the run only when more shift-requiring characters follow, so a run never
    /// trails past its last uppercase letter.
    fn scan_caps_run(rest: &str) -> (usize, usize) {
        let mut run_len = 1usize;
        let mut shifts = 1usize;
        let mut pending = 0usize;

        for ch in rest.chars() {
            if Self::needs_case_shift(ch) {
                run_len += pending + 1;
                pending = 0;
                shifts += 1;
            } else if Self::is_caps_neutral(ch) {
                pending += 1;
            } else {
                break;
            }
        }

        (run_len, shifts)
    }

    /// Finds the `(Modifier, base_code)` pair for a composite codepoint.
    fn find_modifier_pair(codepoint: u32) -> Option<(Modifier, u8)> {
        let mut i = 0usize;
        while i < MODIFIER_TABLE.len() {
            let (modifier, base, result) = MODIFIER_TABLE[i];
            if result == codepoint {
                return Some((modifier, base));
            }
            i += 1;
        }
        None
    }

    /// Encodes an unsupported character using Unicode fallback mode.
    fn encode_unicode_fallback(&mut self, ch: char, output: &mut [u8]) -> Result<(), EncodeError> {
        if !self.in_unicode_mode {
            self.write_byte(UNI_ON, output)?;
            self.in_unicode_mode = true;
        }

        let mut bytes = [0u8; 4];
        let bytes_slice = ch.encode_utf8(&mut bytes).as_bytes();
        for &byte in bytes_slice {
            self.write_byte(byte, output)?;
        }

        Ok(())
    }

    /// Writes a single byte into the output slice with bounds checking.
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

/// Guaranteed upper bound for encoded stream length in bytes.
#[must_use]
#[inline]
pub const fn max_encoded_len(input_len: usize) -> usize {
    input_len * 3 + 2
}
