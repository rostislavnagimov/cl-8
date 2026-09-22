//! Замеры для CHANGELOG и README: сжатие, скорость, стоимость вызова.
//!
//! Запуск: `cargo run --release --example metrics --features std`
//!
//! # Почему не criterion
//!
//! Criterion считает среднее и борется с шумом статистикой: на macOS процесс
//! мигрирует между P- и E-ядрами, отсюда 5–24% выбросов и восемь минут прогона.
//! Здесь берётся минимум из N прогонов — «сколько стоит работа, когда ничто не
//! мешало». Для детерминированного CPU-bound кода без I/O и аллокаций минимум
//! устойчив по построению: пять независимых серий сходятся с точностью 0.1%,
//! а весь прогон занимает около секунды.

use std::time::Instant;

/// Длина входа для замеров пропускной способности.
///
/// При ~5 нс/символ это ~80 мкс на прогон — фиксированная стоимость вызова
/// (десятки наносекунд) становится пренебрежимой.
const BENCH_CHARS: usize = 16_384;

/// Прогонов в замере. На 1000 разброс между сериями падает до 0.1%.
const REPS: usize = 1_000;

/// Прогревочных итераций перед замером.
const WARMUP: usize = 50;

// ------------------------------------------------------------------ хронометраж

/// Минимальное время одного прогона `f` в наносекундах.
fn min_ns<F: FnMut() -> usize>(f: F) -> f64 {
    min_ns_inner(1, f)
}

/// То же, но `inner` вызовов внутри одного замера.
///
/// `Instant` на macOS квантуется примерно по 41 нс: без внутреннего цикла
/// вход в несколько символов измеряется как ноль. Амортизация по `inner`
/// вызовам уводит замер выше порога разрешения.
fn min_ns_inner<F: FnMut() -> usize>(inner: usize, mut f: F) -> f64 {
    for _ in 0..WARMUP {
        std::hint::black_box(f());
    }
    let mut best = f64::MAX;
    for _ in 0..REPS {
        let t = Instant::now();
        for _ in 0..inner {
            std::hint::black_box(f());
        }
        let ns = t.elapsed().as_nanos() as f64 / inner as f64;
        if ns < best {
            best = ns;
        }
    }
    best
}

/// Наносекунды на символ при кодировании.
fn encode_ns_per_char(text: &str) -> f64 {
    let mut out = vec![0u8; cl8::max_encoded_len(text.len())];
    let chars = text.chars().count() as f64;
    min_ns(|| cl8::encode_into(text, &mut out).expect("кодирование")) / chars
}

/// Наносекунды на символ при декодировании.
fn decode_ns_per_char(text: &str) -> f64 {
    let mut enc = vec![0u8; cl8::max_encoded_len(text.len())];
    let n = cl8::encode_into(text, &mut enc).expect("кодирование");
    let enc = &enc[..n];
    let mut out = vec![0u8; cl8::max_decoded_len(n)];
    let chars = text.chars().count() as f64;
    min_ns(|| cl8::decode_into(enc, &mut out).expect("декодирование")) / chars
}

/// Повторяет фрагмент до нужного числа символов.
fn grow(fragment: &str, chars: usize) -> String {
    let per = fragment.chars().count();
    fragment.repeat(chars / per + 1)
}

// ------------------------------------------------------------------ данные

/// Профили реального текста: имя и фрагмент.
const PROFILES: &[(&str, &str)] = &[
    (
        "ru_cyrillic",
        "Съешь ещё этих мягких французских булок да выпей чаю. \
         Кодировка CL-8 хранит кириллицу одним байтом на букву. ",
    ),
    (
        "ru_uppercase",
        "ВНИМАНИЕ: МОСКВА, САНКТ-ПЕТЕРБУРГ, НОВОСИБИРСК, ЕКАТЕРИНБУРГ. ",
    ),
    (
        "en_ascii",
        "The quick brown fox jumps over the lazy dog. \
         Zero allocations, no_std core, single byte per letter. ",
    ),
    (
        "european_diacritics",
        "Příliš žluťoučký kůň úpěl ďábelské ódy. \
         Zażółć gęślą jaźń. München, café, élève, naïve. ",
    ),
    (
        "mixed_chat",
        "User @alex: Привет! How are you? Meeting at 15:30 (Room 402). Price: 250€. ",
    ),
    (
        "unicode_fallback_cjk",
        "日本語のテキスト、中国文字和韩文测试内容。 Текст за пределами базовой таблицы. ",
    ),
];

