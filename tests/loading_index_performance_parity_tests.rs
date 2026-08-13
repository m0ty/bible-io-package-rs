use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Barrier, OnceLock,
    },
    time::{Duration, Instant},
};

use bible_io::{
    Bible, BibleBook, BibleDataFormatError, BibleDataFormatErrorCode, BibleLoadOptions,
    BibleLoadPhase, BibleLoadProgress, BibleSource, SearchIndexMode, SearchMode, SearchOptions,
};
use serde_json::{json, Value};

static NEXT_TEMP_FILE_ID: AtomicU64 = AtomicU64::new(0);
static FULL_KJV: OnceLock<Arc<Bible>> = OnceLock::new();

struct TempJsonFile {
    path: PathBuf,
}

impl TempJsonFile {
    fn new(contents: &str) -> Self {
        let sequence = NEXT_TEMP_FILE_ID.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "bible-io-loading-parity-{}-{sequence}.json",
            std::process::id()
        ));
        fs::write(&path, contents).expect("temporary Bible fixture should be writable");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempJsonFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn minimal_bible_value() -> Value {
    json!({
        "schemaVersion": 1,
        "language": "English",
        "books": {
            "gn": {
                "name": "Genesis",
                "chapters": {
                    "1": {
                        "1": "In the beginning God created.",
                        "2": "And God said, Let there be light.",
                        "3": "Created light appears together."
                    }
                }
            }
        }
    })
}

fn options(search_index_mode: SearchIndexMode) -> BibleLoadOptions {
    BibleLoadOptions {
        search_index_mode,
        ..BibleLoadOptions::default()
    }
}

fn fixture_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/en_kjv.json")
}

fn full_kjv() -> Arc<Bible> {
    FULL_KJV
        .get_or_init(|| {
            Arc::new(
                Bible::from_path(fixture_path(), options(SearchIndexMode::Eager))
                    .expect("the checked-in KJV fixture should load"),
            )
        })
        .clone()
}

fn assert_source_is<E>(error: &BibleDataFormatError)
where
    E: Error + 'static,
{
    assert!(
        error
            .source()
            .and_then(|cause| cause.downcast_ref::<E>())
            .is_some(),
        "expected concrete cause {}, got {:?}",
        std::any::type_name::<E>(),
        error.source().map(ToString::to_string)
    );
}

fn assert_progress_contract(progress: &[BibleLoadProgress]) {
    assert_eq!(
        progress.iter().map(|value| value.phase).collect::<Vec<_>>(),
        [
            BibleLoadPhase::Reading,
            BibleLoadPhase::Reading,
            BibleLoadPhase::Processing,
            BibleLoadPhase::Processing,
            BibleLoadPhase::Complete,
        ]
    );
    assert_eq!(
        progress
            .iter()
            .map(|value| value.fraction)
            .collect::<Vec<_>>(),
        [0.0, 0.65, 0.65, 1.0, 1.0]
    );
    assert_eq!(
        progress
            .iter()
            .map(|value| value.phase_fraction)
            .collect::<Vec<_>>(),
        [0.0, 1.0, 0.0, 1.0, 1.0]
    );
    assert!(progress
        .windows(2)
        .all(|pair| pair[0].fraction <= pair[1].fraction));
}

