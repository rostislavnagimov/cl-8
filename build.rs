//! Code generator for CL-8 lookup tables.
//!
//! Reads human-readable CSV sources from `tables/` and generates `$OUT_DIR/tables.rs`,
//! which is included via `include!` in `src/tables.rs`.
//!
//! # Build-time Validation Invariants
//!
//! 1. The `code` column in `base_table.csv` is strictly sequential from 0 without gaps or duplicates.
//! 2. Codepoints in `base_table.csv` are unique.
//! 3. Modifier combination results do not duplicate base table characters.
//! 4. Modifier results are unique among themselves.
//! 5. Every modifier's base character exists in `base_table.csv`.
//! 6. Lowercase forms in `capitalize_table.csv` are reachable (in base table or via modifiers).
//! 7. Capitalization pairs are not reflexive (`lower != upper`) and have no duplicates.

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

/// `(code, variant_name, doc_comment)`.
///
/// Order must match control codes 238..=251 from Section 8.3 of the specification.
const MODIFIER_NAMES: [(u8, &str, &str); 14] = [
    (
        238,
        "Acute",
        "Acute accent (´). Targets: a, e, i, o, u, y, c, n, s, z, l, r, Cyrillic г, к.",
    ),
    (239, "Grave", "Grave accent. Targets: a, e, i, o, u."),
    (240, "Circumflex", "Circumflex (^). Targets: a, e, i, o, u."),
    (
        241,
        "Umlaut",
        "Umlaut/diaeresis (¨). Targets: a, o, u, e, i, y, Cyrillic і.",
    ),
    (242, "Tilde", "Tilde (~). Targets: a, o, n."),
    (
        243,
        "Caron",
        "Caron/háček (ˇ). Targets: c, d, e, l, n, r, s, t, z.",
    ),
    (244, "Ogonek", "Ogonek (˛). Targets: a, e."),
    (245, "Stroke", "Stroke (/). Targets: l, d, o."),
    (
        246,
        "Cedilla",
        "Cedilla (¸). Targets: c, s. Distinct from `CommaBelow` (ş U+015F vs ș U+0219).",
    ),
    (
        247,
        "CommaBelow",
        "Comma below. Targets: s, t. Distinct codepoints from Cedilla.",
    ),
    (248, "Breve", "Breve (˘). Targets: a, g, Cyrillic у."),
    (249, "DoubleAcute", "Double acute (˝). Targets: o, u."),
    (250, "Ring", "Ring above (˚). Targets: a, u."),
    (
        251,
        "DotAbove",
        "Dot above (˙). Target: z. For 'i', pairs with case-shift (Section 8.4, Turkish İ).",
    ),
];

fn main() {
    for f in [
        "base_table.csv",
        "modifier_table.csv",
        "capitalize_table.csv",
    ] {
        println!("cargo:rerun-if-changed=tables/{f}");
    }
    println!("cargo:rerun-if-changed=build.rs");

    let base = parse_base("tables/base_table.csv");
    let modifiers = parse_modifiers("tables/modifier_table.csv", &base);
    let capitalize = parse_capitalize("tables/capitalize_table.csv", &base, &modifiers);

    let out = env::var("OUT_DIR").expect("OUT_DIR environment variable is not set");
    let mut f =
        File::create(Path::new(&out).join("tables.rs")).expect("failed to create tables.rs");
    emit(&mut f, &base, &modifiers, &capitalize);
}

// ---------------------------------------------------------------- CSV Parsing

/// Strips `# comments` and whitespace. Returns `None` for empty lines.
fn clean(line: &str) -> Option<&str> {
    let body = line.split('#').next().unwrap_or("").trim();
    if body.is_empty() {
        None
    } else {
        Some(body)
    }
}

fn hex(field: &str, ctx: &str) -> u32 {
    let f = field.trim();
    let digits = f
        .strip_prefix("0x")
        .or_else(|| f.strip_prefix("0X"))
        .unwrap_or(f);
    u32::from_str_radix(digits, 16)
        .unwrap_or_else(|e| panic!("{ctx}: failed to parse hex '{f}': {e}"))
}

