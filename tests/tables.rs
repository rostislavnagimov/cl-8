//! Валидация сгенерированных таблиц против счётчиков спецификации.
//!
//! Единственный тестовый файл, который обязан быть ЗЕЛЁНЫМ уже сейчас: он не
//! вызывает `encode`/`decode`, а проверяет только слой данных, полностью
//! готовый. Если он красный — сломан `build.rs` или CSV, и браться за логику
//! кодирования рано.

use cl8::tables::{
    Modifier, BASE_TABLE, BASE_TABLE_LEN, CAPITALIZE_TABLE, CAPITALIZE_TABLE_LEN, MODIFIER_TABLE,
    MODIFIER_TABLE_LEN,
};

#[test]
fn base_table_matches_spec_budget() {
    // коды 0..=225 заполнены, 226..=231 резерв, 232..=255 служебные.
    // 0..=219 — базовый алфавит с русским uppercase.
    // 220..=225 — CR, nbsp, узкий nbsp, мягкий перенос, ×, − (ранее уходили в фоллбэк).
    assert_eq!(
        BASE_TABLE_LEN, 226,
        "формат фиксирует 226 заполненных кодов (0..=225)"
    );
    assert_eq!(BASE_TABLE.len(), BASE_TABLE_LEN);
}

#[test]
fn base_table_codepoints_are_unique() {
    // Дубль означал бы, что один символ имеет два кода, и encode стал бы
    // неоднозначным. build.rs это проверяет; тест закрепляет инвариант.
    let mut sorted: Vec<u16> = BASE_TABLE.to_vec();
    sorted.sort_unstable();
    let before = sorted.len();
    sorted.dedup();
    assert_eq!(
        before,
        sorted.len(),
        "в базовой таблице есть дублирующиеся кодпойнты"
    );
}

#[test]
fn base_table_contains_only_valid_scalars() {
    for (code, cp) in BASE_TABLE.iter().enumerate() {
        assert!(
            char::from_u32((*cp).into()).is_some(),
            "код {code}: 0x{cp:04X} не является валидным Unicode-скаляром"
        );
    }
}

#[test]
fn base_table_layout_matches_spec_sections() {
    // Точечная сверка границ блоков таблицы.
    let at = |i: usize| char::from_u32(BASE_TABLE[i].into()).unwrap();
    assert_eq!(at(0), '0', "формат: код 0 — цифра 0");
    assert_eq!(at(9), '9', "формат: код 9 — цифра 9");
    assert_eq!(at(10), 'a', "формат: латиница начинается с кода 10");
    assert_eq!(at(35), 'z', "формат: латиница кончается кодом 35");
    assert_eq!(at(36), 'а', "формат: кириллица начинается с кода 36");
    assert_eq!(at(42), 'ё', "формат: ё имеет код 42");
    assert_eq!(at(68), 'я', "формат: кириллица база кончается кодом 68");
    assert_eq!(at(69), 'і', "формат: кириллица доп. начинается с кода 69");
    assert_eq!(at(90), 'ӯ', "формат: кириллица доп. кончается кодом 90");
    assert_eq!(
        at(91),
        'ß',
        "формат: нераскладываемые начинаются с кода 91"
    );
    assert_eq!(at(96), 'ı', "формат: турецкая ı имеет код 96");
    assert_eq!(at(97), '`', "формат: пунктуация начинается с кода 97");
    assert_eq!(at(160), '…', "формат: пунктуация кончается кодом 160");
}

#[test]
fn modifier_table_matches_spec_count() {
    // Ожидается 60 записей. Ранее CSV содержал 55 — данные потерялись
    // при переносе из cl8_decode_tables.rs.
    assert_eq!(
        MODIFIER_TABLE_LEN, 60,
        "формат фиксирует 60 пар (модификатор, база)"
    );
}

#[test]
fn modifier_results_are_unique() {
    // Ранее в CSV было 5 дублирующихся результатов (0x00E5, 0x0101, 0x0107,
    // 0x015F, 0x016B) — каждый вместе с неверным базовым кодом или диакритикой.
    let mut results: Vec<u16> = MODIFIER_TABLE.iter().map(|(_, _, r)| *r).collect();
    results.sort_unstable();
    let before = results.len();
    results.dedup();
    assert_eq!(
        before,
        results.len(),
        "в таблице модификаторов есть дублирующиеся результаты"
    );
}