#[test]
fn every_rust_loader_surface_produces_equivalent_content_and_source_provenance() {
    let value = minimal_bible_value();
    let text = serde_json::to_string(&value).unwrap();
    let bytes = text.as_bytes();
    let file = TempJsonFile::new(&text);
    let lazy = options(SearchIndexMode::Lazy);

    let compatibility = Bible::new(file.path().to_str().unwrap()).unwrap();
    let from_path = Bible::from_path(file.path(), lazy).unwrap();
    let from_str = Bible::from_json_str(&text).unwrap();
    let from_str_with_options = Bible::from_json_str_with_options(&text, lazy).unwrap();
    let from_bytes = Bible::from_json_slice(bytes).unwrap();
    let from_bytes_with_options = Bible::from_json_slice_with_options(bytes, lazy).unwrap();
    let from_value = Bible::from_json_value(value.clone()).unwrap();
    let from_value_with_options = Bible::from_json_value_with_options(value.clone(), lazy).unwrap();

    for loaded in [
        &from_path,
        &from_str,
        &from_str_with_options,
        &from_bytes,
        &from_bytes_with_options,
        &from_value,
        &from_value_with_options,
    ] {
        assert_eq!(loaded, &compatibility);
        assert_eq!(
            loaded.get_verse(BibleBook::Genesis, 1, 2).unwrap().text(),
            "And God said, Let there be light."
        );
    }
    assert_eq!(from_path.search_index_mode(), SearchIndexMode::Lazy);
    assert_eq!(
        from_str_with_options.search_index_mode(),
        SearchIndexMode::Lazy
    );
    assert_eq!(
        from_bytes_with_options.search_index_mode(),
        SearchIndexMode::Lazy
    );
    assert_eq!(
        from_value_with_options.search_index_mode(),
        SearchIndexMode::Lazy
    );

    let source = BibleSource::from_asset_path("bible_io_json/English/kjv.json");
    let from_path_with_source = Bible::from_path_with_source(file.path(), &source, lazy).unwrap();
    let from_str_with_source = Bible::from_json_str_with_source(&text, &source, lazy).unwrap();
    let from_bytes_with_source = Bible::from_json_slice_with_source(bytes, &source, lazy).unwrap();
    let from_value_with_source = Bible::from_json_value_with_source(value, &source, lazy).unwrap();

    for loaded in [
        &from_path_with_source,
        &from_str_with_source,
        &from_bytes_with_source,
        &from_value_with_source,
    ] {
        assert_eq!(loaded, &from_path_with_source);
        assert_eq!(loaded.source(), Some(&source));
        assert_eq!(loaded.id(), source.id);
        assert_eq!(loaded.search_index_mode(), SearchIndexMode::Lazy);
    }
}

#[test]
fn loader_failures_preserve_codes_paths_values_and_concrete_causes() {
    let missing_path = std::env::temp_dir().join(format!(
        "bible-io-certainly-missing-{}-{}.json",
        std::process::id(),
        NEXT_TEMP_FILE_ID.fetch_add(1, Ordering::Relaxed)
    ));
    let path_error = Bible::from_path(&missing_path, BibleLoadOptions::default()).unwrap_err();
    assert_eq!(path_error.code(), BibleDataFormatErrorCode::InvalidJson);
    assert_eq!(path_error.path(), "$");
    assert_eq!(path_error.value(), None);
    assert!(path_error.cause().is_some());
    assert_source_is::<std::io::Error>(&path_error);

    let json_error = Bible::from_json_str("{ definitely not JSON").unwrap_err();
    assert_eq!(json_error.code(), BibleDataFormatErrorCode::InvalidJson);
    assert_eq!(json_error.path(), "$");
    assert_eq!(json_error.value(), None);
    assert!(json_error.cause().is_some());
    assert_source_is::<serde_json::Error>(&json_error);

    let source = BibleSource::from_asset_path("bible_io_json/English/kjv.json");
    let sourced_json_error = Bible::from_json_str_with_source(
        "{ definitely not JSON",
        &source,
        BibleLoadOptions::default(),
    )
    .unwrap_err();
    assert_eq!(
        sourced_json_error.code(),
        BibleDataFormatErrorCode::InvalidJson
    );
    assert_eq!(sourced_json_error.path(), "$");
    assert_source_is::<serde_json::Error>(&sourced_json_error);

    let utf8_error = Bible::from_json_slice(&[0xff, 0xfe]).unwrap_err();
    assert_eq!(utf8_error.code(), BibleDataFormatErrorCode::InvalidJson);
    assert_eq!(utf8_error.path(), "$");
    assert_eq!(utf8_error.value(), None);
    assert!(utf8_error.cause().is_some());
    assert_source_is::<std::str::Utf8Error>(&utf8_error);

    let wrong_root = json!(["not", "an", "object"]);
    let type_error = Bible::from_json_value(wrong_root.clone()).unwrap_err();
    assert_eq!(type_error.code(), BibleDataFormatErrorCode::InvalidType);
    assert_eq!(type_error.path(), "$");
    assert_eq!(type_error.value(), Some(&wrong_root));
    assert_eq!(type_error.cause(), None);
    assert!(type_error.source().is_none());

    let wrong_books = json!({"books": []});
    let books_error = Bible::from_json_value(wrong_books).unwrap_err();
    assert_eq!(books_error.code(), BibleDataFormatErrorCode::InvalidType);
    assert_eq!(books_error.path(), "$.books");
    assert_eq!(books_error.value(), Some(&json!([])));

    let missing_books = Bible::from_json_value(json!({"invalid": "structure"})).unwrap_err();
    assert_eq!(missing_books.code(), BibleDataFormatErrorCode::MissingField);
    assert_eq!(missing_books.path(), "$.books");
}

