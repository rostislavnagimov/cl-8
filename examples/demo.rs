//! CL-8 demonstration: encoding, decoding and compression across the supported languages.
//!
//! Every case is a real round-trip: the text is encoded, decoded back, and compared to the
//! original. The exit code is non-zero if any case fails, so this example doubles as a
//! smoke test.
//!
//! Each label lists only the special characters actually present in that sample, and the
//! output marks which cases used the `CAPS_ON`/`CAPS_OFF` toggle (Section 9.1) or the
//! Unicode fallback (Section 10).
//!
//! Run with: `cargo run --example demo`

use std::process::ExitCode;
use std::str;

use cl8::consts::{CAPS_ON, UNI_ON};
use cl8::{decode_into, encode_into, max_decoded_len, max_encoded_len};

/// A group of samples shown under one heading.
struct Section {
    title: &'static str,
    note: &'static str,
    samples: &'static [(&'static str, &'static str)],
}

/// Outcome of a single demonstration case.
struct Outcome {
    ok: bool,
    utf8_len: usize,
    cl8_len: usize,
}

const SECTIONS: &[Section] = &[
    Section {
        title: "Cyrillic — the case CL-8 is built for",
        note: "1 byte per letter where UTF-8 spends 2, uppercase included",
        samples: &[
            ("Привет мир", "Russian (basic Cyrillic)"),
            (
                "Это тестовый текст для проверки эффективности сжатия.",
                "Russian (longer prose)",
            ),
            ("Съешь ещё этих мягких булок", "Russian (pangram fragment, ё)"),
            ("Ёлка ёлка", "Russian (Ё/ё — capitalization exception)"),
            ("ЙоЖмАк", "Russian (alternating case)"),
            ("ПРИВЕТ МОСКВА", "Russian (all caps, direct 1-byte)"),
            (
                "АБВГДЕЁЖЗИЙКЛМНОПРСТУФХЦЧШЩЪЫЬЭЮЯ",
                "Russian (full uppercase alphabet)",
            ),
        ],
    },
    Section {
        title: "Cyrillic beyond Russian",
        note: "supplementary letters occupy single-byte codes 69-90",
        samples: &[
            ("Київ — столиця України", "Ukrainian (ї, em dash)"),
            ("ґанок, єдність, місто", "Ukrainian (ґ, є, і)"),
            ("Беларусь: ў, і", "Belarusian (ў via breve, і)"),
            ("Болгария столица", "Bulgarian (Cyrillic)"),
            (
                "Қазақ әліпбиі: ғ, қ, ң, ө, ү, ұ, һ",
                "Kazakh (8 extra letters)",
            ),
            ("Кыргыз тили: ң, ө, ү", "Kyrgyz (ң, ө, ү)"),
            ("Ўзбекистон: ў, қ, ғ, ҳ", "Uzbek Cyrillic (ў, қ, ғ, ҳ)"),
            ("Тоҷикистон: ҷ, ҳ, қ, ғ, ӣ, ӯ", "Tajik (ҷ, ҳ, ӣ, ӯ)"),
            ("Монгол хэл, Улаанбаатар", "Mongolian (Cyrillic)"),
            (
                "Србија: ј, љ, њ, ћ, ђ, џ",
                "Serbian Cyrillic (6 extra letters)",
            ),
            ("Македонија: ѓ, ќ, ѕ", "Macedonian (ѓ, ќ, ѕ)"),
        ],
    },
    Section {
        title: "Latin with diacritics",
        note: "each composite is 2 bytes: modifier + base — parity with UTF-8, not a win",
        samples: &[
            ("Straße Käse", "German (ß, ä)"),
            ("Grüße aus München", "German (ü)"),
            ("Übergröße", "German (Ü uppercase, ö, ß)"),
            ("café élève à Paris", "French (é, è, à)"),
            ("naïve forêt Noël", "French (ï, ê, ë)"),
            ("El niño pingüino comió", "Spanish (ñ, ü, ó)"),
            ("São Paulo, coração, ação", "Portuguese (ã, ç)"),
            ("città perché così", "Italian (à, é, ì)"),
            ("'n môre sonder reën", "Afrikaans (ô, ë)"),
            ("voorbereiding op reis", "Dutch (ASCII only)"),
            ("đačka škola, čaša, žir", "Croatian (đ, š, č, ž)"),
            ("Bosna i Hercegovina", "Bosnian Latin (ASCII only)"),
            ("Srbija: š, đ, č, ć, ž", "Serbian Latin (5 diacritics)"),
            ("slovenščina: č, š, ž", "Slovenian (č, š, ž)"),
            ("Dzień dobry", "Polish (ń)"),
            (
                "żółć gęślą jaźń",
                "Polish (pangram: ż, ó, ł, ć, ę, ś, ą, ź, ń)",
            ),
            ("čeština: č, ě, š, ř, ž, ď, ť, ů", "Czech (8 diacritics)"),
            ("slovenčina: ľ, ň, ď, ť, ŕ", "Slovak (ľ, ň, ď, ť, ŕ)"),
            ("hűvös nyelv, ő és ű", "Hungarian (ű, ö, ő)"),
            ("română: ă, â, î, ș, ț", "Romanian (comma-below ș/ț)"),
            (
                "română şi turcă: ş vs ș",
                "Cedilla vs comma-below (ş U+015F, ș U+0219)",
            ),
            ("gözlük, nağme, çay, ısı", "Turkish (ö, ü, ğ, ç, dotless ı)"),
            ("İstanbul güzel", "Turkish (İ — the 3-byte special case)"),
            ("grönt blad, äpple, år", "Swedish (ö, ä, å)"),
            ("blåbær på fjøre", "Norwegian (å, æ, ø)"),
            ("æblegrød på fad", "Danish (æ, ø, å)"),
            ("þröstur að æfa", "Icelandic (þ, ö, ð, æ)"),
            ("tjóð og føroyskt", "Faroese (ó, ð, ø)"),
            ("bahasa Indonesia sehari-hari", "Indonesian (ASCII only)"),
            ("bahasa Melayu di Malaysia", "Malay (ASCII only)"),
            ("jambo, habari za dunia", "Swahili (ASCII only)"),
            ("euskara oso zaila da", "Basque (ASCII only)"),
        ],
    },
    Section {
        title: "Uppercase economics (Section 9.1)",
        note: "Latin A-Z and Cyrillic А-Я are direct codes and need no toggle; \
               letters without a direct code are where the toggle pays off",
        samples: &[
            ("A", "Single Latin capital (direct, 1 byte)"),
            ("ABC", "Latin run (direct, no toggle needed)"),
            ("API JSON HTTP", "Acronyms (direct 1-byte uppercase)"),
            (
                "SCREAMING_SNAKE_CASE constant",
                "Uppercase identifier (direct 1-byte uppercase)",
            ),
            ("ЄЂ", "Case-shift run of 2 (prefix is not worse — no toggle)"),
            ("ЄЇЂЅ", "Case-shift run of 4 (toggle wins)"),
            ("ČŠŽ", "Uppercase diacritics (toggle wins over prefixes)"),
            (
                "ŻÓŁĆ GĘŚLĄ JAŹŃ",
                "Polish all-caps (one toggle spans spaces and direct capitals)",
            ),
        ],
    },
    Section {
        title: "Punctuation, symbols, currencies",
        note: "single-byte codes 97-160",
        samples: &[
            ("C++ && Python", "Operators and symbols"),
            ("100% done", "Digits and percent"),
            ("Email: user@example.com", "Email address"),
            ("Price: 100€", "Currency (€ — 3 UTF-8 bytes, 1 in CL-8)"),
            ("Cost: 50£ 100¥", "Currencies (£, ¥)"),
            (
                "\"straight\" vs «typographic»",
                "Quote styles are distinct codes",
            ),
            ("line1\nline2\ttab", "Whitespace control bytes (newline, tab)"),
            ("a b c", "Spaces"),
        ],
    },
    Section {
        title: "Unicode fallback (Section 10)",
        note: "anything outside the tables streams as raw UTF-8 after UNI_ON (236)",
        samples: &[
            ("Привет 🚀 мир", "Emoji inside Cyrillic"),
            ("👨‍👩‍👧‍👦 family", "ZWJ emoji sequence"),
            ("日本語と中文", "CJK"),
            ("مرحبا بالعالم", "Arabic (right-to-left)"),
        ],
    },
    Section {
        title: "Mixed real-world payloads",
        note: "how the mechanisms combine in practice",
        samples: &[
            ("Hello Привет мир", "English + Russian"),
            ("API ключ", "Acronym + Russian"),
            ("Bonjour café", "English + French"),
        ],
    },
];

