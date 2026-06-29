//! transform runs on found results and on the not-found `None`.

mod common;

use common::{block, load_path, search_path, search_root};
use lilconfig::{AsyncLilconfig, Lilconfig};
use serde_json::{json, Value};

/// Adds `transformed: true` to a found config, passes `None` through.
fn add_transformed(
    result: lilconfig::LilconfigResult,
) -> Result<lilconfig::LilconfigResult, lilconfig::Error> {
    Ok(result.map(|mut r| {
        if let Some(Value::Object(map)) = r.config.as_mut() {
            map.insert("transformed".to_string(), json!(true));
        }
        r
    }))
}

#[test]
fn transform_mutates_loaded_config() {
    let path = load_path("test-app.json");
    let searcher = Lilconfig::new("test-app")
        .transform(add_transformed)
        .build()
        .unwrap();
    let result = searcher.load(path.to_str().unwrap()).unwrap().unwrap();
    assert_eq!(
        result.config,
        Some(json!({"jsonTest": true, "transformed": true}))
    );
}

#[test]
fn transform_receives_none_on_not_found() {
    // The transform turns a not-found into a sentinel result.
    let from = search_path("a/b/c");
    let searcher = Lilconfig::new("non-existent")
        .stop_dir(search_root())
        .transform(|result| {
            Ok(result.or(Some(lilconfig::SearchResult {
                config: Some(json!("sentinel")),
                filepath: std::path::PathBuf::new(),
                is_empty: false,
            })))
        })
        .build()
        .unwrap();
    let result = searcher.search(&from).unwrap().unwrap();
    assert_eq!(result.config, Some(json!("sentinel")));
}

#[test]
fn transform_error_propagates() {
    let path = load_path("test-app.json");
    let searcher = Lilconfig::new("test-app")
        .transform(|_| {
            Err(lilconfig::Error::Loader {
                path: std::path::PathBuf::from("x"),
                message: "rejected".to_string(),
            })
        })
        .build()
        .unwrap();
    assert!(searcher.load(path.to_str().unwrap()).is_err());
}

#[test]
fn async_transform_mutates_loaded_config() {
    let path = load_path("test-app.json");
    let searcher = AsyncLilconfig::new("test-app")
        .transform(add_transformed)
        .build()
        .unwrap();
    let result = block(searcher.load(path.to_str().unwrap()))
        .unwrap()
        .unwrap();
    assert_eq!(
        result.config,
        Some(json!({"jsonTest": true, "transformed": true}))
    );
}
