//! Criterion performance benchmarks for CL-8 encoding and decoding.
//!
//! Measures encoding and decoding throughput (MB/s / GB/s) and latency across
//! representative multilingual text corpora, as well as lookup table primitives.
//!
//! Run with: `cargo bench`

use std::fs;
use std::path::{Path, PathBuf};

use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};

/// Returns built-in representative test samples covering distinct language characteristics.
fn built_in_samples() -> Vec<(&'static str, &'static str)> {
    vec![
        (
            "en_ascii",
            "The quick brown fox jumps over the lazy dog. \
             CL-8 provides high-performance 8-bit text encoding for mixed Cyrillic and Latin systems. \
             HTTP JSON REST API requests and responses can be streamed with zero allocations. \
             Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore.",
        ),
        (
            "ru_cyrillic",
            "В чащах юга жил-был цитрус? Да, но фальшивый экземпляр! \
             Съешь ещё этих мягких французских булок да выпей чаю. \
             Кодировка CL-8 разработана специально для компактного хранения и потоковой передачи кириллицы. \
             Она позволяет экономить до пятидесяти процентов объема данных по сравнению со стандартным UTF-8.",
        ),
        (
            "ru_uppercase",
            "ВНИМАНИЕ! СРОЧНОЕ СООБЩЕНИЕ И ПРЕДУПРЕЖДЕНИЕ: \
             МОСКВА, САНКТ-ПЕТЕРБУРГ, НОВОСИБИРСК, ЕКАТЕРИНБУРГ, КАЗАНЬ, НИЖНИЙ НОВГОРОД, ЧЕЛЯБИНСК, САМАРА, УФА, РОСТОВ-НА-ДОНУ. \
             ГОСТ СТАНДАРТИЗАЦИЯ И РЕГЛАМЕНТ ТЕХНИЧЕСКИХ ТРЕБОВАНИЙ КРЕЙТА.",
        ),
        (
            "european_diacritics",
            "München ist eine Großstadt in Deutschland mit vielen Sehenswürdigkeiten. \
             Le cœur a ses raisons que la raison ne connaît point — café, élève, naïve, forêt. \
             Zażółć gęślą jaźń — wszystkie polskie znaki diakrytyczne. \
             Příliš žluťoučký kůň úpěl ďábelské ódy. \
             İstanbul Türkiye'nin en kalabalık ve en önemli şehridir.",
        ),
        (
            "mixed_chat_payload",
            "User @alex: Привет! How are you doing today? 🚀 \
             Meeting scheduled at 15:30 (Room 402). Price: 250€ / 300$. \
             Подтвердите получение файла report_final_2026.json 👍🔥.",
        ),
        (
            "unicode_fallback_cjk",
            "日本語のテキスト、中国文字和韩文测试内容。 \
             Содержимое за пределами базовой таблицы CL-8 транслируется через безопасный режим UNI_ON.",
        ),
    ]
}

fn corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/corpus")
}

/// Loads corpus files from `tests/corpus/` if present.
fn load_corpus_files() -> Vec<(String, String)> {
    let dir = corpus_dir();
    let Ok(entries) = fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut out: Vec<(String, String)> = entries
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|x| x == "txt"))
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            let text = fs::read_to_string(e.path()).ok()?;
            let body: String = text
                .lines()
                .filter(|l| !l.starts_with('#'))
                .collect::<Vec<_>>()
                .join("\n");
            (!body.trim().is_empty()).then_some((name, body))
        })
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

// -------------------------------------------------------------- 1. Size Comparison

fn size_comparison_report() {
    println!("\n================== Size & Compression Summary ==================");
    println!(
        "{:<28} {:>10} {:>10} {:>10} {:>10}",
        "Dataset", "UTF-8", "CL-8", "Savings", "Chars"
    );
    println!("{}", "-".repeat(72));

    for (name, text) in built_in_samples() {
        let mut buf = vec![0u8; cl8::max_encoded_len(text.len())];
        if let Ok(n) = cl8::encode_into(text, &mut buf) {
            let saving = 100.0 - (n as f64 / text.len() as f64) * 100.0;
            println!(
                "{:<28} {:>10} {:>10} {:>9.1}% {:>10}",
                name,
                text.len(),
                n,
                saving,
                text.chars().count()
            );
        }
    }
    println!("================================================================\n");
}

