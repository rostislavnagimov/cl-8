use std::{fs, time::Instant};
    use std::collections::BTreeMap;

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



// --- Какие именно символы ушли в fallback ---
let mut fallback_chars: BTreeMap<char, usize> = BTreeMap::new();
// --- Какие именно символы закодированы через модификатор ---
let mut modifier_chars: BTreeMap<char, usize> = BTreeMap::new();

for ch in text.chars() {
    if cl8::unicode_fallback::requires_fallback(ch) {
        *fallback_chars.entry(ch).or_insert(0) += 1;
    } else if cl8::tables::find_modifier_pair_for_codepoint(ch as u32).is_some() {
        *modifier_chars.entry(ch).or_insert(0) += 1;
    }
}

println!("\n=== ушло в fallback (топ-50) ===");
let mut fb: Vec<_> = fallback_chars.into_iter().collect();
fb.sort_by(|a, b| b.1.cmp(&a.1));
for (ch, count) in fb.iter().take(50) {
    println!("{ch:?} (U+{:04X}) — {count} раз", *ch as u32);
}

println!("\n=== закодировано через модификатор (топ-50) ===");
let mut md: Vec<_> = modifier_chars.into_iter().collect();
md.sort_by(|a, b| b.1.cmp(&a.1));
for (ch, count) in md.iter().take(50) {
    println!("{ch:?} (U+{:04X}) — {count} раз", *ch as u32);
}
}