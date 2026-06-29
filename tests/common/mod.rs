//! Shared helpers for the conformance tests.

#![allow(dead_code)]

use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use lilconfig::{loader, Error, Fs, LilconfigResult, Loader};
use serde_json::Value;

/// Absolute path to the fixtures tree.
pub fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// Path inside the `load/` fixtures.
pub fn load_path(name: &str) -> PathBuf {
    fixtures().join("load").join(name)
}

/// Path inside the `search/` fixtures.
pub fn search_path(rel: &str) -> PathBuf {
    let mut p = fixtures().join("search");
    for part in rel.split('/') {
        p = p.join(part);
    }
    p
}

/// The `search/` directory itself.
pub fn search_root() -> PathBuf {
    fixtures().join("search")
}

/// Runs the async future to completion on the current thread.
pub fn block<F: std::future::Future>(fut: F) -> F::Output {
    pollster::block_on(fut)
}

/// A loader that returns a fixed JSON value, ignoring the file content.
pub fn fixed_value(value: Value) -> Loader {
    loader(move |_p: &Path, _c: &str| Ok(value.clone()))
}

/// A loader that returns the file content verbatim as a JSON string.
pub fn passthrough() -> Loader {
    loader(|_p: &Path, c: &str| Ok(Value::String(c.to_string())))
}

/// A loader that always fails.
pub fn failing() -> Loader {
    loader(|p: &Path, _c: &str| {
        Err(Error::Loader {
            path: p.to_path_buf(),
            message: "boom".to_string(),
        })
    })
}

/// Shorthand for `Some(serde_json::Value)` config in a result assertion.
pub fn config(result: &LilconfigResult) -> Option<&Value> {
    result.as_ref().and_then(|r| r.config.as_ref())
}

/// A filesystem where nothing exists and every access path is recorded.
///
/// Used to pin the order in which search places are tried.
#[derive(Clone, Default)]
pub struct RecordingFs {
    accesses: Arc<Mutex<Vec<PathBuf>>>,
}

impl RecordingFs {
    /// The paths passed to `access`, in order.
    pub fn accesses(&self) -> Vec<PathBuf> {
        self.accesses.lock().unwrap().clone()
    }
}

impl Fs for RecordingFs {
    fn access(&self, path: &Path) -> io::Result<()> {
        self.accesses.lock().unwrap().push(path.to_path_buf());
        Err(io::Error::from(io::ErrorKind::NotFound))
    }

    fn read_to_string(&self, _path: &Path) -> io::Result<String> {
        Err(io::Error::from(io::ErrorKind::NotFound))
    }
}
