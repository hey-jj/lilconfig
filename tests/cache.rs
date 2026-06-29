//! Cache lifecycle, asserted through filesystem call counts.
//!
//! The reference suite mocks `fs` and asserts exact `access`/`readFile` counts.
//! Here a `CountingFs` wraps the real filesystem and tallies the same calls.

mod common;

use common::{block, search_path, search_root};
use lilconfig::{AsyncLilconfig, CountingFs, Lilconfig, RealFs};
use serde_json::json;

/// Search places used by the cache tests. cached.config.json sits at the search
/// root, so the walk checks every ancestor before matching.
const PLACES: [&str; 2] = ["cached.config.json", "package.json"];

#[test]
fn search_cache_lifecycle_sync() {
    let from = search_path("a/b/c");
    let fs = CountingFs::new(RealFs);
    let searcher = Lilconfig::new("cached")
        .stop_dir(search_root())
        .search_places(PLACES)
        .cache(true)
        .build_with_fs(fs)
        .unwrap();

    let lookups = || searcher.fs().access_count();
    let expected = 7;

    assert_eq!(lookups(), 0);

    let r1 = searcher.search(&from).unwrap();
    assert_eq!(lookups(), expected);
    assert_eq!(r1, Some(found()));

    // Repeat search reads from cache, no new lookups.
    let r2 = searcher.search(&from).unwrap();
    assert_eq!(lookups(), expected);
    assert_eq!(r1, r2);

    // Subpaths reuse the cache.
    let r3 = searcher.search(search_path("a")).unwrap();
    let r4 = searcher.search(search_path("a/b")).unwrap();
    assert_eq!(lookups(), expected);
    assert_eq!(r2, r3);
    assert_eq!(r3, r4);

    // clearCaches empties the search cache, forcing fresh lookups.
    searcher.clear_caches();
    let r5 = searcher.search(&from).unwrap();
    assert_eq!(lookups(), expected * 2);
    assert_eq!(r4, r5);

    searcher.clear_search_cache();
    let r6 = searcher.search(&from).unwrap();
    assert_eq!(lookups(), expected * 3);
    assert_eq!(r5, r6);

    // clearLoadCache does not touch the search cache.
    searcher.clear_load_cache();
    let r7 = searcher.search(&from).unwrap();
    assert_eq!(lookups(), expected * 3);
    assert_eq!(r6, r7);

    // A superset path checks fs until it hits a cached ancestor.
    let r8 = searcher.search(search_path("a/b/c/d")).unwrap();
    assert_eq!(lookups(), 3 * expected + 2);
    assert_eq!(r7, r8);

    // Repeating the superset search adds no lookups.
    let r9 = searcher.search(search_path("a/b/c/d")).unwrap();
    assert_eq!(lookups(), 3 * expected + 2);
    assert_eq!(r8, r9);
}

#[test]
fn search_cache_lifecycle_async() {
    let from = search_path("a/b/c");
    let fs = CountingFs::new(RealFs);
    let searcher = AsyncLilconfig::new("cached")
        .stop_dir(search_root())
        .search_places(PLACES)
        .cache(true)
        .build_with_fs(fs)
        .unwrap();

    let lookups = || searcher.fs().access_count();
    let expected = 7;

    assert_eq!(lookups(), 0);
    let r1 = block(searcher.search(&from)).unwrap();
    assert_eq!(lookups(), expected);
    assert_eq!(r1, Some(found()));

    block(searcher.search(&from)).unwrap();
    assert_eq!(lookups(), expected);

    searcher.clear_caches();
    block(searcher.search(&from)).unwrap();
    assert_eq!(lookups(), expected * 2);

    block(searcher.search(search_path("a/b/c/d"))).unwrap();
    assert_eq!(lookups(), expected * 2 + 2);
}

#[test]
fn load_cache_lifecycle_sync() {
    let fs = CountingFs::new(RealFs);
    let searcher = Lilconfig::new("cached")
        .stop_dir(search_root())
        .search_places(PLACES)
        .cache(true)
        .build_with_fs(fs)
        .unwrap();
    let existing = search_path("cached.config.json");
    let path = existing.to_str().unwrap();
    let reads = || searcher.fs().read_count();

    assert_eq!(reads(), 0);
    let r1 = searcher.load(path).unwrap();
    assert_eq!(reads(), 1);

    let r2 = searcher.load(path).unwrap();
    assert_eq!(reads(), 1);
    assert_eq!(r1, r2);

    searcher.clear_caches();
    let r3 = searcher.load(path).unwrap();
    assert_eq!(reads(), 2);
    assert_eq!(r2, r3);

    searcher.clear_load_cache();
    let r4 = searcher.load(path).unwrap();
    assert_eq!(reads(), 3);
    assert_eq!(r3, r4);

    // clearSearchCache does not touch the load cache.
    searcher.clear_search_cache();
    let r5 = searcher.load(path).unwrap();
    assert_eq!(reads(), 3);
    assert_eq!(r4, r5);
}

#[test]
fn cache_disabled_redoes_all_work_sync() {
    let from = search_path("a/b/c");
    let fs = CountingFs::new(RealFs);
    let searcher = Lilconfig::new("cached")
        .stop_dir(search_root())
        .search_places(PLACES)
        .cache(false)
        .build_with_fs(fs)
        .unwrap();
    let lookups = || searcher.fs().access_count();

    let r1 = searcher.search(&from).unwrap();
    assert_eq!(lookups(), 7);
    let r2 = searcher.search(&from).unwrap();
    assert_eq!(lookups(), 14);
    assert_eq!(r1, r2);
}

#[test]
fn cache_disabled_load_rereads_sync() {
    let fs = CountingFs::new(RealFs);
    let searcher = Lilconfig::new("cached")
        .cache(false)
        .build_with_fs(fs)
        .unwrap();
    let existing = search_path("cached.config.json");
    let path = existing.to_str().unwrap();
    let reads = || searcher.fs().read_count();

    searcher.load(path).unwrap();
    assert_eq!(reads(), 1);
    searcher.load(path).unwrap();
    assert_eq!(reads(), 2);
}

#[test]
fn negative_search_result_is_cached() {
    let from = search_path("a/b/c");
    let fs = CountingFs::new(RealFs);
    let searcher = Lilconfig::new("non-existent")
        .stop_dir(search_root())
        .search_places(["non-existent.config.json"])
        .cache(true)
        .build_with_fs(fs)
        .unwrap();
    let lookups = || searcher.fs().access_count();

    // c, b, a, search each check one place: 4 lookups, no match.
    assert_eq!(searcher.search(&from).unwrap(), None);
    let first = lookups();
    assert_eq!(first, 4);

    // The miss is cached, so a repeat does no fs work.
    assert_eq!(searcher.search(&from).unwrap(), None);
    assert_eq!(lookups(), first);
}

/// The config the cache tests expect to find.
fn found() -> lilconfig::SearchResult {
    lilconfig::SearchResult {
        config: Some(json!({"iWasCached": true})),
        filepath: search_path("cached.config.json"),
        is_empty: false,
    }
}