fn dec(field: &str, ctx: &str) -> u32 {
    let f = field.trim();
    f.parse()
        .unwrap_or_else(|e| panic!("{ctx}: failed to parse integer '{f}': {e}"))
}

fn lines_of(path: &str) -> Vec<(usize, String)> {
    let file = File::open(path).unwrap_or_else(|e| panic!("cannot open {path}: {e}"));
    BufReader::new(file)
        .lines()
        .enumerate()
        .map(|(i, l)| (i + 1, l.unwrap_or_else(|e| panic!("reading {path}: {e}"))))
        .collect()
}

/// Parses base table: `code -> codepoint`.
fn parse_base(path: &str) -> Vec<u32> {
    let mut by_code: BTreeMap<u32, u32> = BTreeMap::new();
    let mut seen_cp: BTreeMap<u32, u32> = BTreeMap::new();

    for (ln, line) in lines_of(path) {
        let Some(body) = clean(&line) else { continue };
        let parts: Vec<&str> = body.split(',').collect();
        assert!(
            parts.len() >= 2,
            "{path}:{ln}: expected 'code,codepoint', found '{body}'"
        );
        let code = dec(parts[0], &format!("{path}:{ln}"));
        let cp = hex(parts[1], &format!("{path}:{ln}"));

        if let Some(prev) = by_code.insert(code, cp) {
            panic!("{path}:{ln}: code {code} is already defined (0x{prev:04X})");
        }
        if let Some(prev_code) = seen_cp.insert(cp, code) {
            panic!(
                "{path}:{ln}: codepoint 0x{cp:04X} is already assigned to code {prev_code}; \
                 a single character cannot have two codes"
            );
        }
    }

    // Continuity: codes must be 0..n without gaps
    let mut table = Vec::with_capacity(by_code.len());
    for (expected, (&code, &cp)) in by_code.iter().enumerate() {
        assert_eq!(
            code as usize, expected,
            "{path}: gap in code column — expected {expected}, found {code}. \
             Codes must be sequential from 0."
        );
        table.push(cp);
    }
    assert!(!table.is_empty(), "{path}: base table is empty");
    table
}

/// Parses modifiers: `base_code, modifier_code, result_codepoint`.
fn parse_modifiers(path: &str, base: &[u32]) -> Vec<(u8, u8, u32)> {
    let valid: BTreeMap<u8, &str> = MODIFIER_NAMES.iter().map(|(c, n, _)| (*c, *n)).collect();
    let base_set: BTreeSet<u32> = base.iter().copied().collect();
    let mut rows = Vec::new();
    let mut seen_result: BTreeMap<u32, usize> = BTreeMap::new();
    let mut seen_pair: BTreeSet<(u8, u8)> = BTreeSet::new();

    for (ln, line) in lines_of(path) {
        let Some(body) = clean(&line) else { continue };
        let parts: Vec<&str> = body.split(',').collect();
        assert!(
            parts.len() >= 3,
            "{path}:{ln}: expected 'base_code,modifier_code,result', found '{body}'"
        );
        let ctx = format!("{path}:{ln}");
        let base_code = dec(parts[0], &ctx);
        let mod_code = dec(parts[1], &ctx);
        let result = hex(parts[2], &ctx);

        assert!(
            valid.contains_key(&(mod_code as u8)),
            "{ctx}: {mod_code} is not a valid diacritic modifier code (expected 238..=251)"
        );
        assert!(
            (base_code as usize) < base.len(),
            "{ctx}: base code {base_code} exceeds base table capacity (0..{})",
            base.len() - 1
        );
        assert!(
            !base_set.contains(&result),
            "{ctx}: result 0x{result:04X} is already in the base table as a standalone code"
        );
        if let Some(prev) = seen_result.insert(result, ln) {
            panic!("{ctx}: result 0x{result:04X} was already defined on line {prev}");
        }
        if !seen_pair.insert((base_code as u8, mod_code as u8)) {
            panic!("{ctx}: pair (base {base_code}, modifier {mod_code}) is already defined");
        }
        rows.push((mod_code as u8, base_code as u8, result));
    }
    assert!(!rows.is_empty(), "{path}: modifier table is empty");
    rows
}

