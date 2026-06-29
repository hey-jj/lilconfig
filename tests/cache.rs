//! Cache lifecycle, asserted through filesystem call counts.
//!
//! A `CountingFs` wraps the real filesystem and tallies `access`/`read` calls.
//! Its counters are shared across clones, so the test keeps a clone to read
//! counts while the searcher owns the other.

mod common;

use common::{block, search_path, search_root, CountingFs};
use lilconfig::{AsyncSearcherBuilder, RealFs, SearcherBuilder};
use serde_json::json;

/// Search places used by the cache tests. cached.config.json sits at the search
/// root, so the walk checks every ancestor before matching.
const PLACES: [&str; 2] = ["cached.config.json", "package.json"];

#[test]
fn search_cache_lifecycle_sync() {
    let from = search_path("a/b/c");
    let fs = CountingFs::new(RealFs);
    let counter = fs.clone();
    let searcher = SearcherBuilder::new("cached")
        .stop_dir(search_root())
        .search_places(PLACES)
        .cache(true)
        .build_with_fs(fs)
        .unwrap();

    let lookups = || counter.access_count();
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
    let counter = fs.clone();
    let searcher = AsyncSearcherBuilder::new("cached")
        .stop_dir(search_root())
        .search_places(PLACES)
        .cache(true)
        .build_with_fs(fs)
        .unwrap();

    let lookups = || counter.access_count();
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
    let counter = fs.clone();
    let searcher = SearcherBuilder::new("cached")
        .stop_dir(search_root())
        .search_places(PLACES)
        .cache(true)
        .build_with_fs(fs)
        .unwrap();
    let path = search_path("cached.config.json");
    let reads = || counter.read_count();

    assert_eq!(reads(), 0);
    let r1 = searcher.load(&path).unwrap();
    assert_eq!(reads(), 1);

    let r2 = searcher.load(&path).unwrap();
    assert_eq!(reads(), 1);
    assert_eq!(r1, r2);

    searcher.clear_caches();
    let r3 = searcher.load(&path).unwrap();
    assert_eq!(reads(), 2);
    assert_eq!(r2, r3);

    searcher.clear_load_cache();
    let r4 = searcher.load(&path).unwrap();
    assert_eq!(reads(), 3);
    assert_eq!(r3, r4);

    // clearSearchCache does not touch the load cache.
    searcher.clear_search_cache();
    let r5 = searcher.load(&path).unwrap();
    assert_eq!(reads(), 3);
    assert_eq!(r4, r5);
}

#[test]
fn sync_load_does_not_cache_package_json() {
    // The synchronous load returns a package.json result without writing it to
    // the load cache, so a second load of the same file reads it again.
    let fs = CountingFs::new(RealFs);
    let counter = fs.clone();
    let searcher = SearcherBuilder::new("test-app")
        .cache(true)
        .build_with_fs(fs)
        .unwrap();
    let path = search_path("a/package.json");
    let reads = || counter.read_count();

    let r1 = searcher.load(&path).unwrap();
    assert_eq!(reads(), 1);
    let r2 = searcher.load(&path).unwrap();
    assert_eq!(reads(), 2);
    assert_eq!(r1, r2);
}

#[test]
fn async_load_caches_package_json() {
    // The asynchronous load caches the package.json result, so a second load of
    // the same file serves from cache without a new read.
    let fs = CountingFs::new(RealFs);
    let counter = fs.clone();
    let searcher = AsyncSearcherBuilder::new("test-app")
        .cache(true)
        .build_with_fs(fs)
        .unwrap();
    let path = search_path("a/package.json");
    let reads = || counter.read_count();

    let r1 = block(searcher.load(&path)).unwrap();
    assert_eq!(reads(), 1);
    let r2 = block(searcher.load(&path)).unwrap();
    assert_eq!(reads(), 1);
    assert_eq!(r1, r2);
}

#[test]
fn cache_disabled_redoes_all_work_sync() {
    let from = search_path("a/b/c");
    let fs = CountingFs::new(RealFs);
    let counter = fs.clone();
    let searcher = SearcherBuilder::new("cached")
        .stop_dir(search_root())
        .search_places(PLACES)
        .cache(false)
        .build_with_fs(fs)
        .unwrap();
    let lookups = || counter.access_count();

    let r1 = searcher.search(&from).unwrap();
    assert_eq!(lookups(), 7);
    let r2 = searcher.search(&from).unwrap();
    assert_eq!(lookups(), 14);
    assert_eq!(r1, r2);
}

#[test]
fn cache_disabled_load_rereads_sync() {
    let fs = CountingFs::new(RealFs);
    let counter = fs.clone();
    let searcher = SearcherBuilder::new("cached")
        .cache(false)
        .build_with_fs(fs)
        .unwrap();
    let path = search_path("cached.config.json");
    let reads = || counter.read_count();

    searcher.load(&path).unwrap();
    assert_eq!(reads(), 1);
    searcher.load(&path).unwrap();
    assert_eq!(reads(), 2);
}

#[test]
fn negative_search_result_is_cached() {
    let from = search_path("a/b/c");
    let fs = CountingFs::new(RealFs);
    let counter = fs.clone();
    let searcher = SearcherBuilder::new("non-existent")
        .stop_dir(search_root())
        .search_places(["non-existent.config.json"])
        .cache(true)
        .build_with_fs(fs)
        .unwrap();
    let lookups = || counter.access_count();

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
