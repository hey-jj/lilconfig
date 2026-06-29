//! Loaders turn file text into a config value.
//!
//! A loader receives the file path and its UTF-8 content and returns a
//! [`serde_json::Value`] or an [`Error`]. Loaders are keyed by extension. The
//! key for a file with no extension is the literal `noExt`.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use serde_json::Value;

use crate::error::Error;

/// A function that parses file content into a config value.
///
/// The first argument is the path the content came from. The second is the
/// UTF-8 text. The default JSON loader ignores the path and parses the text.
pub type LoaderFn = dyn Fn(&Path, &str) -> Result<Value, Error> + Send + Sync;

/// A shared, cloneable loader handle.
pub type Loader = Arc<LoaderFn>;

/// A table of loaders keyed by extension (`.json`) or the literal `noExt`.
pub type Loaders = HashMap<String, Loader>;

/// Parses content as JSON.
///
/// This backs both the `.json` and `noExt` default loaders. A parse failure
/// becomes [`Error::Loader`] carrying the path and the serde_json message.
pub fn json_loader(path: &Path, content: &str) -> Result<Value, Error> {
    serde_json::from_str(content).map_err(|e| Error::Loader {
        path: path.to_path_buf(),
        message: e.to_string(),
    })
}

/// Wraps a plain function as a shared [`Loader`].
pub fn loader<F>(f: F) -> Loader
where
    F: Fn(&Path, &str) -> Result<Value, Error> + Send + Sync + 'static,
{
    Arc::new(f)
}

/// The default async loader table.
///
/// JavaScript lilconfig executes `.js`, `.cjs`, and `.mjs` configs. Rust cannot
/// run JavaScript, so this table ships JSON only: `.json` and `noExt` both parse
/// JSON. Register your own loaders for other extensions.
pub fn default_loaders() -> Loaders {
    let mut m = Loaders::new();
    let json = loader(json_loader);
    m.insert(".json".to_string(), json.clone());
    m.insert("noExt".to_string(), json);
    m
}

/// The default sync loader table.
///
/// Identical to [`default_loaders`]. The split exists to mirror the two API
/// surfaces and to leave room for sync-only or async-only loaders.
pub fn default_loaders_sync() -> Loaders {
    default_loaders()
}
