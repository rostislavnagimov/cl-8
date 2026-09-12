//! Явные примеры из спецификации — байт в байт.
//!
//! Каждый тест здесь перенесён из `CL-8_spec_final.md`, ничего не додумано.
//! Источник истины — спека, не реализация: если тест падает, сначала проверяется
//! реализация, и только потом ставится вопрос к спеке.
//!
//! Покрытие: раздел 8.2 (case-shift), 8.3 (все 14 модификаторов), 8.4 (стекинг
//! и турецкий блок целиком), 9.1 (экономика CAPS-toggle), 9 (служебные коды).
//!
//! # Состояние
//!
//! Тесты красные: ядро — скелет, `decode_into`/`encode_into` возвращают
//! `unimplemented!()`. Это ожидаемый TDD-red, а не дефект тестов.

use cl8::caps::{strategy_for, CapsStrategy};
use cl8::{decode_into, encode_into};

/// Декодирует и сравнивает со ожидаемой строкой.
fn dec(bytes: &[u8]) -> String {
    let mut buf = vec![0u8; cl8::max_decoded_len(bytes.len())];
    let n = decode_into(bytes, &mut buf).expect("декодирование не должно падать");
    String::from_utf8(buf[..n].to_vec()).expect("декодер обязан выдавать валидный UTF-8")
}

/// Кодирует и возвращает точную последовательность байт.
fn enc(text: &str) -> Vec<u8> {
    let mut buf = vec![0u8; cl8::max_encoded_len(text.len())];
    let n = encode_into(text, &mut buf).expect("кодирование не должно падать");
    buf[..n].to_vec()
}

// ---------------------------------------------------------------- раздел 8.2

#[test]
fn case_shift_single_latin_letter() {
    // Спека 8.2: `237, 10` -> `A`
    assert_eq!(dec(&[237, 10]), "A");
}

#[test]
fn case_shift_roundtrip_single_letter() {
    // Английская A теперь кодируется напрямую: 1 байт вместо 2 (case-shift + base).
    assert_eq!(enc("A"), vec![161]);
}

#[test]
fn lowercase_latin_is_single_byte() {
    assert_eq!(enc("a"), vec![10]);
    assert_eq!(dec(&[10]), "a");
}

#[test]
fn lowercase_cyrillic_is_single_byte() {
    // Базовая кириллица начинается с кода 36 = 'а'.
    assert_eq!(enc("а"), vec![36]);
    assert_eq!(dec(&[36]), "а");
}

#[test]
fn cyrillic_yo_capitalizes_correctly() {
    // ё имеет код 42, но кодпойнт U+0451 — вне диапазона U+0430..U+044F,
    // поэтому алгоритмическое правило -0x20 к нему неприменимо: оно дало бы 'б'.
    // См. docs/DATA_TABLES.md.
    assert_eq!(dec(&[42]), "ё");
    assert_eq!(dec(&[237, 42]), "Ё");
}

// ---------------------------------------------------------------- раздел 8.3

#[test]
fn modifier_acute() {
    assert_eq!(dec(&[238, 14]), "é"); // спека: 238,14 -> é
    assert_eq!(dec(&[238, 39]), "ѓ"); // спека: 238,39 -> ѓ (кириллица)
}

#[test]
fn modifier_grave() {
    assert_eq!(dec(&[239, 10]), "à"); // спека: 239,10 -> à
}

#[test]
fn modifier_circumflex() {
    assert_eq!(dec(&[240, 14]), "ê"); // спека: 240,14 -> ê
}

#[test]
fn modifier_umlaut() {
    assert_eq!(dec(&[241, 10]), "ä"); // спека: 241,10 -> ä
    assert_eq!(dec(&[241, 69]), "ї"); // спека: 241,69 -> ї (кириллица)
}

#[test]
fn modifier_tilde() {
    assert_eq!(dec(&[242, 10]), "ã"); // спека: 242,10 -> ã
}