/// Encodes, decodes and reports one sample, returning whether the round-trip was exact.
fn demo(text: &str, description: &str) -> Outcome {
    println!("\n--- {description} ---");
    println!("Original        : \"{}\"", text.escape_debug());
    println!(
        "UTF-8 length    : {} bytes ({} chars)",
        text.len(),
        text.chars().count()
    );

    let mut encoded = vec![0u8; max_encoded_len(text.len())];
    let encoded_size = match encode_into(text, &mut encoded) {
        Ok(n) => n,
        Err(e) => {
            println!("Encoding error  : {e}");
            println!("Round-trip      : FAILED");
            return Outcome {
                ok: false,
                utf8_len: text.len(),
                cl8_len: 0,
            };
        }
    };
    let encoded_bytes = &encoded[..encoded_size];

    println!("CL-8 bytes      : {encoded_bytes:?}");
    println!("CL-8 length     : {encoded_size} bytes");

    // Point out which optional mechanisms this sample actually exercised.
    let mut notes = Vec::new();
    if encoded_bytes.contains(&CAPS_ON) {
        notes.push("CAPS-toggle run");
    }
    if encoded_bytes.contains(&UNI_ON) {
        notes.push("Unicode fallback");
    }
    if !notes.is_empty() {
        println!("Mechanisms      : {}", notes.join(", "));
    }

    #[allow(clippy::cast_precision_loss)]
    let savings = (1.0 - (encoded_size as f64) / (text.len() as f64)) * 100.0;
    println!("Savings vs UTF-8: {savings:+.1}%");

    let mut decoded = vec![0u8; max_decoded_len(encoded_size)];
    let decoded_size = match decode_into(encoded_bytes, &mut decoded) {
        Ok(n) => n,
        Err(e) => {
            println!("Decoding error  : {e}");
            println!("Round-trip      : FAILED");
            return Outcome {
                ok: false,
                utf8_len: text.len(),
                cl8_len: encoded_size,
            };
        }
    };

    let ok = match str::from_utf8(&decoded[..decoded_size]) {
        Ok(decoded_str) => {
            let matches = decoded_str == text;
            println!("Decoded text    : \"{}\"", decoded_str.escape_debug());
            println!(
                "Round-trip      : {}",
                if matches { "OK" } else { "MISMATCH" }
            );
            if !matches {
                println!("  expected: {:?}", text.chars().collect::<Vec<_>>());
                println!("  actual  : {:?}", decoded_str.chars().collect::<Vec<_>>());
            }
            matches
        }
        Err(e) => {
            println!("Decoder produced invalid UTF-8: {e}");
            println!("Round-trip      : FAILED");
            false
        }
    };

    Outcome {
        ok,
        utf8_len: text.len(),
        cl8_len: encoded_size,
    }
}