#[test]
fn modifier_results_are_not_in_base_table() {
    // Иначе символ имел бы два представления: одним байтом и парой.
    for (m, b, result) in MODIFIER_TABLE {
        assert!(
            !BASE_TABLE.contains(&result),
            "{m:?}+{b} даёт 0x{result:04X}, который уже есть в базовой таблице"
        );
    }
}

#[test]
fn modifier_bases_exist_in_base_table() {
    for (m, base, _) in MODIFIER_TABLE {
        assert!(
            (base as usize) < BASE_TABLE.len(),
            "{m:?}: базовый код {base} вне таблицы"
        );
    }
}

#[test]
fn every_modifier_has_at_least_two_targets() {
    // «модификатор оправдан, только если целевых букв >= 2».
    // DotAbove — единственное исключение, и оно кажущееся. Названы две
    // цели: z и i, но i помечена «только в паре с case-shift, см. 8.4»: она
    // достижима правилом стекинга (251, 237, 18 -> İ), а не строкой таблицы.
    // Поэтому строк у DotAbove одна.
    // // Это не противоречие, а единственное прочтение, при котором правило «>= 2»
    // и счётчик «60 записей» согласуются между собой: считать
    // İ строкой таблицы дало бы 61. Проверка ниже закрепляет именно его.
    for m in Modifier::ALL {
        let rows = MODIFIER_TABLE.iter().filter(|(mm, _, _)| *mm == m).count();
        let expected_min = if m == Modifier::DotAbove { 1 } else { 2 };
        assert!(
            rows >= expected_min,
            "{m:?}: {rows} строк таблицы, ожидалось минимум {expected_min}"
        );
    }
}

#[test]
fn dot_above_has_exactly_one_table_row() {
    // Закрепляет разбор из теста выше отдельно, чтобы изменение таблицы
    // модификаторов не прошло молча: если у DotAbove появится вторая строка,
    // счётчик 60 сломается, и причина будет видна сразу здесь.
    let rows: Vec<u8> = MODIFIER_TABLE
        .iter()
        .filter(|(m, _, _)| *m == Modifier::DotAbove)
        .map(|(_, b, _)| *b)
        .collect();
    assert_eq!(rows.len(), 1, "у DotAbove ожидается ровно одна строка (z)");
    assert_eq!(
        cl8::tables::codepoint_of(rows[0]),
        Some(u32::from('z')),
        "единственная строка DotAbove — база 'z' (даёт ż)"
    );
    // Турецкая İ обязана НЕ быть в таблице: она результат стекинга, не пары.
    assert!(
        !MODIFIER_TABLE.iter().any(|(_, _, r)| *r == 0x0130),
        "İ (U+0130) не должна быть строкой таблицы модификаторов — формат"
    );
}

#[test]
fn modifier_discriminants_match_service_codes() {
    // диакритические модификаторы занимают 238..=251.
    assert_eq!(Modifier::Acute as u8, 238);
    assert_eq!(Modifier::DotAbove as u8, 251);
    for m in Modifier::ALL {
        let code = m as u8;
        assert!(
            (cl8::consts::MODIFIER_FIRST..=cl8::consts::MODIFIER_LAST).contains(&code),
            "{m:?} имеет код {code} вне диапазона 238..=251"
        );
        assert_eq!(
            Modifier::from_byte(code),
            Some(m),
            "from_byte не обратна дискриминанту"
        );
    }
}

#[test]
fn modifier_from_byte_rejects_non_modifiers() {
    for b in [0u8, 10, 231, 232, 236, 237, 252, 255] {
        assert_eq!(Modifier::from_byte(b), None, "байт {b} не модификатор");
    }
}

#[test]
fn cedilla_and_comma_below_are_distinct() {
    // ş (U+015F) и ș (U+0219) — разные
    // кодпойнты, обязаны быть разными модификаторами. Ранее CSV назначал
    // 0x015F обоим, что стирало румынский.
    let cedilla: Vec<u16> = MODIFIER_TABLE
        .iter()
        .filter(|(m, _, _)| *m == Modifier::Cedilla)
        .map(|(_, _, r)| *r)
        .collect();
    let comma: Vec<u16> = MODIFIER_TABLE
        .iter()
        .filter(|(m, _, _)| *m == Modifier::CommaBelow)
        .map(|(_, _, r)| *r)
        .collect();
    assert!(cedilla.contains(&0x015F), "седиль должна давать ş U+015F");
    assert!(
        comma.contains(&0x0219),
        "запятая снизу должна давать ș U+0219"
    );
    for c in &cedilla {
        assert!(
            !comma.contains(c),
            "0x{c:04X} назначен и седили, и запятой снизу"
        );
    }
}

