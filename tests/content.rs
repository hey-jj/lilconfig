//! File content decoding and emptiness through load.
//!
//! Reads decode bytes as UTF-8 and replace invalid sequences. Emptiness trims
//! the Unicode whitespace set plus the BOM, so a BOM-only or NBSP-only file
//! counts as empty.

mod common;

use common::{load_path, passthrough};
use lilconfig::SearcherBuilder;
use serde_json::Value;

#[test]
fn invalid_utf8_decodes_lossily_to_replacement_char() {
    // The file holds bytes [0x61, 0xFF, 0x62]: 'a', an invalid byte, 'b'. The
    // read replaces the invalid byte with U+FFFD and the loader sees the lossy
    // string.
    let path = load_path("invalid-utf8");
    let searcher = SearcherBuilder::new("invalid-utf8")
        .loader("noExt", passthrough())
        .build()
        .unwrap();
    let result = searcher.load(&path).unwrap().unwrap();
    assert_eq!(result.config, Some(Value::String("a\u{FFFD}b".to_string())));
}

#[test]
fn bom_only_file_counts_as_empty() {
    // A file whose only content is the BOM U+FEFF trims to nothing. str::trim
    // alone would keep the BOM, so the blank check strips it to match.
    let path = load_path("bom-only.json");
    let searcher = SearcherBuilder::new("bom")
        .ignore_empty_search_places(false)
        .build()
        .unwrap();
    let result = searcher.load(&path).unwrap().unwrap();
    assert!(result.is_empty);
    assert_eq!(result.config, None);
}

#[test]
fn nbsp_only_file_counts_as_empty() {
    // A file whose only content is U+00A0 (a non-breaking space) trims to
    // nothing, since NBSP is in the Unicode whitespace set.
    let path = load_path("nbsp-only.json");
    let searcher = SearcherBuilder::new("nbsp")
        .ignore_empty_search_places(false)
        .build()
        .unwrap();
    let result = searcher.load(&path).unwrap().unwrap();
    assert!(result.is_empty);
    assert_eq!(result.config, None);
}