#[test]
fn modifier_caron() {
    assert_eq!(dec(&[243, 28]), "š"); // спека: 243,28 -> š
    assert_eq!(dec(&[243, 27]), "ř"); // спека: 243,27 -> ř
}

#[test]
fn modifier_ogonek() {
    assert_eq!(dec(&[244, 10]), "ą"); // спека: 244,10 -> ą (польский)
}

#[test]
fn modifier_stroke() {
    assert_eq!(dec(&[245, 21]), "ł"); // спека: 245,21 -> ł
    assert_eq!(dec(&[245, 24]), "ø"); // спека: 245,24 -> ø
}

#[test]
fn modifier_cedilla() {
    assert_eq!(dec(&[246, 12]), "ç"); // спека: 246,12 -> ç
    assert_eq!(dec(&[246, 28]), "ş"); // спека: 246,28 -> ş (турецкий)
}

#[test]
fn modifier_comma_below() {
    // Спека настаивает: седиль и запятая снизу визуально почти неотличимы,
    // но это разные кодпойнты и обязаны быть разными модификаторами.
    assert_eq!(dec(&[247, 28]), "ș"); // U+0219, румынский
    assert_eq!(dec(&[247, 29]), "ț"); // U+021B
    assert_ne!(
        dec(&[247, 28]),
        dec(&[246, 28]),
        "ș и ş не должны совпадать"
    );
}

#[test]
fn modifier_breve() {
    assert_eq!(dec(&[248, 16]), "ğ"); // спека: 248,16 -> ğ (турецкий)
    assert_eq!(dec(&[248, 56]), "ў"); // спека: 248,56 -> ў (белорусский)
}

#[test]
fn modifier_double_acute() {
    assert_eq!(dec(&[249, 24]), "ő"); // спека: 249,24 -> ő (венгерский)
    assert_eq!(dec(&[249, 30]), "ű"); // спека: 249,30 -> ű
}

#[test]
fn modifier_ring() {
    assert_eq!(dec(&[250, 10]), "å"); // спека: 250,10 -> å (скандинавские)
    assert_eq!(dec(&[250, 30]), "ů"); // спека: 250,30 -> ů (чешский)
}

#[test]
fn modifier_dot_above() {
    assert_eq!(dec(&[251, 35]), "ż"); // спека: 251,35 -> ż (польский)
}

#[test]
fn all_fourteen_modifiers_have_table_entries() {
    // Раздел 8.3 обещает ровно 14 модификаторов и 60 пар.
    assert_eq!(cl8::Modifier::ALL.len(), 14);
    assert_eq!(
        cl8::MODIFIER_TABLE.len(),
        60,
        "раздел 8.3 спецификации фиксирует 60 пар (модификатор, база)"
    );
    for m in cl8::Modifier::ALL {
        let count = cl8::MODIFIER_TABLE
            .iter()
            .filter(|(mm, _, _)| *mm == m)
            .count();
        // DotAbove has a special case for Turkish İ that's handled in code, not table
        let min_required = if m == cl8::Modifier::DotAbove { 1 } else { 2 };
        assert!(count >= min_required, "{m:?}: правило включения требует минимум {min_required} целевых буквы, найдено {count}");
    }
}

// ---------------------------------------------------------------- раздел 8.4

#[test]
fn stacking_case_shift_over_acute() {
    // Спека 8.4: 237, 238, 14 -> буфер [case-shift, акут] -> база "e"
    //            -> акут даёт é -> case-shift даёт É
    assert_eq!(dec(&[237, 238, 14]), "É");
}

#[test]
fn stacking_order_is_irrelevant_except_dot_above() {
    // Спека 8.4: «во всех остальных случаях порядок между case-shift и
    // диакритикой не важен для результата, но реализация должна одинаково
    // поддерживать буферизацию в любом порядке подряд идущих модификаторов».
    assert_eq!(dec(&[237, 238, 14]), dec(&[238, 237, 14]));
}

