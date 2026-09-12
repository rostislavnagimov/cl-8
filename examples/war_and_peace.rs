use cl8::consts::{is_unicode_terminator, UNI_ON};
use std::collections::BTreeMap;
use std::{fs, time::Instant};

fn main() {
    let input_path = "./examples/WarAndPeace.txt";
    let normalized_path = "./examples/WarAndPeace.normalized.txt";
    let encoded_path = "./examples/WarAndPeace.cl8";

    let text_raw = fs::read_to_string(input_path)
        .unwrap_or_else(|e| panic!("не смог прочитать {input_path}: {e}"));
    println!("original {} bytes", text_raw.len());

    let text: String = text_raw
        .chars()
        .filter(|c| unicode_normalization::char::canonical_combining_class(*c) == 0)
        .collect();
    println!("normalized {} bytes", text.len());

    fs::write(normalized_path, &text)
        .unwrap_or_else(|e| panic!("не смог записать {normalized_path}: {e}"));

    let t0 = Instant::now();
    let encoded = cl8::encode(&text).expect("encode failed");
    let encode_time = t0.elapsed();
    println!("encoded {} bytes in {:?}", encoded.len(), encode_time);

    let savings = 100.0 * (1.0 - encoded.len() as f64 / text.len() as f64);
    println!("savings: {:.1}%", savings);

    fs::write(encoded_path, &encoded)
        .unwrap_or_else(|e| panic!("не смог записать {encoded_path}: {e}"));

    let t1 = Instant::now();
    let decoded = cl8::decode(&encoded).expect("decode failed");
    let decode_time = t1.elapsed();
    println!("decoded {} bytes in {:?}", decoded.len(), decode_time);
    println!("check {}", text == decoded);

    println!("\nsaved:");
    println!("  {normalized_path}");
    println!("  {encoded_path}");

    let has_modifier = encoded.iter().any(|&b| (238..=251).contains(&b));
    let has_unicode_fallback = encoded.contains(&236);

    println!("modifiers used: {has_modifier}");
    println!("unicode fallback used: {has_unicode_fallback}");

    let mut fallback_blocks = 0usize;
    let mut fallback_content: Vec<u8> = Vec::new();
    let mut in_fallback = false;

    for &b in encoded.iter() {
        if in_fallback {
            if is_unicode_terminator(b) {
                in_fallback = false;
            } else {
                fallback_content.push(b);
            }
        } else if b == UNI_ON {
            in_fallback = true;
            fallback_blocks += 1;
        }
    }

    if has_unicode_fallback {
        let text = String::from_utf8(fallback_content).expect("фоллбэк — валидный UTF-8");

        let mut counts: BTreeMap<char, usize> = BTreeMap::new();

        println!("\nunicode fallback used for");
        for ch in text.chars() {
            *counts.entry(ch).or_insert(0) += 1;
        }
        for (ch, n) in &counts {
            println!("U+{:04X} {:?} — {} раз", *ch as u32, ch, n);
        }
    }
}