/// Parses capitalization exceptions: `lower_codepoint, upper_codepoint`.
fn parse_capitalize(path: &str, base: &[u32], modifiers: &[(u8, u8, u32)]) -> Vec<(u32, u32)> {
    let mut reachable: BTreeSet<u32> = base.iter().copied().collect();
    reachable.extend(modifiers.iter().map(|(_, _, r)| *r));

    let mut rows = Vec::new();
    let mut seen_lower: BTreeMap<u32, usize> = BTreeMap::new();

    for (ln, line) in lines_of(path) {
        let Some(body) = clean(&line) else { continue };
        let parts: Vec<&str> = body.split(',').collect();
        assert!(
            parts.len() >= 2,
            "{path}:{ln}: expected 'lower,upper', found '{body}'"
        );
        let ctx = format!("{path}:{ln}");
        let lower = hex(parts[0], &ctx);
        let upper = hex(parts[1], &ctx);

        assert_ne!(
            lower, upper,
            "{ctx}: lowercase and uppercase are identical (0x{lower:04X})"
        );
        assert!(
            reachable.contains(&lower),
            "{ctx}: lowercase 0x{lower:04X} is unreachable (neither in base table nor modifier results)"
        );
        if let Some(prev) = seen_lower.insert(lower, ln) {
            panic!("{ctx}: lowercase 0x{lower:04X} was already defined on line {prev}");
        }
        rows.push((lower, upper));
    }
    assert!(!rows.is_empty(), "{path}: capitalization table is empty");
    rows
}

// -------------------------------------------------------------- Code Generation