#[test]
fn capitalize_table_is_sorted_for_binary_search() {
    // tables::to_upper использует бинарный поиск; несортированная таблица
    // давала бы молча неверный результат, а не ошибку.
    for w in CAPITALIZE_TABLE.windows(2) {
        assert!(
            w[0].0 < w[1].0,
            "таблица капитализации не отсортирована: 0x{:04X} перед 0x{:04X}",
            w[0].0,
            w[1].0
        );
    }
}

#[test]
fn capitalize_table_count_is_documented() {
    // Ожидалось 88 (22+6+60). Фактически нужно 89: пропущена ё —
    // её кодпойнт U+0451 вне диапазона U+0430..U+044F, и алгоритмическое
    // правило -0x20 даёт U+0431 ('б'). Подробно: таблицах.
    assert_eq!(
        CAPITALIZE_TABLE_LEN, 89,
        "ожидается 89 = 22 (кириллица доп.) + 6 (нераскладываемые) + 60 (результаты \
         модификаторов) + 1 (ё); ожидалось 88, расхождение осознанное"
    );
}

#[test]
fn capitalize_covers_yo() {
    assert!(
        CAPITALIZE_TABLE.contains(&(0x0451, 0x0401)),
        "ё (U+0451) обязана быть в таблице: правило -0x20 даёт для неё 'б'"
    );
}

#[test]
fn capitalize_never_maps_to_itself() {
    for (lower, upper) in CAPITALIZE_TABLE {
        assert_ne!(
            lower, upper,
            "0x{lower:04X} отображается в себя — запись бессмысленна"
        );
    }
}

#[test]
fn capitalize_entries_are_reachable() {
    // Капитализировать символ, который невозможно закодировать, бессмысленно.
    for (lower, _) in CAPITALIZE_TABLE {
        let in_base = BASE_TABLE.contains(&lower);
        let in_mods = MODIFIER_TABLE.iter().any(|(_, _, r)| *r == lower);
        assert!(
            in_base || in_mods,
            "строчная 0x{lower:04X} недостижима: её нет ни в базе, ни среди результатов модификаторов"
        );
    }
}

#[test]
fn every_modifier_result_has_a_capital_form() {
    // case-shift обязан работать над любым собранным
    // результатом — значит все 60 обязаны иметь заглавную форму.
    for (m, b, result) in MODIFIER_TABLE {
        assert!(
            cl8::tables::to_upper(result.into()).is_some(),
            "{m:?}+{b} даёт 0x{result:04X} без заглавной формы"
        );
    }
}

#[test]
fn algorithmic_capitalization_covers_plain_letters() {
    // a..z и а..я обязаны капитализироваться правилом, без таблицы.
    for (lo, up) in [('a', 'A'), ('z', 'Z'), ('а', 'А'), ('я', 'Я')] {
        assert_eq!(
            cl8::tables::to_upper(lo as u32),
            Some(up as u32),
            "{lo} -> {up} должно покрываться алгоритмическим правилом"
        );
    }
    // И не должны дублироваться в таблице исключений.
    for c in ['a', 'z', 'а', 'я'] {
        assert!(
            !CAPITALIZE_TABLE.iter().any(|(l, _)| u32::from(*l) == c as u32),
            "{c} покрыт правилом и не должен быть в таблице исключений"
        );
    }
}

#[test]
fn lookup_helpers_are_mutually_inverse() {
    for (code, cp) in BASE_TABLE.iter().enumerate() {
        assert_eq!(cl8::tables::codepoint_of(code as u8), Some(u32::from(*cp)));
        assert_eq!(cl8::tables::code_of(u32::from(*cp)), Some(code as u8));
    }
}

#[test]
fn apply_modifier_matches_table() {
    for (m, base, result) in MODIFIER_TABLE {
        assert_eq!(cl8::tables::apply_modifier(m, base), Some(u32::from(result)));
    }
}

#[test]
fn apply_modifier_returns_none_for_undefined_pairs() {
    // Акут перед 'ж' (код 43) не определён.
    assert_eq!(cl8::tables::apply_modifier(Modifier::Acute, 43), None);
}

#[test]
fn spec_version_is_pinned() {
    // Смена версии формата обязана ломать сборку тестов, а не проходить молча.
    assert_eq!(cl8::SPEC_VERSION, "final-2026-08");
}