#[test]
fn path_loaders_report_stable_monotonic_progress_with_and_without_source() {
    let text = serde_json::to_string(&minimal_bible_value()).unwrap();
    let file = TempJsonFile::new(&text);
    let mut progress = Vec::new();
    let plain =
        Bible::from_path_with_progress(file.path(), options(SearchIndexMode::Lazy), |value| {
            progress.push(value)
        })
        .unwrap();
    assert_progress_contract(&progress);
    assert!(plain.performance_metrics().load_time > Duration::ZERO);

    let source = BibleSource::from_asset_path("bible_io_json/English/kjv.json");
    let mut sourced_progress = Vec::new();
    let sourced = Bible::from_path_with_source_and_progress(
        file.path(),
        &source,
        options(SearchIndexMode::Lazy),
        |value| sourced_progress.push(value),
    )
    .unwrap();
    assert_progress_contract(&sourced_progress);
    assert_eq!(sourced.source(), Some(&source));
}

#[test]
fn eager_lazy_and_disabled_indexes_have_complete_rebuild_and_search_parity() {
    let eager =
        Bible::from_json_value_with_options(minimal_bible_value(), options(SearchIndexMode::Eager))
            .unwrap();
    let lazy =
        Bible::from_json_value_with_options(minimal_bible_value(), options(SearchIndexMode::Lazy))
            .unwrap();
    let disabled = Bible::from_json_value_with_options(
        minimal_bible_value(),
        options(SearchIndexMode::Disabled),
    )
    .unwrap();

    assert!(eager.has_search_index());
    assert!(!lazy.has_search_index());
    assert!(!disabled.has_search_index());

    let scanned = disabled.search("created");
    assert_eq!(eager.search("created"), scanned);
    assert_eq!(lazy.search("created"), scanned);
    assert!(lazy.has_search_index());
    assert!(!disabled.has_search_index());

    let advanced_options = SearchOptions {
        mode: SearchMode::All,
        whole_words: true,
        ..SearchOptions::default()
    };
    let scanned_advanced = disabled
        .search_with_options("created light", &advanced_options)
        .unwrap();
    assert_eq!(
        eager
            .search_with_options("created light", &advanced_options)
            .unwrap(),
        scanned_advanced
    );
    assert_eq!(
        lazy.search_with_options("created light", &advanced_options)
            .unwrap(),
        scanned_advanced
    );

    lazy.clear_search_index();
    assert!(!lazy.has_search_index());
    let rebuilt = lazy.build_search_index();
    assert!(!rebuilt.is_empty());
    assert_eq!(rebuilt.search("created").len(), scanned.len());
    assert!(
        !lazy.has_search_index(),
        "building a reusable detached index must not silently retain it"
    );
    lazy.prewarm_search_index();
    assert!(lazy.has_search_index());

    eager.clear_search_index();
    assert!(!eager.has_search_index());
    assert_eq!(eager.search("created"), scanned);
    assert!(eager.has_search_index());

    disabled.prewarm_search_index();
    assert!(!disabled.has_search_index());
    assert_eq!(disabled.search("created"), scanned);
    assert!(!disabled.has_search_index());

    let eager_metrics = eager.performance_metrics();
    assert!(eager_metrics.search_index_built);
    assert!(eager_metrics.search_index_size > 0);
    assert!(eager_metrics.posting_count > 0);
    let disabled_metrics = disabled.performance_metrics();
    assert!(!disabled_metrics.search_index_built);
    assert_eq!(disabled_metrics.search_index_size, 0);
    assert_eq!(disabled_metrics.posting_count, 0);
    assert_eq!(disabled_metrics.verse_count, eager_metrics.verse_count);
    assert_eq!(disabled_metrics.text_bytes, eager_metrics.text_bytes);
}