fn emit(f: &mut File, base: &[u32], modifiers: &[(u8, u8, u32)], capitalize: &[(u32, u32)]) {
    let w = |f: &mut File, s: String| f.write_all(s.as_bytes()).expect("writing tables.rs");

    w(
        f,
        "// GENERATED BY build.rs from tables/*.csv — DO NOT EDIT MANUALLY.\n".into(),
    );
    w(
        f,
        "// Make edits in tables/*.csv and run `cargo build`.\n\n".into(),
    );

    w(
        f,
        "/// Diacritic modifier enum — Section 8.3 of the specification.\n".into(),
    );
    w(
        f,
        "/// Discriminants match control byte codes directly.\n".into(),
    );
    w(
        f,
        "#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]\n".into(),
    );
    w(f, "#[repr(u8)]\npub enum Modifier {\n".into());
    for (code, name, doc) in MODIFIER_NAMES {
        w(f, format!("    /// {doc}\n"));
        w(f, format!("    {name} = {code},\n"));
    }
    w(f, "}\n\n".into());

    w(f, "impl Modifier {\n".into());
    w(
        f,
        "    /// All variants in ascending code order for exhaustive iteration.\n".into(),
    );
    w(
        f,
        format!(
            "    pub const ALL: [Modifier; {}] = [\n",
            MODIFIER_NAMES.len()
        ),
    );
    for (_, name, _) in MODIFIER_NAMES {
        w(f, format!("        Modifier::{name},\n"));
    }
    w(f, "    ];\n\n".into());
    w(
        f,
        "    /// Reconstructs a modifier enum from its control byte code.\n".into(),
    );
    w(
        f,
        "    /// Returns `None` for bytes outside 238..=251.\n".into(),
    );
    w(
        f,
        "    #[must_use]\n    pub const fn from_byte(b: u8) -> Option<Modifier> {\n".into(),
    );
    w(f, "        match b {\n".into());
    for (code, name, _) in MODIFIER_NAMES {
        w(
            f,
            format!("            {code} => Some(Modifier::{name}),\n"),
        );
    }
    w(f, "            _ => None,\n        }\n    }\n}\n\n".into());

    // Base table
    w(
        f,
        "/// Base character lookup table: index is CL-8 code, value is Unicode codepoint.\n".into(),
    );
    w(
        f,
        format!("pub const BASE_TABLE: [u32; {}] = [\n", base.len()),
    );
    for chunk in base.chunks(8) {
        let row: Vec<String> = chunk.iter().map(|c| format!("0x{c:05X}")).collect();
        w(f, format!("    {},\n", row.join(", ")));
    }
    w(f, "];\n\n".into());

    w(
        f,
        "/// Modifier lookup table: `(modifier, base_code, result_codepoint)`.\n".into(),
    );
    w(f, "/// Sorted by modifier, then by base code.\n".into());
    w(
        f,
        format!(
            "pub const MODIFIER_TABLE: [(Modifier, u8, u32); {}] = [\n",
            modifiers.len()
        ),
    );
    let names: BTreeMap<u8, &str> = MODIFIER_NAMES.iter().map(|(c, n, _)| (*c, *n)).collect();
    let mut sorted = modifiers.to_vec();
    sorted.sort_by_key(|(m, b, _)| (*m, *b));
    for (m, b, r) in &sorted {
        w(
            f,
            format!("    (Modifier::{}, {b}, 0x{r:04X}),\n", names[m]),
        );
    }
    w(f, "];\n\n".into());

    w(
        f,
        "/// Capitalization exceptions: `(lower_codepoint, upper_codepoint)`.\n".into(),
    );
    w(
        f,
        "/// Sorted by lowercase codepoint (binary search compatible).\n".into(),
    );
    w(
        f,
        format!(
            "pub const CAPITALIZE_TABLE: [(u32, u32); {}] = [\n",
            capitalize.len()
        ),
    );
    let mut cap = capitalize.to_vec();
    cap.sort_by_key(|(l, _)| *l);
    for (l, u) in &cap {
        w(f, format!("    (0x{l:04X}, 0x{u:04X}),\n"));
    }
    w(f, "];\n\n".into());

    w(
        f,
        "/// Reverse capitalization table: `(upper_codepoint, lower_codepoint)`.\n".into(),
    );
    w(
        f,
        "/// Sorted by uppercase codepoint (binary search compatible).\n".into(),
    );
    w(
        f,
        format!(
            "pub const REVERSE_CAPITALIZE_TABLE: [(u32, u32); {}] = [\n",
            capitalize.len()
        ),
    );
    let mut rev_cap: Vec<(u32, u32)> = capitalize.iter().map(|&(l, u)| (u, l)).collect();
    rev_cap.sort_by_key(|(u, _)| *u);
    for (u, l) in &rev_cap {
        w(f, format!("    (0x{u:04X}, 0x{l:04X}),\n"));
    }
    w(f, "];\n\n".into());

    w(f, "/// Total assigned codes in the base table.\n".into());
    w(
        f,
        format!("pub const BASE_TABLE_LEN: usize = {};\n", base.len()),
    );
    w(
        f,
        "/// Total modifier pairs in the modifier table.\n".into(),
    );
    w(
        f,
        format!(
            "pub const MODIFIER_TABLE_LEN: usize = {};\n",
            modifiers.len()
        ),
    );
    w(f, "/// Total capitalization exception entries.\n".into());
    w(
        f,
        format!(
            "pub const CAPITALIZE_TABLE_LEN: usize = {};\n",
            capitalize.len()
        ),
    );
    w(
        f,
        "/// Total entries in the reverse capitalization table.\n".into(),
    );
    w(
        f,
        format!(
            "pub const REVERSE_CAPITALIZE_TABLE_LEN: usize = {};\n",
            capitalize.len()
        ),
    );
}