#[test]
fn reverse_capitalize_table_is_sorted_for_binary_search() {
    use cl8::tables::REVERSE_CAPITALIZE_TABLE;
    for w in REVERSE_CAPITALIZE_TABLE.windows(2) {
        assert!(
            w[0].0 < w[1].0,
            "обратная таблица капитализации не отсортирована: 0x{:04X} перед 0x{:04X}",
            w[0].0,
            w[1].0
        );
    }
}

#[test]
fn from_upper_to_lower_covers_all_capitalize_table_entries() {
    for &(lower, upper) in &CAPITALIZE_TABLE {
        // Исключение: турецкая ı (0x0131) капитализируется в I (0x0049),
        // но заглавная I по базовому правилу ASCII понижается до 'i' (0x0069).
        // Это зафиксированное в формат поведение.
        if lower == 0x0131 {
            assert_eq!(cl8::tables::from_upper_to_lower(upper.into()), Some(0x0069));
            continue;
        }
        assert_eq!(
            cl8::tables::from_upper_to_lower(upper.into()),
            Some(u32::from(lower)),
            "from_upper_to_lower не восстанавливает строчную форму для 0x{upper:04X} (ожидалось 0x{lower:04X})"
        );
    }
}

#[test]
fn capitalization_roundtrip_for_all_supported_languages() {
    // Проверяем все заглавные буквы и лигатуры европейских и славянских языков
    let sample_pairs = [
        ('A', 'a'),
        ('Z', 'z'),
        ('А', 'а'),
        ('Я', 'я'),
        ('Ё', 'ё'),
        ('І', 'і'),
        ('Є', 'є'),
        ('Ґ', 'ґ'),
        ('Ђ', 'ђ'),
        ('Ј', 'ј'),
        ('Љ', 'љ'),
        ('Њ', 'њ'),
        ('Ћ', 'ћ'),
        ('Џ', 'џ'),
        ('Ѕ', 'ѕ'),
        ('Ғ', 'ғ'),
        ('Қ', 'қ'),
        ('Ң', 'ң'),
        ('Ү', 'ү'),
        ('Ұ', 'ұ'),
        ('Ҳ', 'ҳ'),
        ('Ҷ', 'ҷ'),
        ('Һ', 'һ'),
        ('Ә', 'ә'),
        ('Ӣ', 'ӣ'),
        ('Ө', 'ө'),
        ('Ӯ', 'ӯ'),
        ('ẞ', 'ß'),
        ('Œ', 'œ'),
        ('Æ', 'æ'),
        ('Þ', 'þ'),
        ('Ð', 'ð'),
        ('Á', 'á'),
        ('É', 'é'),
        ('Í', 'í'),
        ('Ó', 'ó'),
        ('Ú', 'ú'),
        ('Ý', 'ý'),
        ('Ć', 'ć'),
        ('Ń', 'ń'),
        ('Ś', 'ś'),
        ('Ź', 'ź'),
        ('À', 'à'),
        ('È', 'è'),
        ('Ì', 'ì'),
        ('Ò', 'ò'),
        ('Ù', 'ù'),
        ('Â', 'â'),
        ('Ê', 'ê'),
        ('Î', 'î'),
        ('Ô', 'ô'),
        ('Û', 'û'),
        ('Ä', 'ä'),
        ('Ö', 'ö'),
        ('Ü', 'ü'),
        ('Ë', 'ë'),
        ('Ï', 'ï'),
        ('Ÿ', 'ÿ'),
        ('Ї', 'ї'),
        ('Ã', 'ã'),
        ('Õ', 'õ'),
        ('Ñ', 'ñ'),
        ('Č', 'č'),
        ('Ď', 'ď'),
        ('Ě', 'ě'),
        ('Ľ', 'ľ'),
        ('Ň', 'ň'),
        ('Ř', 'ř'),
        ('Š', 'š'),
        ('Ť', 'ť'),
        ('Ž', 'ž'),
        ('Ą', 'ą'),
        ('Ę', 'ę'),
        ('Ł', 'ł'),
        ('Đ', 'đ'),
        ('Ø', 'ø'),
        ('Ç', 'ç'),
        ('Ş', 'ş'),
        ('Ș', 'ș'),
        ('Ț', 'ț'),
        ('Ă', 'ă'),
        ('Ğ', 'ğ'),
        ('Ў', 'ў'),
        ('Ő', 'ő'),
        ('Ű', 'ű'),
        ('Å', 'å'),
        ('Ů', 'ů'),
        ('Ż', 'ż'),
    ];

    for (upper, lower) in sample_pairs {
        assert_eq!(
            cl8::tables::from_upper_to_lower(upper as u32),
            Some(lower as u32),
            "from_upper_to_lower('{upper}') -> '{lower}' failed"
        );
        assert_eq!(
            cl8::tables::to_upper(lower as u32),
            Some(upper as u32),
            "to_upper('{lower}') -> '{upper}' failed"
        );

        // Проверяем roundtrip через encode -> decode
        let s = upper.to_string();
        let mut enc_buf = [0u8; 16];
        let n = cl8::encode::encode_into(&s, &mut enc_buf).expect("encode failed");
        let mut dec_buf = [0u8; 16];
        let m = cl8::decode::decode_into(&enc_buf[..n], &mut dec_buf).expect("decode failed");
        let decoded = core::str::from_utf8(&dec_buf[..m]).expect("invalid utf-8");
        assert_eq!(decoded, s, "Roundtrip failed for upper letter '{upper}'");
    }
}

