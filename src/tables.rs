//! Lookup tables generated at compile time by `build.rs` from `tables/*.csv`.
//!
//! The generated table file resides in `OUT_DIR` and is included via `include!`.
//! It is omitted from the source tree to ensure strict consistency with the CSV sources.

include!(concat!(env!("OUT_DIR"), "/tables.rs"));

/// Returns the Unicode codepoint for a given CL-8 byte code.
///
/// Returns `None` for unassigned codes: the reserved range 220..=231
/// and control codes 232..=255.
#[must_use]
#[inline]
pub fn codepoint_of(code: u8) -> Option<u32> {
    BASE_TABLE.get(code as usize).copied()
}

/// Returns the CL-8 byte code for a Unicode codepoint.
///
/// For over 95% of standard text (digits, Latin `a-z`/`A-Z`, basic Cyrillic `а-я`/`А-Я`),
/// the code is resolved in $O(1)$ time using direct arithmetic.
/// For punctuation and supplementary characters (codes 69..160), a linear scan over the table slice is used.
#[must_use]
#[inline]
#[allow(clippy::cast_possible_truncation)]
pub fn code_of(codepoint: u32) -> Option<u8> {
    match codepoint {
        0x0030..=0x0039 => Some((codepoint - 0x0030) as u8),
        0x0061..=0x007A => Some((codepoint - 0x0061 + 10) as u8),
        0x0430..=0x044F => {
            let offset = codepoint - 0x0430;
            if offset < 6 {
                Some((36 + offset) as u8)
            } else {
                Some((36 + offset + 1) as u8)
            }
        }
        0x0451 => Some(42),
        0x0041..=0x005A => Some((codepoint - 0x0041 + 161) as u8),
        0x0410..=0x042F => {
            let offset = codepoint - 0x0410;
            if offset < 6 {
                Some((187 + offset) as u8)
            } else {
                Some((187 + offset + 1) as u8)
            }
        }
        0x0401 => Some(193),
        _ => {
            // Search supplementary Cyrillic, special symbols, and punctuation (codes 69..160)
            let mut i = 69usize;
            let limit = BASE_TABLE.len().min(161);
            while i < limit {
                if BASE_TABLE[i] == codepoint {
                    return Some(i as u8);
                }
                i += 1;
            }
            if BASE_TABLE.len() > 220 {
                let mut j = 220usize;
                while j < BASE_TABLE.len() {
                    if BASE_TABLE[j] == codepoint {
                        return Some(j as u8);
                    }
                    j += 1;
                }
            }
            None
        }
    }
}

/// Applies a diacritic modifier to a base character code.
///
/// Returns `None` if the combination is undefined in Section 8.3 of the specification.
#[must_use]
pub fn apply_modifier(modifier: Modifier, base: u8) -> Option<u32> {
    let mut i = 0usize;
    while i < MODIFIER_TABLE.len() {
        let (m, b, result) = MODIFIER_TABLE[i];
        if m as u8 == modifier as u8 && b == base {
            return Some(result);
        }
        i += 1;
    }
    None
}

/// Returns the uppercase form of a Unicode codepoint.
///
/// First applies algorithmic shifts (`-0x20`) for standard ASCII `a..z` and Cyrillic `а..я`,
/// then performs a binary search over `CAPITALIZE_TABLE`. Returns `None` if the character
/// has no uppercase counterpart (digits, punctuation, symbols).
#[must_use]
pub fn to_upper(codepoint: u32) -> Option<u32> {
    // Latin a..z and basic Cyrillic а..я are regular pairs with 0x20 shift
    if matches!(codepoint, 0x0061..=0x007A | 0x0430..=0x044F) {
        return Some(codepoint - 0x20);
    }
    if codepoint == 0x0451 {
        return Some(0x0401);
    }
    let mut lo = 0usize;
    let mut hi = CAPITALIZE_TABLE.len();
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        let (lower, upper) = CAPITALIZE_TABLE[mid];
        match lower.cmp(&codepoint) {
            core::cmp::Ordering::Equal => return Some(upper),
            core::cmp::Ordering::Less => lo = mid + 1,
            core::cmp::Ordering::Greater => hi = mid,
        }
    }
    None
}

/// Finds the `(Modifier, base_code)` pair that produces the given composite codepoint.
#[must_use]
pub fn find_modifier_pair_for_codepoint(codepoint: u32) -> Option<(Modifier, u8)> {
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

/// Returns the lowercase form of an uppercase Unicode codepoint.
///
/// First applies algorithmic shifts (`+0x20`) for standard ASCII `A..Z` and Cyrillic `А..Я`,
/// handles special cases (e.g. Turkish `İ`), and performs a binary search over `REVERSE_CAPITALIZE_TABLE`.
/// Returns `None` if the character has no lowercase counterpart.
#[must_use]
pub fn from_upper_to_lower(codepoint: u32) -> Option<u32> {
    // Latin A..Z and basic Cyrillic А..Я are regular pairs with 0x20 shift
    if matches!(codepoint, 0x0041..=0x005A | 0x0410..=0x042F) {
        return Some(codepoint + 0x20);
    }
    if codepoint == 0x0401 {
        return Some(0x0451);
    }
    // Turkish capital I with dot (U+0130) -> lowercase i (U+0069)
    if codepoint == 0x0130 {
        return Some(0x0069);
    }

    // Search in the reverse capitalization table (sorted by uppercase codepoints)
    let mut lo = 0usize;
    let mut hi = REVERSE_CAPITALIZE_TABLE.len();
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        let (upper, lower) = REVERSE_CAPITALIZE_TABLE[mid];
        match upper.cmp(&codepoint) {
            core::cmp::Ordering::Equal => return Some(lower),
            core::cmp::Ordering::Less => lo = mid + 1,
            core::cmp::Ordering::Greater => hi = mid,
        }
    }
    None
}
