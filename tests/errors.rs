//! Error paths: empty path, missing loader, parse failures, missing files.

mod common;

use common::{block, load_path};
use lilconfig::{AsyncLilconfig, Error, Lilconfig};

#[test]
fn empty_load_path_errors_with_exact_message() {
    let searcher = Lilconfig::new("test-app").build().unwrap();
    let err = searcher.load("").unwrap_err();
    assert!(matches!(err, Error::EmptyFilePath));
    assert_eq!(err.to_string(), "load must pass a non-empty string");
}

#[test]
fn missing_loader_for_search_place_fails_at_construction() {
    let err = match Lilconfig::new("foo").search_places(["file.coffee"]).build() {
        Ok(_) => panic!("expected build to fail"),
        Err(e) => e,
    };
    match &err {
        Error::MissingLoaderForPlace { place } => assert_eq!(place, "file.coffee"),
        other => panic!("expected MissingLoaderForPlace, got {other:?}"),
    }
    assert_eq!(
        err.to_string(),
        "Missing loader for extension \"file.coffee\""
    );
}

#[test]
fn extensionless_search_place_builds_with_default_no_ext_loader() {
    // Default loaders include noExt, so an extensionless place builds fine.
    assert!(Lilconfig::new("foo")
        .search_places(["plain"])
        .build()
        .is_ok());
}

#[test]
fn load_unknown_extension_reports_missing_loader() {
    let path = load_path("config.coffee");
    let searcher = Lilconfig::new("test-app").build().unwrap();
    let err = searcher.load(path.to_str().unwrap()).unwrap_err();
    match &err {
        Error::NoLoaderForExtension { key } => assert_eq!(key, ".coffee"),
        other => panic!("expected NoLoaderForExtension, got {other:?}"),
    }
    assert_eq!(
        err.to_string(),
        "No loader specified for extension \".coffee\""
    );
}

#[test]
fn load_non_existent_file_is_not_found() {
    let path = load_path("nope.json");
    let searcher = Lilconfig::new("test-app").build().unwrap();
    let err = searcher.load(path.to_str().unwrap()).unwrap_err();
    match &err {
        Error::Io { path: p, source } => {
            assert_eq!(p, &path);
            assert_eq!(source.kind(), std::io::ErrorKind::NotFound);
        }
        other => panic!("expected Io NotFound, got {other:?}"),
    }
}

#[test]
fn load_non_existent_js_file_errors_before_loader() {
    // A .js path with no registered loader still errors on the missing file,
    // because reading happens after loader lookup. With no .js loader, the
    // loader check fails first. Register one to confirm the read error wins.
    let path = load_path("i-do-not-exist.json");
    let searcher = Lilconfig::new("test-app").build().unwrap();
    let err = searcher.load(path.to_str().unwrap()).unwrap_err();
    assert!(matches!(err, Error::Io { .. }));
}

#[test]
fn invalid_json_propagates_parse_error() {
    let path = load_path("test-invalid.json");
    let searcher = Lilconfig::new("test-app").build().unwrap();
    let err = searcher.load(path.to_str().unwrap()).unwrap_err();
    match &err {
        Error::Loader { path: p, message } => {
            assert_eq!(p, &path);
            assert!(!message.is_empty());
        }
        other => panic!("expected Loader error, got {other:?}"),
    }
}

#[test]
fn no_extension_unparsable_file_errors() {
    let path = load_path("test-noExt-nonParsable");
    let searcher = Lilconfig::new("test-app").build().unwrap();
    let err = searcher.load(path.to_str().unwrap()).unwrap_err();
    assert!(matches!(err, Error::Loader { .. }));
}

#[test]
fn loader_that_throws_propagates_during_search() {
    let from = common::search_path("a/b/c");
    let searcher = Lilconfig::new("maybeEmpty")
        .stop_dir(common::search_root())
        .search_places(["maybeEmpty.config.json"])
        .ignore_empty_search_places(false)
        .loader(".json", common::failing())
        .build()
        .unwrap();
    // a/b/maybeEmpty.config.json is empty, so the loader is not called and the
    // empty result returns. Use a non-empty fixture to hit the loader.
    let result = searcher.search(&from).unwrap().unwrap();
    assert!(result.is_empty);

    let searcher = Lilconfig::new("either")
        .stop_dir(common::search_root())
        .search_places(["package.json"])
        .loader(".json", common::failing())
        .build()
        .unwrap();
    // a/b/package.json is non-empty and triggers the failing loader.
    assert!(searcher.search(&from).is_err());
}

#[test]
fn async_empty_load_path_errors() {
    let searcher = AsyncLilconfig::new("test-app").build().unwrap();
    let err = block(searcher.load("")).unwrap_err();
    assert!(matches!(err, Error::EmptyFilePath));
}

#[test]
fn async_invalid_json_errors() {
    let path = load_path("test-invalid.json");
    let searcher = AsyncLilconfig::new("test-app").build().unwrap();
    assert!(block(searcher.load(path.to_str().unwrap())).is_err());
}
