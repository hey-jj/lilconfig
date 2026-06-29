//! The public surface: both searchers expose the same methods, root traversal
//! works, and sync and async return equal results.

mod common;

use common::{block, search_path, search_root};
use lilconfig::{AsyncSearcherBuilder, SearcherBuilder};
use serde_json::json;

#[test]
fn searcher_exposes_all_methods() {
    let searcher = SearcherBuilder::new("test-app").build().unwrap();
    // Methods exist and accept the documented forms.
    searcher.clear_load_cache();
    searcher.clear_search_cache();
    searcher.clear_caches();
    let _ = searcher.load(search_path("cached.config.json"));
    let _ = searcher.search(search_root());
    let _ = searcher.search_cwd();
}

#[test]
fn async_searcher_exposes_all_methods() {
    let searcher = AsyncSearcherBuilder::new("test-app").build().unwrap();
    searcher.clear_load_cache();
    searcher.clear_search_cache();
    searcher.clear_caches();
    let _ = block(searcher.load(search_path("cached.config.json")));
    let _ = block(searcher.search(search_root()));
    let _ = block(searcher.search_cwd());
}

#[test]
fn traversal_reaches_filesystem_root_without_panic() {
    // With no stop dir set to a fixture, walking from a deep path must terminate
    // at the filesystem root rather than loop forever.
    let searcher = SearcherBuilder::new("definitely-no-such-config-xyz")
        .stop_dir(std::path::MAIN_SEPARATOR.to_string())
        .build()
        .unwrap();
    let from = search_path("a/b/c");
    assert_eq!(searcher.search(&from).unwrap(), None);
}

#[test]
fn sync_and_async_agree_on_found_config() {
    let from = search_path("a/b/c");
    let sync = SearcherBuilder::new("hidden").build().unwrap();
    let asy = AsyncSearcherBuilder::new("hidden").build().unwrap();

    let s = sync.search(&from).unwrap();
    let a = block(asy.search(&from)).unwrap();
    assert_eq!(s, a);
    assert_eq!(common::config(&s), Some(&json!({"hidden": true})));
}

#[test]
fn sync_and_async_agree_on_not_found() {
    let from = search_path("a/b/c");
    let sync = SearcherBuilder::new("non-existent")
        .stop_dir(search_root())
        .build()
        .unwrap();
    let asy = AsyncSearcherBuilder::new("non-existent")
        .stop_dir(search_root())
        .build()
        .unwrap();
    assert_eq!(sync.search(&from).unwrap(), None);
    assert_eq!(block(asy.search(&from)).unwrap(), None);
}
