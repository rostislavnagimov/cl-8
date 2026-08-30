use std::{fs, time::Instant};

fn main() {
    let input_path = "./examples/WarAndPeace.txt";
    // let output_path = "./WarAndPeace.cl8";

    let text_raw = fs::read_to_string(input_path)
        .unwrap_or_else(|e| panic!("не смог прочитать {input_path}: {e}"));

    println!("original {} bytes", text_raw.len());

    let text: String = text_raw
        .chars()
        .filter(|c| unicode_normalization::char::canonical_combining_class(*c) == 0)
        .collect();
    println!("normalized {} bytes", text.len());

    let t0 = Instant::now();

    let encoded = cl8::encode(&text).expect("encode failed");

    let encode_time = t0.elapsed();

    println!("encoded {} bytes in {:?}", encoded.len(), encode_time);

    let savings = 100.0 * (1.0 - encoded.len() as f64 / text.len() as f64);
    println!("savings: {:.1}%", savings);

    let t1 = Instant::now();

    let decoded = cl8::decode(&encoded).expect("decode failed");

    let decode_time = t1.elapsed();

    println!("decoded {} bytes in {:?}", decoded.len(), decode_time);
    println!("check {}", text == decoded);
}
