//! packageProp extraction: dotted strings, arrays, null-in-chain, falsy collapse.

mod common;

use common::{search_path, search_root};
use lilconfig::{Error, PackageProp, SearcherBuilder};
use serde_json::json;

#[test]
fn dotted_string_descends_path() {
    let path = search_path("a/package.json");
    let searcher = SearcherBuilder::new("foo")
        .package_prop(PackageProp::Single("bar.baz".to_string()))
        .build()
        .unwrap();
    let result = searcher.load(&path).unwrap().unwrap();
    assert_eq!(result.config, Some(json!({"insideBarBaz": true})));
}

#[test]
fn explicit_array_descends_path() {
    let path = search_path("a/package.json");
    let searcher = SearcherBuilder::new("foo")
        .package_prop(PackageProp::Path(vec![
            "zoo".to_string(),
            "foo".to_string(),
        ]))
        .build()
        .unwrap();
    let result = searcher.load(&path).unwrap().unwrap();
    assert_eq!(result.config, Some(json!({"insideZooFoo": true})));
}

#[test]
fn search_finds_dotted_prop_skipping_null_package() {
    // a/b/package.json has bar:null and no zoo. Searching from a/b/c for zoo.foo
    // skips it (no match) and finds a/package.json.
    let from = search_path("a/b/c");
    let searcher = SearcherBuilder::new("foo")
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
    let searcher = SearcherBuilder::new("foo")
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
    let searcher = SearcherBuilder::new("tool")
        .package_prop(PackageProp::Single("foo.bar".to_string()))
        .loader(".json", lilconfig::loader(literal_pkg_loader))
        .build()
        .unwrap();
    let result = searcher.load(&path).unwrap().unwrap();
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
    let searcher = SearcherBuilder::new("tool")
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
    let searcher = SearcherBuilder::new("tool")
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

/// Builds a searcher whose package.json loader returns `value` under "flag".
fn fast_path_searcher(value: serde_json::Value) -> lilconfig::Searcher {
    SearcherBuilder::new("tool")
        .stop_dir(search_root())
        .search_places(["package.json"])
        .loader(
            ".json",
            lilconfig::loader(move |_p: &std::path::Path, _c: &str| {
                Ok(json!({ "flag": value.clone() }))
            }),
        )
        .package_prop(PackageProp::Single("flag".to_string()))
        .build()
        .unwrap()
}

#[test]
fn fast_path_empty_string_matches_in_search() {
    // The fast path returns a literal key raw. An empty string is not null, so
    // search counts it as a match and stops.
    let from = search_path("a");
    let result = fast_path_searcher(json!(""))
        .search(&from)
        .unwrap()
        .unwrap();
    assert_eq!(result.config, Some(json!("")));
}

#[test]
fn fast_path_zero_matches_in_search() {
    // Zero is not null, so the fast path counts it as a match.
    let from = search_path("a");
    let result = fast_path_searcher(json!(0)).search(&from).unwrap().unwrap();
    assert_eq!(result.config, Some(json!(0)));
}

/// Builds a searcher whose package.json loader nests `value` under nested.flag.
fn dotted_path_searcher(value: serde_json::Value) -> lilconfig::Searcher {
    SearcherBuilder::new("tool")
        .stop_dir(search_root())
        .search_places(["package.json"])
        .loader(
            ".json",
            lilconfig::loader(move |_p: &std::path::Path, _c: &str| {
                Ok(json!({ "nested": { "flag": value.clone() } }))
            }),
        )
        .package_prop(PackageProp::Single("nested.flag".to_string()))
        .build()
        .unwrap()
}

#[test]
fn dotted_path_empty_string_collapses_to_null_in_search() {
    // The dotted path ends with a || null coercion, so a falsy empty string
    // collapses to null and does not match.
    let from = search_path("a");
    assert_eq!(dotted_path_searcher(json!("")).search(&from).unwrap(), None);
}

#[test]
fn dotted_path_zero_collapses_to_null_in_search() {
    // Zero is falsy, so the dotted path collapses it to null and does not match.
    let from = search_path("a");
    assert_eq!(dotted_path_searcher(json!(0)).search(&from).unwrap(), None);
}
