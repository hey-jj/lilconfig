//! RealFs::access tests existence, not readability.
//!
//! A present but unreadable file must report as accessible so the later read
//! surfaces the read error instead of the search silently skipping the file.

mod common;

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use lilconfig::{Fs, RealFs, SearcherBuilder};

/// A unique temp path for one test.
fn temp_path(tag: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    p.push(format!("lilconfig-{tag}-{nanos}"));
    p
}

#[test]
fn access_reports_existing_file() {
    let path = temp_path("exists");
    fs::write(&path, b"{}").unwrap();
    assert!(RealFs.access(&path).is_ok());
    fs::remove_file(&path).ok();
}

#[test]
fn access_reports_missing_file_as_error() {
    let path = temp_path("missing");
    assert!(RealFs.access(&path).is_err());
}

/// A filesystem where the config file exists but cannot be read.
///
/// `access` reports it present. `read_to_string` fails with a permission error.
/// This is the present-but-unreadable case that `access` must not hide.
struct PresentButUnreadable {
    target: PathBuf,
}

impl Fs for PresentButUnreadable {
    fn access(&self, path: &Path) -> io::Result<()> {
        if path == self.target {
            Ok(())
        } else {
            Err(io::Error::from(io::ErrorKind::NotFound))
        }
    }

    fn read_to_string(&self, path: &Path) -> io::Result<String> {
        if path == self.target {
            Err(io::Error::from(io::ErrorKind::PermissionDenied))
        } else {
            Err(io::Error::from(io::ErrorKind::NotFound))
        }
    }
}

#[test]
fn present_but_unreadable_config_surfaces_read_error() {
    // The first search place exists but the read fails. The search must report
    // the read error, not skip the file and keep climbing.
    let dir = common::search_root();
    let target = dir.join("present.config.json");
    let fs = PresentButUnreadable {
        target: target.clone(),
    };
    let searcher = SearcherBuilder::new("present")
        .stop_dir(&dir)
        .search_places(["present.config.json"])
        .build_with_fs(fs)
        .unwrap();

    let err = searcher.search(&dir).unwrap_err();
    match err {
        lilconfig::Error::Io { path, source } => {
            assert_eq!(path, target);
            assert_eq!(source.kind(), io::ErrorKind::PermissionDenied);
        }
        other => panic!("expected Io PermissionDenied, got {other:?}"),
    }
}