// -------------------------------------------------------------- 2. Encoding Benchmarks

fn bench_encode(c: &mut Criterion) {
    size_comparison_report();

    let mut group = c.benchmark_group("encode_throughput");

    for (name, text) in built_in_samples() {
        group.throughput(Throughput::Bytes(text.len() as u64));
        group.bench_function(name, |b| {
            let mut out = vec![0u8; cl8::max_encoded_len(text.len())];
            b.iter(|| {
                let n = cl8::encode_into(black_box(text), black_box(&mut out)).unwrap();
                black_box(n);
            });
        });
    }

    for (name, text) in load_corpus_files() {
        group.throughput(Throughput::Bytes(text.len() as u64));
        group.bench_function(format!("corpus/{name}"), |b| {
            let mut out = vec![0u8; cl8::max_encoded_len(text.len())];
            b.iter(|| {
                let n = cl8::encode_into(black_box(&text), black_box(&mut out)).unwrap();
                black_box(n);
            });
        });
    }

    group.finish();
}

// -------------------------------------------------------------- 3. Decoding Benchmarks

fn bench_decode(c: &mut Criterion) {
    let mut group = c.benchmark_group("decode_throughput");

    for (name, text) in built_in_samples() {
        let mut encoded = vec![0u8; cl8::max_encoded_len(text.len())];
        let n = cl8::encode_into(text, &mut encoded).unwrap();
        let encoded_slice = &encoded[..n];

        group.throughput(Throughput::Bytes(text.len() as u64));
        group.bench_function(name, |b| {
            let mut out = vec![0u8; cl8::max_decoded_len(n)];
            b.iter(|| {
                let m = cl8::decode_into(black_box(encoded_slice), black_box(&mut out)).unwrap();
                black_box(m);
            });
        });
    }

    for (name, text) in load_corpus_files() {
        let mut encoded = vec![0u8; cl8::max_encoded_len(text.len())];
        if let Ok(n) = cl8::encode_into(&text, &mut encoded) {
            let encoded_slice = &encoded[..n];
            group.throughput(Throughput::Bytes(text.len() as u64));
            group.bench_function(format!("corpus/{name}"), |b| {
                let mut out = vec![0u8; cl8::max_decoded_len(n)];
                b.iter(|| {
                    let m =
                        cl8::decode_into(black_box(encoded_slice), black_box(&mut out)).unwrap();
                    black_box(m);
                });
            });
        }
    }

    group.finish();
}

// -------------------------------------------------------------- 4. Table Microbenchmarks

fn bench_tables(c: &mut Criterion) {
    let mut group = c.benchmark_group("table_primitives");

    group.bench_function("codepoint_of_o1", |b| {
        b.iter(|| {
            let mut acc = 0u32;
            for code in 0..=219u8 {
                acc = acc.wrapping_add(cl8::tables::codepoint_of(black_box(code)).unwrap_or(0));
            }
            black_box(acc)
        });
    });

    group.bench_function("code_of_fast_path", |b| {
        b.iter(|| {
            let mut acc = 0u8;
            for cp in 0x0430..=0x044Fu32 {
                acc = acc.wrapping_add(cl8::tables::code_of(black_box(cp)).unwrap_or(0));
            }
            black_box(acc)
        });
    });

    group.bench_function("code_of_linear_search", |b| {
        let last_cp = cl8::BASE_TABLE[150];
        b.iter(|| cl8::tables::code_of(black_box(last_cp)));
    });

    group.bench_function("apply_modifier", |b| {
        b.iter(|| cl8::tables::apply_modifier(black_box(cl8::Modifier::Acute), black_box(14)));
    });

    group.bench_function("to_upper_binary_search", |b| {
        b.iter(|| cl8::tables::to_upper(black_box(0x0451)));
    });

    group.bench_function("from_upper_to_lower_binary_search", |b| {
        b.iter(|| cl8::tables::from_upper_to_lower(black_box(0x0401)));
    });

    group.finish();
}

criterion_group!(benches, bench_encode, bench_decode, bench_tables);
criterion_main!(benches);
