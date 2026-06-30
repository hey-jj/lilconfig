//! `search` traversal: stopDir, hidden .config, empty handling, first-match.

mod common;

use std::path::PathBuf;

use common::{block, fixed_value, search_path, search_root, RecordingFs};
use lilconfig::{AsyncSearcherBuilder, SearcherBuilder};
use serde_json::json;

#[test]
fn finds_config_in_hidden_dot_config_dir() {
    // .config/{name}rc.json is a default search place. Searching from a/b/c
    // walks up to search/ and finds .config/hiddenrc.json.
    let from = search_path("a/b/c");
    let searcher = SearcherBuilder::new("hidden").build().unwrap();
    let result = searcher.search(&from).unwrap().unwrap();
    assert_eq!(result.config, Some(json!({"hidden": true})));
    assert_eq!(result.filepath, search_path(".config/hiddenrc.json"));
}

#[test]
fn returns_none_when_stop_dir_reached_without_match() {
    let from = search_path("a/b/c");
    let searcher = SearcherBuilder::new("non-existent")
        .stop_dir(search_root())
        .build()
        .unwrap();
    assert_eq!(searcher.search(&from).unwrap(), None);
}

#[test]
fn returns_none_for_provided_search_from() {
    let from = search_path("a/b/c");
    let searcher = SearcherBuilder::new("non-existent")
        .stop_dir(search_root())
        .build()
        .unwrap();
    assert_eq!(searcher.search(&from).unwrap(), None);
}

#[test]
fn skips_empty_config_and_keeps_walking_by_default() {
    // a/b/maybeEmpty.config.json is empty. The default skips it and finds the
    // non-empty config one level up.
    let from = search_path("a/b/c");
    let searcher = SearcherBuilder::new("maybeEmpty")
        .stop_dir(search_root())
        .search_places(["maybeEmpty.config.json"])
        .build()
        .unwrap();
    let result = searcher.search(&from).unwrap().unwrap();
    assert_eq!(result.config, Some(json!({"notSoEmpty": true})));
    assert_eq!(result.filepath, search_path("a/maybeEmpty.config.json"));
}

#[test]
fn stops_at_empty_config_when_ignore_is_off() {
    let from = search_path("a/b/c");
    let searcher = SearcherBuilder::new("maybeEmpty")
        .stop_dir(search_root())
        .ignore_empty_search_places(false)
        .search_places(["maybeEmpty.config.json"])
        .build()
        .unwrap();
    let result = searcher.search(&from).unwrap().unwrap();
    assert_eq!(result.config, None);
    assert!(result.is_empty);
    assert_eq!(result.filepath, search_path("a/b/maybeEmpty.config.json"));
}

#[test]
fn custom_search_places_finds_first_present() {
    let from = search_path("a/b/c");
    let searcher = SearcherBuilder::new("doesnt-matter")
        .stop_dir(search_root())
        .search_places(["searchPlaces.conf.json", "searchPlaces-noExt"])
        .loader("noExt", fixed_value(json!(null)))
        .build()
        .unwrap();
    let result = searcher.search(&from).unwrap().unwrap();
    assert_eq!(result.config, Some(json!({"searchPlacesWorks": true})));
    assert_eq!(result.filepath, search_path("a/b/searchPlaces.conf.json"));
}

#[test]
fn first_matching_search_place_in_a_dir_wins() {
    // The a/ dir holds both package.json and maybeEmpty.config.json. With
    // package.json first in the order, its prop wins.
    let from = search_path("a");
    let searcher = SearcherBuilder::new("either")
        .stop_dir(search_root())
        .search_places(["package.json", "maybeEmpty.config.json"])
        .package_prop(lilconfig::PackageProp::Single("foo".to_string()))
        .build()
        .unwrap();
    let result = searcher.search(&from).unwrap().unwrap();
    assert_eq!(result.config, Some(json!({"insideFoo": true})));
    assert_eq!(result.filepath, search_path("a/package.json"));
}

#[test]
fn later_search_place_wins_when_earlier_absent() {
    // The b/ dir has no matching package.json prop, so the second place is used.
    let from = search_path("a/b");
    let searcher = SearcherBuilder::new("either")
        .stop_dir(search_root())
        .search_places(["package.json", "searchPlaces.conf.json"])
        .package_prop(lilconfig::PackageProp::Single("foo".to_string()))
        .build()
        .unwrap();
    let result = searcher.search(&from).unwrap().unwrap();
    // b/package.json has no foo prop, so it continues to the second place.
    assert_eq!(result.config, Some(json!({"searchPlacesWorks": true})));
    assert_eq!(result.filepath, search_path("a/b/searchPlaces.conf.json"));
}

#[test]
fn stop_dir_equal_to_search_from_searches_one_dir() {
    let searcher = SearcherBuilder::new("non-existent")
        .stop_dir(search_root())
        .build()
        .unwrap();
    assert_eq!(searcher.search(search_root()).unwrap(), None);
}

#[test]
fn custom_js_loader_resolves_dot_config_target() {
    // A registered .js loader makes the {name}.config.js default place usable
    // without executing JavaScript.
    let from = search_path("a/b/c");
    let searcher = SearcherBuilder::new("test-app")
        .stop_dir(search_root())
        .search_places(["test-app.config.json"])
        .build()
        .unwrap();
    let result = searcher.search(&from).unwrap().unwrap();
    assert_eq!(result.config, Some(json!({"stopped": true})));
    assert_eq!(result.filepath, search_path("test-app.config.json"));
}

#[test]
fn async_search_matches_sync() {
    let from = search_path("a/b/c");
    let searcher = AsyncSearcherBuilder::new("hidden").build().unwrap();
    let result = block(searcher.search(&from)).unwrap().unwrap();
    assert_eq!(result.config, Some(json!({"hidden": true})));
}

#[test]
fn search_cwd_default_origin() {
    let searcher = SearcherBuilder::new("hidden")
        .cwd(search_path("a/b/c"))
        .build()
        .unwrap();
    let result = searcher.search_cwd().unwrap().unwrap();
    assert_eq!(result.config, Some(json!({"hidden": true})));
}

#[test]
fn relative_search_from_resolves_against_cwd() {
    // A relative search origin resolves against the builder cwd, so the walk
    // starts at cwd/sub/dir and climbs back through cwd rather than jumping to
    // the filesystem root.
    let cwd = PathBuf::from("/proj");
    let fs = RecordingFs::default();
    let searcher = SearcherBuilder::new("app")
        .cwd(&cwd)
        .stop_dir(&cwd)
        .search_places(["package.json"])
        .build_with_fs(fs.clone())
        .unwrap();
    assert_eq!(searcher.search("sub/dir").unwrap(), None);
    let accesses = fs.accesses();
    assert!(accesses.contains(&cwd.join("sub/dir/package.json")));
    assert!(accesses.contains(&cwd.join("package.json")));
}