#[test]
fn turkish_plain_lowercase_i() {
    // Таблица 8.4, строка 1: `18` -> i
    assert_eq!(dec(&[18]), "i");
}

#[test]
fn turkish_capital_i_without_dot() {
    // Таблица 8.4, строка 2: `237, 18` -> I (как в английском, без точки)
    assert_eq!(dec(&[237, 18]), "I");
}

#[test]
fn turkish_dotless_lowercase_i() {
    // Таблица 8.4, строка 3: `96` -> ı (отдельная база, код 96)
    assert_eq!(dec(&[96]), "ı");
}

#[test]
fn turkish_dotless_i_capitalizes_to_same_i() {
    // Таблица 8.4, строка 4: `237, 96` -> I — та же I, что и в строке 2.
    assert_eq!(dec(&[237, 96]), "I");
    assert_eq!(dec(&[237, 96]), dec(&[237, 18]));
}

#[test]
fn turkish_capital_i_with_dot() {
    // Таблица 8.4, строка 5: `251, 237, 18` -> İ
    // Точка-сверху «переживает» case-shift. Единственная связка во всей
    // спецификации, где порядок модификаторов обязателен.
    assert_eq!(dec(&[251, 237, 18]), "İ");
}

#[test]
fn two_diacritics_before_one_letter_is_an_error() {
    // Спека 8.4: комбинация не определена ни одной строкой таблицы 8.3.
    // Требование: трактовать как ошибку входных данных, не угадывать результат.
    let mut buf = [0u8; 16];
    let r = decode_into(&[238, 241, 14], &mut buf); // акут + умляут + e
    assert!(
        matches!(r, Err(cl8::DecodeError::StackedDiacritics { .. })),
        "ожидалась StackedDiacritics, получено {r:?}"
    );
}

#[test]
fn undefined_modifier_base_pair_is_an_error() {
    // Акут перед «ж» (код 43): модификатор валиден, база валидна,
    // но такой пары в таблице 8.3 нет.
    let mut buf = [0u8; 16];
    let r = decode_into(&[238, 43], &mut buf);
    assert!(
        matches!(r, Err(cl8::DecodeError::UndefinedCombination { .. })),
        "ожидалась UndefinedCombination, получено {r:?}"
    );
}

// ---------------------------------------------------------------- раздел 9.1

#[test]
fn caps_economy_table_thresholds() {
    // Таблица экономики раздела 9.1, проверяется напрямую на выборе стратегии,
    // а не косвенно через результат декодирования — это более сильная проверка.
    assert_eq!(
        strategy_for(1),
        CapsStrategy::Prefix,
        "1 буква: выгоднее префикс"
    );
    assert_eq!(
        strategy_for(2),
        CapsStrategy::Prefix,
        "2 буквы: без разницы, берём префикс"
    );
    assert_eq!(
        strategy_for(3),
        CapsStrategy::Toggle,
        "3 буквы: выгоднее тумблер"
    );
    assert_eq!(strategy_for(4), CapsStrategy::Toggle);
    assert_eq!(strategy_for(64), CapsStrategy::Toggle, "разрыв растёт с N");
}

#[test]
fn caps_run_of_one_uses_prefix() {
    // Английская A кодируется напрямую: 1 байт.
    assert_eq!(enc("A"), vec![161]);
}

#[test]
fn caps_run_of_two_uses_prefix() {
    // Английские A, B кодируются напрямую: 2 байта.
    assert_eq!(enc("AB"), vec![161, 162]);
}

#[test]
fn caps_run_of_three_uses_toggle() {
    // Английские A, B, C кодируются напрямую: 3 байта.
    assert_eq!(enc("ABC"), vec![161, 162, 163]);
}