/// Чистые классы символов: показывают разброс каскада энкодера.
const CLASSES: &[(&str, &str)] = &[
    ("ASCII строчные", "abcdefghijklmnopqrstuvwxyz"),
    ("ASCII ЗАГЛАВНЫЕ", "ABCDEFGHIJKLMNOPQRSTUVWXYZ"),
    ("Цифры", "0123456789"),
    ("ASCII пунктуация", ".,!?;:()[]{}+-*/=<>@#$%&"),
    ("Кириллица строчная", "абвгдежзийклмнопрстуфхцчшщъыьэюя"),
    ("Кириллица ЗАГЛАВНАЯ", "АБВГДЕЖЗИЙКЛМНОПРСТУФХЦЧШЩЪЫЬЭЮЯ"),
    ("Доп. кириллица строчная", "єїђѕјљњћџіґ"),
    ("Доп. кириллица ЗАГЛАВНАЯ", "ЄЇЂЅЈЉЊЋЏІҐ"),
    ("Диакритика строчная", "áéíóúýčďěňřšťžůàèìòùâêîôûäëïöüãõñåøçł"),
    ("Диакритика ЗАГЛАВНАЯ", "ÁÉÍÓÚÝČĎĚŇŘŠŤŽŮÀÈÌÒÙÂÊÎÔÛÄËÏÖÜÃÕÑÅØÇŁ"),
    ("CJK (фоллбэк)", "日本語漢字中国文字韓"),
    ("Эмодзи (фоллбэк)", "🚀🔥👍😀🎉"),
];

/// Длины для замера фиксированной стоимости вызова.
const SWEEP: &[usize] = &[1, 4, 16, 64, 256, 1024, 4096, 16384];

// ------------------------------------------------------------------ отчёт

fn profiles_table() {
    println!("## Профили\n");
    println!(
        "| Профиль | Б/симв UTF-8 | Б/симв CL-8 | Экономия | enc нс/симв | dec нс/симв |"
    );
    println!("|---|---:|---:|---:|---:|---:|");
    for (name, fragment) in PROFILES {
        let text = grow(fragment, BENCH_CHARS);
        let chars = text.chars().count() as f64;
        let mut enc = vec![0u8; cl8::max_encoded_len(text.len())];
        let n = cl8::encode_into(&text, &mut enc).expect("кодирование");
        let utf8_per = text.len() as f64 / chars;
        let cl8_per = n as f64 / chars;
        println!(
            "| {} | {:.2} | {:.2} | {:+.1}% | {:.2} | {:.2} |",
            name,
            utf8_per,
            cl8_per,
            100.0 - cl8_per / utf8_per * 100.0,
            encode_ns_per_char(&text),
            decode_ns_per_char(&text),
        );
    }
    println!();
}

fn classes_table() {
    println!("## Чистые классы символов\n");
    println!("| Класс | Б/симв CL-8 | enc нс/симв | dec нс/симв |");
    println!("|---|---:|---:|---:|");
    for (name, fragment) in CLASSES {
        let text = grow(fragment, BENCH_CHARS);
        let chars = text.chars().count() as f64;
        let mut enc = vec![0u8; cl8::max_encoded_len(text.len())];
        let n = cl8::encode_into(&text, &mut enc).expect("кодирование");
        println!(
            "| {} | {:.2} | {:.2} | {:.2} |",
            name,
            n as f64 / chars,
            encode_ns_per_char(&text),
            decode_ns_per_char(&text),
        );
    }
    println!();
}

/// Раскладывает время на фиксированную часть вызова и стоимость символа.
///
/// Точки расположены логарифмически, поэтому наименьшие квадраты подмял бы
/// самый большой замер и дал бы бессмысленный отрицательный свободный член.
/// Наклон берётся по двум крайним большим точкам, фикс — по самой малой.
fn fit(points: &[(f64, f64)]) -> (f64, f64) {
    let (x1, y1) = points[points.len() - 2];
    let (x2, y2) = points[points.len() - 1];
    let slope = (y2 - y1) / (x2 - x1);
    let (xs, ys) = points[0];
    ((ys - slope * xs).max(0.0), slope)
}

