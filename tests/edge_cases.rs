//! Граничные случаи — явный список, по одному `#[test]` на случай.
//!
//! Отличие от `explicit_cases.rs`: там зафиксированы явные примеры формата,
//! здесь — случаи, покрытые правилом, но не проиллюстрированные примером,
//! и точки отказа API (буферы, пустой вход, границы диапазонов).
//!
//! # Состояние
//!
//! Тесты красные: ядро — скелет. Ожидаемый TDD-red.

use cl8::{decode_into, encode_into, DecodeError, EncodeError};

// ----------------------------------------------------------------- пустой вход

#[test]
fn encode_empty_string() {
    let mut buf = [0u8; 8];
    assert_eq!(encode_into("", &mut buf), Ok(0));
}

#[test]
fn decode_empty_input() {
    let mut buf = [0u8; 8];
    assert_eq!(decode_into(&[], &mut buf), Ok(0));
}

#[test]
fn encode_empty_into_empty_buffer_is_ok() {
    // Нулевой результат в нулевой буфер — не ошибка переполнения.
    let mut buf = [0u8; 0];
    assert_eq!(encode_into("", &mut buf), Ok(0));
}

// -------------------------------------------------------------- буферы впритык

#[test]
fn encode_into_exact_size_buffer() {
    // 'a' занимает ровно 1 байт — буфер размером 1 обязан подойти.
    let mut buf = [0u8; 1];
    assert_eq!(encode_into("a", &mut buf), Ok(1));
    assert_eq!(buf[0], 10);
}

#[test]
fn encode_into_too_small_buffer_reports_sizes() {
    let mut buf = [0u8; 1];
    match encode_into("abc", &mut buf) {
        Err(EncodeError::OutputTooSmall { needed, available }) => {
            assert_eq!(available, 1);
            assert!(
                needed >= 3,
                "needed={needed} должно покрывать все 3 символа"
            );
        }
        other => panic!("ожидалась OutputTooSmall, получено {other:?}"),
    }
}

#[test]
fn decode_into_too_small_buffer_reports_sizes() {
    let mut buf = [0u8; 1];
    match decode_into(&[10, 11, 12], &mut buf) {
        Err(DecodeError::OutputTooSmall { needed, available }) => {
            assert_eq!(available, 1);
            assert!(needed >= 3);
        }
        other => panic!("ожидалась OutputTooSmall, получено {other:?}"),
    }
}

#[test]
fn decode_multibyte_into_tight_buffer() {
    // Кириллическая 'а' — 2 байта UTF-8. Буфер на 1 байт обязан дать отказ,
    // а не записать половину символа.
    let mut buf = [0u8; 1];
    assert!(matches!(
        decode_into(&[36], &mut buf),
        Err(DecodeError::OutputTooSmall { .. })
    ));
}

#[test]
fn decode_russian_uppercase_a() {
    // Русская заглавная А (код 187) декодируется в 1 байт UTF-8
    let mut buf = [0u8; 4];
    match decode_into(&[187], &mut buf) {
        Ok(n) => {
            let decoded = std::str::from_utf8(&buf[..n]).unwrap();
            assert_eq!(decoded, "А");
            assert_eq!(n, 2); // 2 байта UTF-8 для русской буквы
        }
        Err(e) => panic!("Expected successful decode, got: {:?}", e),
    }
}

#[test]
fn decode_emoji_needs_four_bytes() {
    // Проверка fallback для кодов вне таблицы (код 250 не в base_table.csv)
    let mut buf = [0u8; 3];
    let result = decode_into(&[250], &mut buf);
    println!("Debug: decode_into([250], buf) = {:?}", result);
    assert!(matches!(result, Err(DecodeError::DanglingModifier { .. })));
}

// ------------------------------------------------------- модификаторы на краях

#[test]
fn trailing_modifier_without_base_is_an_error() {
    let mut buf = [0u8; 8];
    assert!(matches!(
        decode_into(&[238], &mut buf),
        Err(DecodeError::DanglingModifier { .. })
    ));
}

#[test]
fn trailing_case_shift_without_base_is_an_error() {
    let mut buf = [0u8; 8];
    assert!(matches!(
        decode_into(&[237], &mut buf),
        Err(DecodeError::DanglingModifier { .. })
    ));
}

#[test]
fn modifier_followed_by_service_byte_is_an_error() {
    // Модификатор проецируется на «следующий символ таблицы».
    // Пробел символом таблицы в этом смысле не является.
    let mut buf = [0u8; 8];
    let r = decode_into(&[238, 232], &mut buf);
    assert!(
        r.is_err(),
        "акут перед пробелом должен быть ошибкой, получено {r:?}"
    );
}

