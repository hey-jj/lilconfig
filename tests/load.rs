//! `load` of a single file: JSON, no-extension JSON, package.json, empty files.

mod common;

use common::{block, load_path, passthrough};
use lilconfig::{AsyncLilconfig, Lilconfig, PackageProp};
use serde_json::json;

#[test]
fn loads_existing_json_file() {
    let path = load_path("test-app.json");
    let searcher = Lilconfig::new("test-app").build().unwrap();
    let result = searcher.load(path.to_str().unwrap()).unwrap().unwrap();
    assert_eq!(result.config, Some(json!({"jsonTest": true})));
    assert_eq!(result.filepath, path);
    assert!(!result.is_empty);
}

#[test]
fn loads_no_extension_json_file() {
    let path = load_path("test-noExt-json");
    let searcher = Lilconfig::new("test-app").build().unwrap();
    let result = searcher.load(path.to_str().unwrap()).unwrap().unwrap();
    assert_eq!(result.config, Some(json!({"noExtJsonFile": true})));
    assert_eq!(result.filepath, path);
}

#[test]
fn loads_package_json_default_prop() {
    // Default packageProp is the tool name, so "test-app" is extracted.
    let path = load_path("package.json");
    let searcher = Lilconfig::new("test-app").build().unwrap();
    let result = searcher.load(path.to_str().unwrap()).unwrap().unwrap();
    assert_eq!(
        result.config,
        Some(json!({"customThingHere": "is-configured"}))
    );
    assert_eq!(result.filepath, path);
}

#[test]
fn loads_package_json_plain_prop_string() {
    let path = load_path("package.json");
    let searcher = Lilconfig::new("foo")
        .package_prop(PackageProp::Single("foo".to_string()))
        .build()
        .unwrap();
    let result = searcher.load(path.to_str().unwrap()).unwrap().unwrap();
    assert_eq!(result.config, Some(json!({"insideFoo": true})));
}

#[test]
fn load_package_json_absent_prop_yields_null_config() {
    // load does not skip an absent prop the way search does. It returns a null
    // config rather than nothing.
    let path = load_path("package.json");
    let searcher = Lilconfig::new("not-present")
        .package_prop(PackageProp::Single("not-present".to_string()))
        .build()
        .unwrap();
    let result = searcher.load(path.to_str().unwrap()).unwrap().unwrap();
    assert_eq!(result.config, Some(serde_json::Value::Null));
    assert_eq!(result.filepath, path);
}

#[test]
fn loads_empty_file_ignored_by_default() {
    let path = load_path("test-empty.json");
    let searcher = Lilconfig::new("test-app").build().unwrap();
    let result = searcher.load(path.to_str().unwrap()).unwrap().unwrap();
    assert_eq!(result.config, None);
    assert_eq!(result.filepath, path);
    assert!(result.is_empty);
}

#[test]
fn load_empty_file_returns_is_empty_for_both_ignore_values() {
    // The ignore flag changes search, not load. Both settings return is_empty.
    let path = load_path("test-empty.json");
    for ignore in [true, false] {
        let searcher = Lilconfig::new("test-app")
            .ignore_empty_search_places(ignore)
            .build()
            .unwrap();
        let result = searcher.load(path.to_str().unwrap()).unwrap().unwrap();
        assert_eq!(result.config, None);
        assert!(result.is_empty);
        assert_eq!(result.filepath, path);
    }
}

#[test]
fn relative_load_path_resolves_against_cwd() {
    let searcher = Lilconfig::new("test-app")
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
    let searcher = Lilconfig::new("noExtension")
        .loader("noExt", passthrough())
        .build()
        .unwrap();
    let result = searcher.load(path.to_str().unwrap()).unwrap().unwrap();
    assert_eq!(result.config, Some(json!("this file has no extension\n")));
}

#[test]
fn async_load_matches_sync() {
    let path = load_path("test-app.json");
    let searcher = AsyncLilconfig::new("test-app").build().unwrap();
    let result = block(searcher.load(path.to_str().unwrap()))
        .unwrap()
        .unwrap();
    assert_eq!(result.config, Some(json!({"jsonTest": true})));
    assert_eq!(result.filepath, path);
}

#[test]
fn async_load_package_json() {
    let path = load_path("package.json");
    let searcher = AsyncLilconfig::new("test-app").build().unwrap();
    let result = block(searcher.load(path.to_str().unwrap()))
        .unwrap()
        .unwrap();
    assert_eq!(
        result.config,
        Some(json!({"customThingHere": "is-configured"}))
    );
}