fn overhead_table() {
    println!("## Стоимость вызова на коротких входах\n");
    println!("| Символов | encode нс | decode нс |");
    println!("|---:|---:|---:|");
    let fragment = PROFILES[0].1;
    let mut enc_pts = Vec::new();
    let mut dec_pts = Vec::new();
    for &len in SWEEP {
        let text: String = grow(fragment, len).chars().take(len).collect();
        // Каждый замер должен длиться хотя бы несколько микросекунд.
        let inner = (4096 / len).max(1);
        let mut enc = vec![0u8; cl8::max_encoded_len(text.len())];
        let e = min_ns_inner(inner, || {
            cl8::encode_into(&text, &mut enc).expect("кодирование")
        });
        let n = cl8::encode_into(&text, &mut enc).expect("кодирование");
        let encoded = enc[..n].to_vec();
        let mut out = vec![0u8; cl8::max_decoded_len(n)];
        let d = min_ns_inner(inner, || {
            cl8::decode_into(&encoded, &mut out).expect("декодирование")
        });
        println!("| {len} | {e:.1} | {d:.1} |");
        enc_pts.push((len as f64, e));
        dec_pts.push((len as f64, d));
    }
    let (e0, es) = fit(&enc_pts);
    let (d0, ds) = fit(&dec_pts);
    println!("\nencode: {e0:.0} нс на вызов + {es:.2} нс/символ");
    println!("decode: {d0:.0} нс на вызов + {ds:.2} нс/символ\n");
}

fn large_text() {
    let path = "./examples/WarAndPeace.normalized.txt";
    let Ok(text) = std::fs::read_to_string(path) else {
        println!("## Большой текст\n\n`{path}` отсутствует — пропущено.\n");
        return;
    };
    let chars = text.chars().count();
    let mut enc = vec![0u8; cl8::max_encoded_len(text.len())];
    let n = cl8::encode_into(&text, &mut enc).expect("кодирование");
    let encoded = enc[..n].to_vec();
    let mut dec = vec![0u8; cl8::max_decoded_len(n)];

    // Большой вход: прогонов меньше, иначе замер тянется секундами.
    fn best_of<F: FnMut() -> usize>(mut f: F) -> f64 {
        let mut best = f64::MAX;
        for _ in 0..20 {
            let t = Instant::now();
            let k = f();
            let ns = t.elapsed().as_nanos() as f64;
            std::hint::black_box(k);
            if ns < best {
                best = ns;
            }
        }
        best
    }
    let e = best_of(|| cl8::encode_into(&text, &mut enc).expect("кодирование"));
    let d = best_of(|| cl8::decode_into(&encoded, &mut dec).expect("декодирование"));

    println!("## Большой текст (Война и мир)\n");
    println!("| | |");
    println!("|---|---:|");
    println!("| Символов | {chars} |");
    println!("| UTF-8, байт | {} |", text.len());
    println!("| CL-8, байт | {n} |");
    println!(
        "| Экономия | {:+.1}% |",
        100.0 - n as f64 / text.len() as f64 * 100.0
    );
    println!("| encode | {:.1} мс ({:.0} МБ/с) |", e / 1e6, text.len() as f64 / e * 1e3);
    println!("| decode | {:.1} мс ({:.0} МБ/с) |", d / 1e6, text.len() as f64 / d * 1e3);
    println!(
        "| Блоков UNI_ON | {} |",
        encoded.iter().filter(|&&b| b == cl8::consts::UNI_ON).count()
    );
    println!();
}

fn tables_footprint() {
    use std::mem::size_of_val;
    let base = size_of_val(&cl8::BASE_TABLE);
    let modi = size_of_val(&cl8::MODIFIER_TABLE);
    let caps = size_of_val(&cl8::CAPITALIZE_TABLE);
    println!("## Статические таблицы\n");
    println!("| Таблица | Записей | Байт |");
    println!("|---|---:|---:|");
    println!("| BASE_TABLE | {} | {base} |", cl8::BASE_TABLE.len());
    println!("| MODIFIER_TABLE | {} | {modi} |", cl8::MODIFIER_TABLE.len());
    println!("| CAPITALIZE_TABLE | {} | {caps} |", cl8::CAPITALIZE_TABLE.len());
    println!("| REVERSE_CAPITALIZE_TABLE | {} | {caps} |", cl8::CAPITALIZE_TABLE.len());
    println!("| **Итого** | | **{}** |", base + modi + caps * 2);
    println!();
}

fn main() {
    assert!(
        !cfg!(debug_assertions),
        "метрики имеют смысл только в release: debug медленнее примерно в 10 раз.\n\
         Запускай: cargo run --release --example metrics --features std"
    );

    println!("# CL-8 metrics\n");
    println!("Формат: `{}`\n", cl8::SPEC_VERSION);
    println!(
        "Метод: минимум из {REPS} прогонов на входе {BENCH_CHARS} символов, \
         прогрев {WARMUP}. Сжатие детерминировано и от железа не зависит.\n"
    );

    profiles_table();
    classes_table();
    overhead_table();
    large_text();
    tables_footprint();
}