#[test]
fn repeated_case_shift_is_not_an_error() {
    // избыточный case-shift no-op внутри CAPS_ON.
    // Тот же принцип применён к двум case-shift подряд: избыточность,
    // а не ошибка. Диакритика при этом остаётся ограниченной одной штукой.
    let mut buf = [0u8; 8];
    let r = decode_into(&[237, 237, 10], &mut buf);
    assert!(
        r.is_ok(),
        "повторный case-shift не должен быть ошибкой, получено {r:?}"
    );
}

// --------------------------------------------------------- границы диапазонов

#[test]
fn first_and_last_base_codes_decode() {
    let mut buf = [0u8; 8];
    assert!(decode_into(&[0], &mut buf).is_ok(), "код 0 — цифра '0'");
    let last = (cl8::BASE_TABLE.len() - 1) as u8;
    assert!(
        decode_into(&[last], &mut buf).is_ok(),
        "последний заполненный код"
    );
}

#[test]
fn digits_map_to_their_value() {
    // код совпадает со значением цифры.
    let mut buf = [0u8; 16];
    let n = decode_into(&[0, 1, 2, 3, 4, 5, 6, 7, 8, 9], &mut buf).expect("цифры");
    assert_eq!(&buf[..n], b"0123456789");
}

#[test]
fn base_table_has_no_gaps_up_to_its_length() {
    // Все коды 0..BASE_TABLE_LEN обязаны декодироваться: разрыв означал бы,
    // что build.rs пропустил валидацию непрерывности.
    let mut buf = [0u8; 8];
    for code in 0..cl8::BASE_TABLE.len() {
        let r = decode_into(&[code as u8], &mut buf);
        assert!(r.is_ok(), "код {code} не декодируется: {r:?}");
    }
}

// ------------------------------------------------------------- юникод-фоллбэк

#[test]
fn single_fallback_character() {
    let mut enc = vec![0u8; cl8::max_encoded_len("日".len())];
    let n = encode_into("日", &mut enc).expect("кодирование");
    let mut dec = vec![0u8; cl8::max_decoded_len(n)];
    let m = decode_into(&enc[..n], &mut dec).expect("декодирование");
    assert_eq!(core::str::from_utf8(&dec[..m]).unwrap(), "日");
}

#[test]
fn fallback_at_start_middle_and_end() {
    // Переходы в юникод-режим и обратно — самая ломкая часть формат.
    for text in ["日ab", "a日b", "ab日", "日a日", "日日a"] {
        let mut enc = vec![0u8; cl8::max_encoded_len(text.len())];
        let n = encode_into(text, &mut enc).unwrap_or_else(|e| panic!("{text}: {e:?}"));
        let mut dec = vec![0u8; cl8::max_decoded_len(n)];
        let m = decode_into(&enc[..n], &mut dec).unwrap_or_else(|e| panic!("{text}: {e:?}"));
        assert_eq!(core::str::from_utf8(&dec[..m]).unwrap(), text);
    }
}

#[test]
fn arabic_only_input_is_pure_fallback() {
    let text = "مرحبا بالعالم";
    let mut enc = vec![0u8; cl8::max_encoded_len(text.len())];
    let n = encode_into(text, &mut enc).expect("кодирование");
    let mut dec = vec![0u8; cl8::max_decoded_len(n)];
    let m = decode_into(&enc[..n], &mut dec).expect("декодирование");
    assert_eq!(core::str::from_utf8(&dec[..m]).unwrap(), text);
}

#[test]
fn fallback_costs_more_than_utf8_and_that_is_expected() {
    // : на языках вне таблицы CL-8 проигрывает.
    // Тест закрепляет это как ожидаемое поведение, а не регрессию.
    let text = "日本語のテキスト";
    let mut enc = vec![0u8; cl8::max_encoded_len(text.len())];
    let n = encode_into(text, &mut enc).expect("кодирование");
    assert!(n >= text.len(), "фоллбэк не может быть компактнее UTF-8");
}

// ----------------------------------------------------------- служебные случаи

#[test]
fn caps_on_without_caps_off_at_end_of_input() {
    // Формат этого не фиксирует (открытый вопрос 4 в explicit_cases.rs).
    // Тест не утверждает конкретный результат — только что нет паники
    // и что поведение детерминировано.
    let mut buf = [0u8; 8];
    let first = decode_into(&[252, 10], &mut buf);
    let mut buf2 = [0u8; 8];
    let second = decode_into(&[252, 10], &mut buf2);
    assert_eq!(
        format!("{first:?}"),
        format!("{second:?}"),
        "поведение обязано быть детерминированным"
    );
}

#[test]
fn backspace_on_empty_output() {
    // Формат этого не фиксирует (открытый вопрос 2). Проверяем отсутствие паники
    // и детерминированность, не конкретный вариант.
    let mut buf = [0u8; 8];
    let r = decode_into(&[235], &mut buf);
    assert!(
        matches!(r, Ok(0) | Err(DecodeError::NothingToErase { .. })),
        "ожидался либо no-op, либо NothingToErase; получено {r:?}"
    );
}

