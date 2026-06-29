//! `load` of a single file: JSON, no-extension JSON, package.json, empty files.

mod common;

use common::{block, load_path, passthrough};
use lilconfig::{AsyncSearcherBuilder, PackageProp, SearcherBuilder};
use serde_json::json;

#[test]
fn loads_existing_json_file() {
    let path = load_path("test-app.json");
    let searcher = SearcherBuilder::new("test-app").build().unwrap();
    let result = searcher.load(&path).unwrap().unwrap();
    assert_eq!(result.config, Some(json!({"jsonTest": true})));
    assert_eq!(result.filepath, path);
    assert!(!result.is_empty);
}

#[test]
fn loads_no_extension_json_file() {
    let path = load_path("test-noExt-json");
    let searcher = SearcherBuilder::new("test-app").build().unwrap();
    let result = searcher.load(&path).unwrap().unwrap();
    assert_eq!(result.config, Some(json!({"noExtJsonFile": true})));
    assert_eq!(result.filepath, path);
}

#[test]
fn loads_package_json_default_prop() {
    // Default packageProp is the tool name, so "test-app" is extracted.
    let path = load_path("package.json");
    let searcher = SearcherBuilder::new("test-app").build().unwrap();
    let result = searcher.load(&path).unwrap().unwrap();
    assert_eq!(
        result.config,
        Some(json!({"customThingHere": "is-configured"}))
    );
    assert_eq!(result.filepath, path);
}

#[test]
fn loads_package_json_plain_prop_string() {
    let path = load_path("package.json");
    let searcher = SearcherBuilder::new("foo")
        .package_prop(PackageProp::Single("foo".to_string()))
        .build()
        .unwrap();
    let result = searcher.load(&path).unwrap().unwrap();
    assert_eq!(result.config, Some(json!({"insideFoo": true})));
}

#[test]
fn load_package_json_literal_null_prop_yields_some_null() {
    // search/a/b/package.json is {"bar": null}. A single string prop present as
    // a literal key takes the fast path and returns the value raw, so an
    // explicit null comes back as Some(Value::Null), not nothing.
    let path = common::search_path("a/b/package.json");
    let searcher = SearcherBuilder::new("bar")
        .package_prop(PackageProp::Single("bar".to_string()))
        .build()
        .unwrap();
    let result = searcher.load(&path).unwrap().unwrap();
    assert_eq!(result.config, Some(serde_json::Value::Null));
    assert!(!result.is_empty);
}

#[test]
fn load_package_json_absent_prop_yields_null_config() {
    // load does not skip an absent prop the way search does. It returns a null
    // config rather than nothing.
    let path = load_path("package.json");
    let searcher = SearcherBuilder::new("not-present")
        .package_prop(PackageProp::Single("not-present".to_string()))
        .build()
        .unwrap();
    let result = searcher.load(&path).unwrap().unwrap();
    assert_eq!(result.config, Some(serde_json::Value::Null));
    assert_eq!(result.filepath, path);
}

#[test]
fn loads_empty_file_ignored_by_default() {
    let path = load_path("test-empty.json");
    let searcher = SearcherBuilder::new("test-app").build().unwrap();
    let result = searcher.load(&path).unwrap().unwrap();
    assert_eq!(result.config, None);
    assert_eq!(result.filepath, path);
    assert!(result.is_empty);
}

#[test]
fn load_empty_file_returns_is_empty_for_both_ignore_values() {
    // The ignore flag changes search, not load. Both settings return is_empty.
    let path = load_path("test-empty.json");
    for ignore in [true, false] {
        let searcher = SearcherBuilder::new("test-app")
            .ignore_empty_search_places(ignore)
            .build()
            .unwrap();
        let result = searcher.load(&path).unwrap().unwrap();
        assert_eq!(result.config, None);
        assert!(result.is_empty);
        assert_eq!(result.filepath, path);
    }
}

#[test]
fn relative_load_path_resolves_against_cwd() {
    let searcher = SearcherBuilder::new("test-app")
        .cwd(common::fixtures())
        .build()
        .unwrap();
    let result = searcher.load("load/test-app.json").unwrap().unwrap();
    assert_eq!(result.config, Some(json!({"jsonTest": true})));
    assert_eq!(result.filepath, load_path("test-app.json"));
}

#[test]
fn passthrough_loader_returns_raw_content() {
    let path = common::search_path("noExtension");
    let searcher = SearcherBuilder::new("noExtension")
        .loader("noExt", passthrough())
        .build()
        .unwrap();
    let result = searcher.load(&path).unwrap().unwrap();
    assert_eq!(result.config, Some(json!("this file has no extension\n")));
}

#[test]
fn async_load_matches_sync() {
    let path = load_path("test-app.json");
    let searcher = AsyncSearcherBuilder::new("test-app").build().unwrap();
    let result = block(searcher.load(&path)).unwrap().unwrap();
    assert_eq!(result.config, Some(json!({"jsonTest": true})));
    assert_eq!(result.filepath, path);
}

#[test]
fn async_load_package_json() {
    let path = load_path("package.json");
    let searcher = AsyncSearcherBuilder::new("test-app").build().unwrap();
    let result = block(searcher.load(&path)).unwrap().unwrap();
    assert_eq!(
        result.config,
        Some(json!({"customThingHere": "is-configured"}))
    );
}

#[test]
fn leading_dot_name_keys_no_ext() {
    // A leading-dot name with no other dot has no extension, so the key is
    // noExt and the default JSON loader runs. The file holds `{}`.
    let path = load_path(".foorc");
    let searcher = SearcherBuilder::new("test-app").build().unwrap();
    let result = searcher.load(&path).unwrap().unwrap();
    assert_eq!(result.config, Some(json!({})));
}

#[test]
fn multi_dot_name_keys_last_segment() {
    // A multi-dot name keys on the last extension segment. foo.config.json keys
    // on .json, so the default JSON loader runs.
    let path = load_path("foo.config.json");
    let searcher = SearcherBuilder::new("test-app").build().unwrap();
    let result = searcher.load(&path).unwrap().unwrap();
    assert_eq!(result.config, Some(json!({"lastWins": true})));
}