fn main() -> ExitCode {
    println!("CL-8 encoding and decoding demonstration");
    println!("========================================");
    println!();
    println!("Single-byte alphabets : digits, Latin a-z/A-Z, Cyrillic а-я/А-Я/Ё");
    println!("Two-byte composites   : 14 diacritic modifiers (codes 238-251)");
    println!("Fallback              : anything else streams as UTF-8 after UNI_ON (236)");
    println!();
    println!("Languages below cover the 38 named in the specification's language list.");
    println!("(Its section header says 41 while naming 38 — a known discrepancy,");
    println!("recorded in tests/corpus/MANIFEST.tsv.)");

    let outcomes: Vec<Outcome> = SECTIONS
        .iter()
        .flat_map(|section| {
            println!("\n\n########## {} ##########", section.title);
            println!("({})", section.note);
            section
                .samples
                .iter()
                .map(|(text, label)| demo(text, label))
                .collect::<Vec<_>>()
        })
        .collect();

    let total = outcomes.len();
    let failed = outcomes.iter().filter(|c| !c.ok).count();
    let utf8_total: usize = outcomes.iter().map(|c| c.utf8_len).sum();
    let cl8_total: usize = outcomes.iter().map(|c| c.cl8_len).sum();
    #[allow(clippy::cast_precision_loss)]
    let overall = (1.0 - (cl8_total as f64) / (utf8_total as f64)) * 100.0;

    println!("\n========================================");
    println!("Cases             : {total}");
    println!("Exact round-trips : {}", total - failed);
    println!("Failures          : {failed}");
    println!("Total UTF-8       : {utf8_total} bytes");
    println!("Total CL-8        : {cl8_total} bytes");
    println!("Overall savings   : {overall:+.1}%");

    if failed == 0 {
        println!("\nAll cases round-tripped exactly.");
        ExitCode::SUCCESS
    } else {
        println!("\n{failed} case(s) FAILED — see the entries marked above.");
        ExitCode::FAILURE
    }
}