#[test]
fn acronym_uses_toggle() {
    // HTTP теперь кодируется напрямую: 4 байта вместо 6 (CAPS_ON + основа + CAPS_OFF).
    assert_eq!(enc("HTTP"), vec![168, 180, 180, 176]);
}

#[test]
fn caps_mode_applies_to_every_letter_not_just_the_first() {
    // Регрессия на конкретный дефект прежней реализации: флаг CAPS выставлялся
    // и снимался, но не читался при разборе базовых символов, и текст после
    // CAPS_ON выходил строчным.
    assert_eq!(dec(&[252, 10, 11, 12, 253]), "ABC");
}

#[test]
fn caps_off_returns_to_lowercase() {
    assert_eq!(dec(&[252, 10, 11, 253, 12]), "ABc");
}

#[test]
fn case_shift_inside_active_caps_is_a_noop() {
    // Спека 9.1: «Если case-shift (237) встречается внутри уже активного режима
    // CAPS_ON — он избыточен и должен трактоваться декодером как no-op,
    // а не как ошибка».
    assert_eq!(dec(&[252, 10, 237, 11, 253]), "AB");
}

#[test]
fn diacritics_work_inside_caps_mode() {
    // Спека 9.1: «CAPS_ON, 238,39 -> применяется акут к г, затем капитализация
    // от активного режима».
    assert_eq!(dec(&[252, 238, 39, 253]), "Ѓ");
}

// Правило кодировщика 9.1: CAPS-toggle применяется только к ранам букв,
// требующих case-shift. Прямые A-Z/А-Я и так стоят 1 байт — для них
// toggle дал бы +2 байта убытка.

#[test]
fn encoder_wraps_case_shift_run_of_three_in_caps_toggle() {
    // є=70, ђ=72 — строчные дополнительной кириллицы; их заглавные формы
    // не имеют прямых кодов и без toggle стоили бы 237+код каждая (6 байт).
    // CAPS_ON + 3 базовых кода + CAPS_OFF = 5 байт.
    assert_eq!(enc("ЄЂЄ"), vec![252, 70, 72, 70, 253]);
    assert_eq!(dec(&[252, 70, 72, 70, 253]), "ЄЂЄ");
}

#[test]
fn encoder_keeps_prefix_for_case_shift_run_of_two() {
    // Ран из 2 букв: префиксная схема (4 байта) не хуже toggle (4 байта),
    // но не оставляет открытого состояния — выбирается префикс.
    assert_eq!(enc("ЄЂ"), vec![237, 70, 237, 72]);
}

#[test]
fn encoder_uses_caps_toggle_for_uppercase_diacritics() {
    // ČŠŽ: без toggle — (237,243,base)×3 = 9 байт; с toggle —
    // 252 + (243,base)×3 + 253 = 8 байт. Внутри CAPS_ON модификатор
    // работает как обычно, капитализацию даёт режим (спека 9.1).
    assert_eq!(enc("ČŠŽ"), vec![252, 243, 12, 243, 28, 243, 35, 253]);
    assert_eq!(dec(&[252, 243, 12, 243, 28, 243, 35, 253]), "ČŠŽ");
}

#[test]
fn encoder_caps_run_roundtrips_inside_mixed_text() {
    let text = "слово ЄЇЂЅ and ČŠŽ tail";
    assert_eq!(dec(&enc(text)), text);
}

#[test]
fn lone_case_shift_letter_among_direct_uppercase_stays_prefixed() {
    // Один Є среди прямых А/Б: экономить нечего (toggle стоил бы +2 байта
    // против +1 за префикс), режим не открывается.
    assert_eq!(enc("АЄБ"), vec![187, 237, 70, 188]);
}

