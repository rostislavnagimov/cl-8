//! Control and service bytes.

/// Space character, U+0020.
pub const SPACE: u8 = 232;

/// Line feed / newline, U+000A.
pub const ENTER: u8 = 233;

/// Horizontal tab, U+0009.
pub const TAB: u8 = 234;

/// Erases the last emitted character. A stream instruction, not a printable character.
pub const BACKSPACE: u8 = 235;

/// Enter Unicode fallback mode.
pub const UNI_ON: u8 = 236;

/// Capitalize the next resolved character.
pub const CASE_SHIFT: u8 = 237;

/// First diacritic modifier code (inclusive).
pub const MODIFIER_FIRST: u8 = 238;

/// Last diacritic modifier code (inclusive).
pub const MODIFIER_LAST: u8 = 251;

/// Enable uppercase mode until `CAPS_OFF`.
pub const CAPS_ON: u8 = 252;

/// Disable uppercase mode.
pub const CAPS_OFF: u8 = 253;

/// End of message.
pub const EOM: u8 = 254;

/// Cancel entire message.
pub const CANCEL: u8 = 255;

/// Run length at which `CAPS_ON`/`CAPS_OFF` beats a `CASE_SHIFT` prefix per letter.
///
/// One letter costs 1 extra byte as a prefix against 2 for the pair; two are a tie,
/// and the tie goes to prefixes because they leave no open state. Three or more win.
pub const CAPS_TOGGLE_THRESHOLD: usize = 3;

/// Returns `true` for the bytes that close Unicode fallback mode.
///
/// `{0xC0, 0xC1, 0xF5..=0xFF}` never start a canonical UTF-8 sequence — `0xC0`/`0xC1`
/// would be overlong, `0xF5..=0xFF` exceed `U+10FFFF` — so they terminate unambiguously.
#[must_use]
#[inline]
pub const fn is_unicode_terminator(b: u8) -> bool {
    matches!(b, 0xC0 | 0xC1 | 0xF5..=0xFF)
}
