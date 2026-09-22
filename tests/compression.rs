//! Регрессия по степени сжатия.
//!
//! Сжатие детерминировано: одна и та же строка даёт одно и то же число байт на
//! любой машине. Значит это материал для теста, а не для бенчмарка — и
//! единственная защита от «оптимизации», которая молча испортит выход.
//!
//! Байты закреплены точно, а не порогом. Порог пропустил бы регрессию на один
//! байт на символ; точное значение ловит любое изменение формата — включая
//! намеренное, и тогда его надо обновить осознанно, вместе с версией формата.

/// `(имя, текст, байт UTF-8, байт CL-8)`.
const CASES: &[(&str, &str, usize, usize)] = &[
    // Главное обещание: кириллица втрое компактнее не бывает, но вдвое — да.
    (
        "ru_prose",
        "Съешь ещё этих мягких французских булок да выпей чаю.",
        97,
        53,
    ),
    // Заглавная кириллица тоже однобайтовая — без префиксов и переключателей.
    ("ru_upper", "ПРИВЕТ МОСКВА САНКТ-ПЕТЕРБУРГ", 55, 29),
    // ASCII в CL-8 ровно такой же, как в UTF-8: байт в байт, без потерь.
    ("en_ascii", "The quick brown fox jumps over the lazy dog.", 44, 44),
    // Диакритика — два байта, столько же, сколько в UTF-8. Ничья, не проигрыш.
    ("cz_diacritics", "Příliš žluťoučký kůň úpěl ďábelské ódy.", 54, 54),
    // CAPS-ран: два байта переключателей на всю строку вместо префикса на букву.
    ("pl_caps", "ŻÓŁĆ GĘŚLĄ JAŹŃ", 24, 26),
    // Эмодзи проходит фоллбэком, кириллица вокруг — по байту на букву.
    ("mixed", "Привет 🚀 мир", 24, 17),
    // CRLF: с кодом 220 перевод строки Windows стоит байт, а не три.
    ("crlf", "строка\r\nвторая\r\n", 28, 16),
    // Неразрывный пробел получил код 221 — раньше уходил в фоллбэк по 4 байта.
    ("nbsp", "100\u{a0}000\u{a0}₽", 13, 9),
];

#[test]
fn compression_ratios_do_not_regress() {
    for &(name, text, utf8_len, cl8_len) in CASES {
        assert_eq!(text.len(), utf8_len, "{name}: изменился исходный текст теста");

        let mut buf = vec![0u8; cl8::max_encoded_len(text.len())];
        let n = cl8::encode_into(text, &mut buf).unwrap_or_else(|e| panic!("{name}: {e:?}"));
        assert_eq!(
            n, cl8_len,
            "{name}: CL-8 дал {n} байт вместо {cl8_len} — формат изменился"
        );

        let mut out = vec![0u8; cl8::max_decoded_len(n)];
        let m = cl8::decode_into(&buf[..n], &mut out).unwrap_or_else(|e| panic!("{name}: {e:?}"));
        assert_eq!(
            core::str::from_utf8(&out[..m]).expect("UTF-8"),
            text,
            "{name}: round-trip не совпал"
        );
    }
}

#[test]
fn cyrillic_costs_exactly_one_byte_per_letter() {
    // Обещание из README: буква кириллицы — ровно один байт, строчная и заглавная.
    for text in ["абвгдежзийклмнопрстуфхцчшщъыьэюяё", "АБВГДЕЖЗИЙКЛМНОПРСТУФХЦЧШЩЪЫЬЭЮЯЁ"] {
        let mut buf = vec![0u8; cl8::max_encoded_len(text.len())];
        let n = cl8::encode_into(text, &mut buf).expect("кодирование");
        assert_eq!(
            n,
            text.chars().count(),
            "{text}: {n} байт на {} символов — где-то потерялся прямой код",
            text.chars().count()
        );
    }
}

#[test]
fn windows_line_endings_cost_one_byte() {
    // Регрессия на коды 220..=225: до их появления \r уходил в фоллбэк и стоил
    // три байта вместо одного — по два байта рамки на каждую строку.
    for (ch, name) in [
        ('\r', "CR"),
        ('\u{a0}', "nbsp"),
        ('\u{202f}', "узкий nbsp"),
        ('\u{ad}', "мягкий перенос"),
        ('×', "знак умножения"),
        ('−', "минус"),
    ] {
        let s = ch.to_string();
        let mut buf = vec![0u8; cl8::max_encoded_len(s.len())];
        let n = cl8::encode_into(&s, &mut buf).expect("кодирование");
        assert_eq!(n, 1, "{name} (U+{:04X}) должен стоить один байт", ch as u32);
        assert_ne!(
            buf[0],
            cl8::consts::UNI_ON,
            "{name} не должен открывать фоллбэк"
        );
    }
}

#[test]
fn fallback_frame_costs_two_bytes_per_block() {
    // Рамка блока — UNI_ON плюс терминатор. На сплошном куске она амортизируется,
    // на россыпи одиночных символов и составляет всю переплату.
    let solid = "日本語漢字";
    let scattered = "日a本a語a漢a字";

    let enc = |s: &str| {
        let mut buf = vec![0u8; cl8::max_encoded_len(s.len())];
        let n = cl8::encode_into(s, &mut buf).expect("кодирование");
        (n, buf[..n].iter().filter(|&&b| b == cl8::consts::UNI_ON).count())
    };

    let (solid_len, solid_blocks) = enc(solid);
    assert_eq!(solid_blocks, 1, "сплошной кусок — один блок");
    assert_eq!(solid_len, solid.len() + 2, "один блок стоит два байта рамки");

    let (scattered_len, scattered_blocks) = enc(scattered);
    assert_eq!(scattered_blocks, 5, "каждый символ вразбивку — свой блок");
    assert_eq!(
        scattered_len,
        scattered.len() + scattered_blocks * 2,
        "накладные растут линейно по числу блоков"
    );
}