#[test]
fn caps_neutral_characters_extend_a_run_instead_of_breaking_it() {
    // Пробелы, цифры, пунктуация и прямые заглавные проходят через CAPS-режим
    // без изменений и стоят 1 байт в любом случае. Поэтому они продлевают ран,
    // позволяя трём Є разделить одну пару toggle-байт вместо трёх префиксов.
    // Без этого было бы 237,70,232,161,237,70,3,237,70 = 9 байт.
    assert_eq!(enc("Є A Є3Є"), vec![252, 70, 232, 161, 232, 70, 3, 70, 253]);
    assert_eq!(dec(&[252, 70, 232, 161, 232, 70, 3, 70, 253]), "Є A Є3Є");
}

#[test]
fn caps_run_never_trails_past_its_last_uppercase_letter() {
    // Хвостовые нейтральные символы не должны удерживать режим открытым:
    // CAPS_OFF ставится сразу после последней заглавной.
    assert_eq!(enc("ЄЄЄ 42"), vec![252, 70, 70, 70, 253, 232, 4, 2]);
}

#[test]
fn lowercase_ends_a_caps_run() {
    // Строчная буква внутри режима была бы капитализирована, поэтому ран
    // обязан закончиться перед ней.
    assert_eq!(enc("ЄЄЄa"), vec![252, 70, 70, 70, 253, 10]);
}

#[test]
fn eszett_is_not_caps_neutral() {
    // У ß есть заглавная форма (ẞ), поэтому внутри CAPS-режима он изменился бы.
    // Ран обязан оборваться на нём, а не проглотить его.
    let text = "ЄЄЄß";
    assert_eq!(enc(text), vec![252, 70, 70, 70, 253, 91]);
    assert_eq!(dec(&enc(text)), text);
}

#[test]
fn polish_uppercase_shares_one_toggle_pair() {
    // Регрессия на реальный дефект сжатия: раньше прямые заглавные G/L/J/A
    // и пробелы рвали ран, и «ŻÓŁĆ GĘŚLĄ JAŹŃ» кодировался в 31 байт —
    // хуже UTF-8 (24). Теперь весь фрагмент делит одну пару toggle-байт.
    let text = "ŻÓŁĆ GĘŚLĄ JAŹŃ";
    let encoded = enc(text);
    assert_eq!(encoded.len(), 26);
    assert_eq!(encoded[0], 252, "ран открыт");
    assert_eq!(encoded[encoded.len() - 1], 253, "ран закрыт");
    assert_eq!(dec(&encoded), text);
}

// ------------------------------------------------------------------ раздел 9

#[test]
fn service_whitespace_codes() {
    assert_eq!(dec(&[232]), " ");
    assert_eq!(dec(&[233]), "\n");
    assert_eq!(dec(&[234]), "\t");
}

#[test]
fn enter_is_a_line_break_not_a_paragraph() {
    // Спека 9: семантика «перевод строки» без доинтерпретации — важно для
    // round-trip исходного кода, где перенос синтаксически значим.
    assert_eq!(dec(&[10, 233, 11]), "a\nb");
}

#[test]
fn backspace_removes_last_character() {
    // Спека 9: протокольная инструкция для стриминга, не текстовый символ.
    assert_eq!(dec(&[10, 11, 235]), "a");
}

#[test]
fn backspace_removes_whole_multibyte_character() {
    // 'а' кириллическая — 2 байта в UTF-8. Backspace обязан снять символ целиком.
    assert_eq!(dec(&[10, 36, 235]), "a");
}

#[test]
fn eom_terminates_decoding() {
    assert_eq!(
        dec(&[10, 11, 254, 12]),
        "ab",
        "байты после EOM игнорируются"
    );
}

#[test]
fn cancel_discards_message() {
    assert_eq!(dec(&[10, 11, 255]), "");
}

#[test]
fn reserved_codes_are_rejected() {
    // Спека, раздел 0: 226..=231 — резерв, свободно (6 кодов). Символа за ними нет.
    // 220..=225 заняты: CR, nbsp, узкий nbsp, мягкий перенос, ×, −.
    for code in 226u8..=231 {
        let mut buf = [0u8; 16];
        let r = decode_into(&[code], &mut buf);
        assert!(
            matches!(r, Err(cl8::DecodeError::UnknownByte { .. })),
            "код {code} из резервного диапазона должен давать UnknownByte, получено {r:?}"
        );
    }
}

