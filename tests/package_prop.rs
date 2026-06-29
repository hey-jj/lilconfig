//! packageProp extraction: dotted strings, arrays, null-in-chain, falsy collapse.

mod common;

use common::{search_path, search_root};
use lilconfig::{Error, Lilconfig, PackageProp};
use serde_json::json;

#[test]
fn dotted_string_descends_path() {
    let path = search_path("a/package.json");
    let searcher = Lilconfig::new("foo")
        .package_prop(PackageProp::Single("bar.baz".to_string()))
        .build()
        .unwrap();
    let result = searcher.load(path.to_str().unwrap()).unwrap().unwrap();
    assert_eq!(result.config, Some(json!({"insideBarBaz": true})));
}

#[test]
fn explicit_array_descends_path() {
    let path = search_path("a/package.json");
    let searcher = Lilconfig::new("foo")
        .package_prop(PackageProp::Path(vec![
            "zoo".to_string(),
            "foo".to_string(),
        ]))
        .build()
        .unwrap();
    let result = searcher.load(path.to_str().unwrap()).unwrap().unwrap();
    assert_eq!(result.config, Some(json!({"insideZooFoo": true})));
}

#[test]
fn search_finds_dotted_prop_skipping_null_package() {
    // a/b/package.json has bar:null and no zoo. Searching from a/b/c for zoo.foo
    // skips it (no match) and finds a/package.json.
    let from = search_path("a/b/c");
    let searcher = Lilconfig::new("foo")
        .stop_dir(search_root())
        .package_prop(PackageProp::Single("zoo.foo".to_string()))
        .build()
        .unwrap();
    let result = searcher.search(&from).unwrap().unwrap();
    assert_eq!(result.config, Some(json!({"insideZooFoo": true})));
    assert_eq!(result.filepath, search_path("a/package.json"));
}

#[test]
fn null_in_the_middle_of_chain_errors() {
    // a/b/package.json has bar:null. Descending bar.baz reads baz off null,
    // which JavaScript rejects. The same traversal errors here.
    let from = search_path("a/b/c");
    let searcher = Lilconfig::new("foo")
        .stop_dir(search_root())
        .package_prop(PackageProp::Single("bar.baz".to_string()))
        .build()
        .unwrap();
    let err = searcher.search(&from).unwrap_err();
    match err {
        Error::NullInPropPath { key } => assert_eq!(key, "baz"),
        other => panic!("expected NullInPropPath, got {other:?}"),
    }
}

#[test]
fn literal_dotted_key_uses_fast_path() {
    // A single string that is present as a literal key returns that key's value
    // without splitting on dots. Here "foo.bar" is a real key, not a path.
    let path = search_path("a/package.json");
    let searcher = Lilconfig::new("tool")
        .package_prop(PackageProp::Single("foo.bar".to_string()))
        .loader(".json", lilconfig::loader(literal_pkg_loader))
        .build()
        .unwrap();
    let result = searcher.load(path.to_str().unwrap()).unwrap().unwrap();
    assert_eq!(result.config, Some(json!("literal")));
}

fn literal_pkg_loader(_p: &std::path::Path, _c: &str) -> Result<serde_json::Value, Error> {
    // Both a literal "foo.bar" key and a nested foo.bar path exist. The fast
    // path must pick the literal key.
    Ok(json!({"foo.bar": "literal", "foo": {"bar": "nested"}}))
}

#[test]
fn falsy_prop_value_collapses_to_null_in_search() {
    // A dotted-path prop value of false collapses to null via the || null
    // coercion, so search treats it as no match and returns nothing.
    let from = search_path("a");
    let searcher = Lilconfig::new("tool")
        .stop_dir(search_root())
        .search_places(["package.json"])
        // a/package.json has no such nested falsy value, so build a fixture via
        // a custom loader that returns a package object with a false prop.
        .loader(".json", lilconfig::loader(false_pkg))
        .package_prop(PackageProp::Single("nested.flag".to_string()))
        .build()
        .unwrap();
    assert_eq!(searcher.search(&from).unwrap(), None);
}

fn false_pkg(_p: &std::path::Path, _c: &str) -> Result<serde_json::Value, Error> {
    Ok(json!({"nested": {"flag": false}}))
}

#[test]
fn fast_path_falsy_value_matches_in_search() {
    // The fast path returns a literal key raw. A false value is not null, so
    // search counts it as a match.
    let from = search_path("a");
    let searcher = Lilconfig::new("tool")
        .stop_dir(search_root())
        .search_places(["package.json"])
        .loader(".json", lilconfig::loader(false_flag_pkg))
        .package_prop(PackageProp::Single("flag".to_string()))
        .build()
        .unwrap();
    let result = searcher.search(&from).unwrap().unwrap();
    assert_eq!(result.config, Some(json!(false)));
}

fn false_flag_pkg(_p: &std::path::Path, _c: &str) -> Result<serde_json::Value, Error> {
    Ok(json!({"flag": false}))
}
