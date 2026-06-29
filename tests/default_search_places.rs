//! The default search places and the order they are tried.

mod common;

use std::path::PathBuf;

use common::RecordingFs;
use lilconfig::{default_loaders, SearcherBuilder};

#[test]
fn default_order_for_one_directory() {
    // Search one directory with a recording filesystem so the exact order of
    // access calls is observable. The default set carries only loaders this
    // crate can run: JSON and the extensionless noExt.
    let dir = PathBuf::from("/proj");
    let fs = RecordingFs::default();
    let searcher = SearcherBuilder::new("myapp")
        .stop_dir(&dir)
        .build_with_fs(fs.clone())
        .unwrap();

    assert_eq!(searcher.search(&dir).unwrap(), None);

    let expected: Vec<PathBuf> = [
        "package.json",
        ".myapprc.json",
        ".config/myapprc",
        ".config/myapprc.json",
    ]
    .iter()
    .map(|p| dir.join(p))
    .collect();

    assert_eq!(fs.accesses(), expected);
}

#[test]
fn default_loader_table_exposes_json_and_no_ext() {
    let table = default_loaders();
    assert!(table.contains_key(".json"));
    assert!(table.contains_key("noExt"));
    // No JavaScript execution loaders ship by default.
    assert!(!table.contains_key(".js"));
    assert!(!table.contains_key(".cjs"));
    assert!(!table.contains_key(".mjs"));
}