#[test]
fn caps_segmenter_reports_only_runs_the_encoder_acts_on() {
    use cl8::caps::{segment, CapsRun, CapsStrategy};

    let blank = CapsRun {
        start: 0,
        len: 0,
        strategy: CapsStrategy::Prefix,
    };

    // ASCII A-Z уже однобайтовые: CAPS-режим им ничего не экономит, раном они не являются.
    let mut runs = [blank; 10];
    let count = segment("Hello WORLD test ABC def", &mut runs).expect("сегментация");
    assert_eq!(
        count, 0,
        "прямые однобайтовые заглавные не образуют ранов — режим не дал бы выигрыша"
    );

    // Č требует CASE_SHIFT, но в одиночку: префикс дешевле пары переключателей.
    // ЄЇЂ — три подряд, выгоднее обернуть.
    let mut runs = [blank; 10];
    let count = segment("Čas ЄЇЂ zzz", &mut runs).expect("сегментация");
    assert_eq!(count, 2);
    assert_eq!((runs[0].start, runs[0].len), (0, 1));
    assert_eq!(runs[0].strategy, CapsStrategy::Prefix);
    assert_eq!((runs[1].start, runs[1].len), (4, 3));
    assert_eq!(runs[1].strategy, CapsStrategy::Toggle);

    // Нейтральные символы продлевают ран бесплатно: один ран накрывает пробел.
    let mut runs = [blank; 10];
    let count = segment("ŻÓŁĆ GĘŚLĄ", &mut runs).expect("сегментация");
    assert_eq!(count, 1);
    assert_eq!((runs[0].start, runs[0].len), (0, 10));
    assert_eq!(runs[0].strategy, CapsStrategy::Toggle);
}

#[test]
fn caps_segmenter_agrees_with_the_encoder() {
    use cl8::caps::{segment, CapsRun, CapsStrategy};
    use cl8::consts::CAPS_ON;

    // Публичный сегментатор и энкодер обязаны отвечать на один вопрос одинаково.
    // Расхождение здесь означало бы вторую реализацию правила — ровно тот класс
    // бага, из-за которого сегментатор раньше обещал Toggle для "WORLD".
    let blank = CapsRun {
        start: 0,
        len: 0,
        strategy: CapsStrategy::Prefix,
    };
    let cases = [
        "Hello WORLD test ABC def",
        "Čas ЄЇЂ zzz",
        "ŻÓŁĆ GĘŚLĄ JAŹŃ",
        "Če",
        "ČŠŽ",
        "ПРИВЕТ МОСКВА",
        "",
        "abc",
        "İstanbul",
    ];
    for text in cases {
        let mut runs = [blank; 32];
        let count = segment(text, &mut runs).expect("сегментация");
        let toggles = runs[..count]
            .iter()
            .filter(|r| r.strategy == CapsStrategy::Toggle)
            .count();

        let mut buf = vec![0u8; cl8::max_encoded_len(text.len())];
        let n = cl8::encode_into(text, &mut buf).expect("кодирование");
        let emitted = buf[..n].iter().filter(|&&b| b == CAPS_ON).count();

        assert_eq!(
            toggles, emitted,
            "{text:?}: сегментатор насчитал {toggles} Toggle-ранов, энкодер выдал {emitted} байт CAPS_ON"
        );
    }
}