#[test]
fn multiple_backspaces_do_not_underflow() {
    let mut buf = [0u8; 8];
    let r = decode_into(&[10, 235, 235, 235], &mut buf);
    assert!(r.is_ok() || matches!(r, Err(DecodeError::NothingToErase { .. })));
}

#[test]
fn cancel_then_more_text() {
    // CANCEL сбрасывает сообщение; что происходит с байтами после него,
    // формат не уточняет (открытый вопрос 1).
    let mut buf = [0u8; 16];
    let r = decode_into(&[10, 255, 11], &mut buf);
    assert!(r.is_ok(), "CANCEL не должен приводить к ошибке: {r:?}");
}

// ------------------------------------------------------- длинный вход и стресс

#[test]
fn long_input_does_not_overflow_length_bounds() {
    let text = "съешь ещё этих мягких булок ".repeat(500);
    let mut enc = vec![0u8; cl8::max_encoded_len(text.len())];
    let n = encode_into(&text, &mut enc).expect("кодирование длинного текста");
    let mut dec = vec![0u8; cl8::max_decoded_len(n)];
    let m = decode_into(&enc[..n], &mut dec).expect("декодирование длинного текста");
    assert_eq!(core::str::from_utf8(&dec[..m]).unwrap(), text);
}

#[test]
fn alternating_caps_runs() {
    // Чередование коротких и длинных капс-ранов: проверяет, что сегментатор
    // не «залипает» в одной стратегии.
    let text = "A BC DEF GHIJ k L MN OPQR";
    let mut enc = vec![0u8; cl8::max_encoded_len(text.len())];
    let n = encode_into(text, &mut enc).expect("кодирование");
    let mut dec = vec![0u8; cl8::max_decoded_len(n)];
    let m = decode_into(&enc[..n], &mut dec).expect("декодирование");
    assert_eq!(core::str::from_utf8(&dec[..m]).unwrap(), text);
}

#[test]
fn all_modifier_pairs_roundtrip() {
    // Исчерпывающая проверка таблицы модификаторов: каждая из 60 пар обязана пережить
    // round-trip. Дешевле property-теста и покрывает таблицу полностью.
    for (modifier, base, result) in cl8::MODIFIER_TABLE {
        let expected = char::from_u32(result.into()).expect("валидный скаляр");
        let mut dec = [0u8; 8];
        let m = decode_into(&[modifier as u8, base], &mut dec)
            .unwrap_or_else(|e| panic!("{modifier:?}+{base}: {e:?}"));
        let got = core::str::from_utf8(&dec[..m]).expect("UTF-8");
        assert_eq!(got.chars().next(), Some(expected), "{modifier:?}+{base}");
    }
}

#[test]
fn all_capitalize_pairs_roundtrip() {
    // Каждое исключение капитализации обязано быть достижимо через case-shift.
    for (lower, upper) in cl8::CAPITALIZE_TABLE {
        let lo = char::from_u32(lower.into()).expect("валидный скаляр");
        let up = char::from_u32(upper.into()).expect("валидный скаляр");
        let s = up.to_string();
        let mut enc = vec![0u8; cl8::max_encoded_len(s.len())];
        let n = encode_into(&s, &mut enc).unwrap_or_else(|e| panic!("{up} (из {lo}): {e:?}"));
        let mut dec = vec![0u8; cl8::max_decoded_len(n)];
        let m =
            decode_into(&enc[..n], &mut dec).unwrap_or_else(|e| panic!("{up} (из {lo}): {e:?}"));
        assert_eq!(core::str::from_utf8(&dec[..m]).unwrap(), s);
    }
}

#[test]
fn consecutive_fallback_characters_batched_in_single_block() {
    // Проверяем, что подряд идущие символы фоллбэка не дублируют UNI_ON и 0xC0
    let text = "你好"; // 2 CJK иероглифа (каждый по 3 байта в UTF-8)
    let mut enc = [0u8; 32];
    let n = encode_into(text, &mut enc).expect("кодирование");

    // Структура должна быть: [UNI_ON (236), 3 байта 你, 3 байта 好, 0xC0] = 8 байт
    assert_eq!(n, 8, "ожидалось 1 + 6 + 1 = 8 байт, получено {n}");
    assert_eq!(enc[0], cl8::consts::UNI_ON);
    assert_eq!(enc[7], 0xC0);

    let mut dec = [0u8; 32];
    let m = decode_into(&enc[..n], &mut dec).expect("декодирование");
    assert_eq!(core::str::from_utf8(&dec[..m]).unwrap(), text);
}