#[test]
fn full_fixture_boundaries_metrics_and_indexed_search_are_populated_and_bounded() {
    let bible = full_kjv();

    assert_eq!(bible.books().len(), 66);
    assert_eq!(bible.get_book_by_id(1).unwrap().title(), "Genesis");
    assert_eq!(bible.get_book_by_id(66).unwrap().title(), "Revelation");
    assert_eq!(
        bible.get_chapter(BibleBook::Genesis, 1).unwrap().number(),
        1
    );
    assert_eq!(
        bible
            .get_chapter(BibleBook::Revelation, 22)
            .unwrap()
            .number(),
        22
    );
    assert_eq!(
        bible.get_verse(BibleBook::Genesis, 1, 1).unwrap().number(),
        1
    );
    assert_eq!(
        bible
            .get_verse(BibleBook::Revelation, 22, 21)
            .unwrap()
            .number(),
        21
    );

    let metrics = bible.performance_metrics();
    assert!(metrics.load_time > Duration::ZERO);
    assert!(metrics.search_index_built);
    assert!(metrics.search_index_size > 0);
    // Independent raw counts of the Dart package's and this repository's
    // checked-in KJV fixtures both produce 31,100; do not substitute a
    // generic KJV versification total for the actual parity fixtures.
    assert_eq!(metrics.verse_count, 31_100);
    assert!(metrics.posting_count > metrics.verse_count);
    assert!(metrics.text_bytes > 0);
    assert!(metrics.memory_usage_kib > 0);

    let started = Instant::now();
    let results = bible.search("the");
    let elapsed = started.elapsed();
    assert!(results.len() > 1_000);
    assert!(
        elapsed < Duration::from_secs(5),
        "indexed full-fixture search took {elapsed:?}"
    );
}

#[test]
fn immutable_full_fixture_access_is_safe_and_consistent_across_threads() {
    const THREADS: usize = 8;
    let bible = full_kjv();
    let barrier = Arc::new(Barrier::new(THREADS + 1));
    let mut workers = Vec::with_capacity(THREADS);

    for _ in 0..THREADS {
        let bible = Arc::clone(&bible);
        let barrier = Arc::clone(&barrier);
        workers.push(std::thread::spawn(move || {
            barrier.wait();
            for _ in 0..16 {
                assert_eq!(
                    bible.get_book(BibleBook::Genesis).unwrap().title(),
                    "Genesis"
                );
                assert_eq!(
                    bible.get_verse(BibleBook::Genesis, 1, 1).unwrap().text(),
                    "In the beginning God created the heaven and the earth."
                );
                let results = bible.search("beginning God");
                assert!(!results.is_empty());
                assert!(bible.performance_metrics().search_index_built);
            }
            Arc::as_ptr(&bible) as usize
        }));
    }

    barrier.wait();
    let shared_address = Arc::as_ptr(&bible) as usize;
    for worker in workers {
        assert_eq!(
            worker.join().expect("reader thread must not panic"),
            shared_address
        );
    }
}
