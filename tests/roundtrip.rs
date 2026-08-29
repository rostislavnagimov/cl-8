//! Property-based тесты: `decode(encode(s)) == s`.
//!
//! # Ограничение, зафиксированное явно
//!
//! Все генераторы выдают только NFC-нормализованные строки. Это не обход
//! проблемы, а следствие раздела 10.3 спецификации: NFC — обязательный
//! препроцессинг, кодировщик ненормализованный вход отвергает
//! ([`cl8::EncodeError::NotNfcNormalized`]). Тест на NFD-вход существует
//! отдельно и проверяет именно предсказуемый отказ, а не round-trip.
//!
//! # Состояние
//!
//! Тесты красные: ядро — скелет. Ожидаемый TDD-red.

use proptest::prelude::*;

/// Round-trip через core-API, без обёрток с аллокацией: так тест работает при
/// любом наборе feature-флагов крейта.
fn roundtrip(text: &str) -> Result<String, String> {
    let mut encoded = vec![0u8; cl8::max_encoded_len(text.len())];
    let n = cl8::encode_into(text, &mut encoded).map_err(|e| format!("encode: {e:?}"))?;
    let encoded = &encoded[..n];

    let mut decoded = vec![0u8; cl8::max_decoded_len(encoded.len())];
    let m = cl8::decode_into(encoded, &mut decoded).map_err(|e| format!("decode: {e:?}"))?;
    String::from_utf8(decoded[..m].to_vec()).map_err(|e| format!("не UTF-8: {e}"))
}

// ------------------------------------------------------------------ генераторы

/// Символы, кодируемые одним байтом из базовой таблицы.
fn base_char() -> impl Strategy<Value = char> {
    (0usize..cl8::BASE_TABLE.len()).prop_map(|i| {
        char::from_u32(cl8::BASE_TABLE[i]).expect("таблица содержит валидные скаляры")
    })
}

/// Символы, кодируемые парой модификатор+база.
fn modified_char() -> impl Strategy<Value = char> {
    (0usize..cl8::MODIFIER_TABLE.len()).prop_map(|i| {
        let (_, _, cp) = cl8::MODIFIER_TABLE[i];
        char::from_u32(cp).expect("таблица модификаторов содержит валидные скаляры")
    })
}

/// Символы вне таблицы — проверка юникод-фоллбэка.
///
/// Диапазоны выбраны заведомо отсутствующими в CL-8: CJK, арабский,
/// деванагари, хангыль.
fn fallback_char() -> impl Strategy<Value = char> {
    prop_oneof![
        (0x4E00u32..0x9FFF), // CJK
        (0x0600u32..0x06FF), // арабский
        (0x0900u32..0x097F), // деванагари
        (0xAC00u32..0xD7A3), // хангыль
    ]
    .prop_filter_map("валидный скаляр", char::from_u32)
}

