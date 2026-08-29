//! Проверка полноты и валидности тестового корпуса.
//!
//! Читает `tests/corpus/MANIFEST.tsv` и сверяет с фактическим содержимым
//! каталога. Тест НЕ падает при неполном корпусе — иначе вся сборка была бы
//! красной до окончания сбора текстов, который является отдельной задачей.
//! Он падает, если файл присутствует, но нарушает требования
//! `agent3_corpus.md`: BOM, невалидный UTF-8, отсутствие пометки о NFC.

use std::fs;
use std::path::{Path, PathBuf};

fn corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/corpus")
}

/// `(имя файла, язык)` для всех записей манифеста.
fn manifest_entries() -> Vec<(String, String)> {
    let path = corpus_dir().join("MANIFEST.tsv");
    let text = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("не прочитать {}: {e}", path.display()));
    text.lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .filter_map(|l| {
            let mut parts = l.split('\t');
            Some((parts.next()?.to_string(), parts.next()?.to_string()))
        })
        .collect()
}

#[test]
fn manifest_is_parseable_and_non_empty() {
    let entries = manifest_entries();
    assert!(!entries.is_empty(), "манифест не содержит записей");
    for (file, lang) in &entries {
        assert!(file.ends_with(".txt"), "{file}: ожидалось расширение .txt");
        assert!(!lang.trim().is_empty(), "{file}: пустое имя языка");
    }
}

#[test]
fn manifest_has_no_duplicate_files() {
    let mut files: Vec<String> = manifest_entries().into_iter().map(|(f, _)| f).collect();
    files.sort();
    let before = files.len();
    files.dedup();
    assert_eq!(
        before,
        files.len(),
        "в манифесте есть дублирующиеся имена файлов"
    );
}

#[test]
fn manifest_lists_the_named_languages_from_spec_section_11() {
    // 38 поимённо названных языков + 5 стресс-файлов.
    //
    // Раздел 11 спеки в заголовке заявляет 41 язык, но поимённо называет 38
    // (группа диакритики помечена «(29)», перечисляет 26). Расхождение
    // зафиксировано в шапке манифеста и в docs/DATA_TABLES.md; здесь
    // проверяется ровно то, что реально перечислено, без домысливания.
    let entries = manifest_entries();
    let stress = [
        "mixed.txt",
        "fallback-cjk.txt",
        "fallback-arabic.txt",
        "code-sample.txt",
        "acronyms.txt",
    ];
    let languages = entries
        .iter()
        .filter(|(f, _)| !stress.contains(&f.as_str()))
        .count();
    assert_eq!(
        languages, 38,
        "ожидалось 38 языковых файлов — столько языков раздел 11 называет поимённо"
    );
    for s in stress {
        assert!(
            entries.iter().any(|(f, _)| f == s),
            "стресс-файл {s} отсутствует в манифесте"
        );
    }
}

#[test]
fn mixed_file_is_present_in_manifest() {
    // Раздел 12 спеки: смешанный текст — главный аргумент питча, и он же
    // выделен особо в agent4_benchmarks.md. Потерять его нельзя.
    assert!(
        manifest_entries().iter().any(|(f, _)| f == "mixed.txt"),
        "mixed.txt обязан быть в манифесте"
    );
}

#[test]
fn present_corpus_files_satisfy_requirements() {
    // Не требует наличия файлов, но проверяет каждый существующий.
    let dir = corpus_dir();
    let mut checked = 0usize;
    for (file, lang) in manifest_entries() {
        let path = dir.join(&file);
        if !path.exists() {
            continue;
        }
        checked += 1;
        let bytes = fs::read(&path).unwrap_or_else(|e| panic!("{file}: {e}"));

        assert!(
            !bytes.starts_with(&[0xEF, 0xBB, 0xBF]),
            "{file} ({lang}): найден BOM — раздел 10.3 требует строгий UTF-8 без BOM"
        );
        let text = std::str::from_utf8(&bytes)
            .unwrap_or_else(|e| panic!("{file} ({lang}): невалидный UTF-8: {e}"));
        assert!(
            text.lines().next().is_some_and(|l| l.contains("NFC-normalized")),
            "{file} ({lang}): первая строка обязана содержать пометку `# NFC-normalized YYYY-MM-DD` \
             — agent3_corpus.md требует подтверждения выполненной нормализации"
        );
        let content_lines = text.lines().filter(|l| !l.starts_with('#')).count();
        assert!(
            content_lines >= 50,
            "{file} ({lang}): {content_lines} содержательных строк, минимум 50"
        );
    }
    eprintln!(
        "корпус: проверено {checked} файлов из {} в манифесте",
        manifest_entries().len()
    );
}

#[test]
fn report_corpus_completeness() {
    // Диагностика, не проверка: печатает, чего не хватает. Всегда зелёный.
    let dir = corpus_dir();
    let entries = manifest_entries();
    let missing: Vec<&str> = entries
        .iter()
        .filter(|(f, _)| !dir.join(f).exists())
        .map(|(f, _)| f.as_str())
        .collect();
    if missing.is_empty() {
        eprintln!("корпус полон: {} файлов", entries.len());
    } else {
        eprintln!(
            "корпус неполон: {} из {} файлов отсутствуют.\nНе хватает: {}",
            missing.len(),
            entries.len(),
            missing.join(", ")
        );
        eprintln!("Это ожидаемо до завершения задачи агента 3 — см. tests/corpus/README.md");
    }
}