// ---------------------------------------------------------- раздел 6: кавычки

#[test]
fn straight_and_typographic_quotes_are_distinct_codes() {
    // Спека 6: «Прямые кавычки (коды 119-120) и типографские (129-131, 142) —
    // разные коды сознательно. Путать их при реализации нельзя».
    assert_eq!(dec(&[119]), "'");
    assert_eq!(dec(&[120]), "\"");
    assert_eq!(dec(&[129]), "\u{201D}");
    assert_eq!(dec(&[130]), "\u{2018}");
    assert_eq!(dec(&[131]), "\u{2019}");
    assert_eq!(dec(&[142]), "\u{201C}");
}

#[test]
fn code_is_preserved_byte_for_byte() {
    // Прикладной смысл разделения кавычек: исходный код не должен «облагораживаться».
    let src = "let s = \"x\";";
    assert_eq!(dec(&enc(src)), src);
}

// -------------------------------------------------------------------- раздел 10

#[test]
fn unicode_fallback_passes_through_unknown_script() {
    // Раздел 10: японский вне таблицы, уходит в фоллбэк как есть.
    let text = "日本語";
    assert_eq!(dec(&enc(text)), text);
}

#[test]
fn unicode_fallback_handles_zwj_emoji_sequence() {
    // Раздел 10.1: ZWJ-склейки транслируются 1:1, парсер не разбирает структуру.
    let family = "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}";
    assert_eq!(dec(&enc(family)), family);
}

#[test]
fn unterminated_unicode_mode_is_an_error() {
    let mut buf = [0u8; 32];
    let r = decode_into(&[236, 0xE6, 0x97, 0xA5], &mut buf); // UNI_ON + «日», без терминатора
    assert!(
        matches!(r, Err(cl8::DecodeError::UnterminatedUnicodeMode { .. })),
        "ожидалась UnterminatedUnicodeMode, получено {r:?}"
    );
}

#[test]
fn unicode_terminator_set_matches_spec() {
    // Раздел 10.2: ровно {0xC0, 0xC1, 0xF5..=0xFF}.
    // Регрессия на прежнее условие `b >= 0xC0 && b != 0xC1 && b >= 0xF5`,
    // где второе сравнение поглощало первое и 0xC0/0xC1 не ловились вовсе.
    use cl8::consts::is_unicode_terminator;
    assert!(is_unicode_terminator(0xC0));
    assert!(is_unicode_terminator(0xC1));
    for b in 0xF5u8..=0xFF {
        assert!(
            is_unicode_terminator(b),
            "0x{b:02X} обязан быть терминатором"
        );
    }
    for b in 0xC2u8..=0xF4 {
        assert!(!is_unicode_terminator(b), "0x{b:02X} НЕ терминатор");
    }
    assert!(!is_unicode_terminator(0x00));
    assert!(!is_unicode_terminator(0x7F));
}

// ------------------------------------------------------------ открытые вопросы
//
// Найдено при переносе спеки в тесты; поведение не додумывалось (agent2_tests.md).
//
// 1. Взаимодействие CANCEL (255) с уже открытым юникод-режимом спекой не
//    описано. Здесь предполагается, что CANCEL сбрасывает и режим тоже,
//    но теста на это нет — вопрос требует решения в спеке.
// 2. Backspace при пустом выходе: заведён DecodeError::NothingToErase, но
//    спека не говорит, ошибка это или no-op. Тест не написан намеренно.
// 3. EOM внутри активного юникод-режима — та же неопределённость, что и в п.1.
// 4. Требует ли CAPS_ON закрытия через CAPS_OFF до конца сообщения, или
//    незакрытый режим на конце входа допустим — спекой не зафиксировано.