/// Смешанная строка: базовые + диакритика + фоллбэк вперемешку.
///
/// Именно этот случай — главный аргумент спеки (раздел 12), и именно на нём
/// ломаются переходы в юникод-режим и обратно.
fn mixed_string() -> impl Strategy<Value = String> {
    prop::collection::vec(
        prop_oneof![
            3 => base_char(),
            2 => modified_char(),
            1 => fallback_char(),
        ],
        0..64,
    )
    .prop_map(|chars| chars.into_iter().collect())
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 10_000, ..ProptestConfig::default() })]

    /// Основное свойство на символах базовой таблицы.
    #[test]
    fn roundtrip_base_table(chars in prop::collection::vec(base_char(), 0..128)) {
        let s: String = chars.into_iter().collect();
        prop_assert_eq!(roundtrip(&s).map_err(TestCaseError::fail)?, s);
    }

    /// Символы, требующие пары модификатор+база.
    #[test]
    fn roundtrip_modified(chars in prop::collection::vec(modified_char(), 0..128)) {
        let s: String = chars.into_iter().collect();
        prop_assert_eq!(roundtrip(&s).map_err(TestCaseError::fail)?, s);
    }

    /// Чистый фоллбэк — вся строка уходит в юникод-режим.
    #[test]
    fn roundtrip_fallback_only(chars in prop::collection::vec(fallback_char(), 0..64)) {
        let s: String = chars.into_iter().collect();
        prop_assert_eq!(roundtrip(&s).map_err(TestCaseError::fail)?, s);
    }

    /// Смешение всех трёх видов в одной строке.
    #[test]
    fn roundtrip_mixed(s in mixed_string()) {
        prop_assert_eq!(roundtrip(&s).map_err(TestCaseError::fail)?, s);
    }

    /// Произвольный Unicode: не обязан кодироваться компактно, но обязан
    /// пережить round-trip либо дать явную ошибку — молча портить текст нельзя.
    #[test]
    fn roundtrip_arbitrary_unicode(s in ".{0,64}") {
        match roundtrip(&s) {
            Ok(out) => prop_assert_eq!(out, s),
            Err(e) => prop_assert!(
                e.starts_with("encode:") || e.starts_with("decode:"),
                "ошибка должна быть типизированной, получено: {}", e
            ),
        }
    }

    /// Декодер обязан выдавать валидный UTF-8 на ЛЮБОМ входе, в том числе
    /// заведомо битом: либо строка, либо типизированная ошибка, но не мусор.
    #[test]
    fn decoder_emits_valid_utf8(bytes in prop::collection::vec(any::<u8>(), 0..128)) {
        let mut out = vec![0u8; cl8::max_decoded_len(bytes.len())];
        if let Ok(n) = cl8::decode_into(&bytes, &mut out) {
            prop_assert!(
                core::str::from_utf8(&out[..n]).is_ok(),
                "декодер выдал невалидный UTF-8 на входе {:?}", bytes
            );
        }
    }

    /// Декодер не должен паниковать ни на каком входе — только Result.
    #[test]
    fn decoder_never_panics(bytes in prop::collection::vec(any::<u8>(), 0..256)) {
        let mut out = vec![0u8; cl8::max_decoded_len(bytes.len())];
        let _ = cl8::decode_into(&bytes, &mut out);
    }

    /// Объявленная верхняя граница обязана быть достаточной: если encode_into
    /// возвращает OutputTooSmall при буфере max_encoded_len — граница неверна.
    #[test]
    fn max_encoded_len_is_sufficient(s in mixed_string()) {
        let mut buf = vec![0u8; cl8::max_encoded_len(s.len())];
        if let Err(cl8::EncodeError::OutputTooSmall { needed, available }) = cl8::encode_into(&s, &mut buf) { prop_assert!(
            false,
            "max_encoded_len({}) = {} недостаточно, нужно {}",
            s.len(), available, needed
        ) }
    }

    /// То же для декодера.
    #[test]
    fn max_decoded_len_is_sufficient(chars in prop::collection::vec(base_char(), 0..64)) {
        let s: String = chars.into_iter().collect();
        let mut enc = vec![0u8; cl8::max_encoded_len(s.len())];
        if let Ok(n) = cl8::encode_into(&s, &mut enc) {
            let mut dec = vec![0u8; cl8::max_decoded_len(n)];
            if let Err(cl8::DecodeError::OutputTooSmall { needed, available }) = cl8::decode_into(&enc[..n], &mut dec) { prop_assert!(
                false, "max_decoded_len({n}) = {available} недостаточно, нужно {needed}"
            ) }
        }
    }
}

/// NFD-вход обязан давать предсказуемый отказ, а не round-trip.
///
/// Раздел 10.3 требует NFC. «é» как e + U+0301 — ровно тот случай, ради
/// которого введена [`cl8::EncodeError::NotNfcNormalized`].
#[test]
fn nfd_input_is_rejected_predictably() {
    let nfd = "e\u{0301}"; // e + комбинирующий акут
    let mut buf = vec![0u8; cl8::max_encoded_len(nfd.len())];
    let r = cl8::encode_into(nfd, &mut buf);
    assert!(
        matches!(r, Err(cl8::EncodeError::NotNfcNormalized { .. })),
        "NFD-вход должен отвергаться явно, получено {r:?}"
    );
}

/// Компактность на кириллице: главное обещание кодировки.
///
/// Кириллица в UTF-8 — 2 байта на букву, в CL-8 — 1. Тест фиксирует, что
/// выигрыш есть, не претендуя на точный процент (это работа бенчмарков).
#[test]
fn cyrillic_is_more_compact_than_utf8() {
    let text = "съешь ещё этих мягких французских булок да выпей чаю";
    let mut buf = vec![0u8; cl8::max_encoded_len(text.len())];
    let n = cl8::encode_into(text, &mut buf).expect("кодирование");
    assert!(
        n < text.len(),
        "CL-8 дал {n} байт против {} в UTF-8 — выигрыша нет",
        text.len()
    );
}
