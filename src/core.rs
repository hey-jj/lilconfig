//! The shared engine for search and load.
//!
//! Both the sync and async API run the same logic here. The engine takes a
//! filesystem and resolved options and walks directories, reads files, runs
//! loaders, and maintains the two caches. The public modules wrap it.

use std::collections::{HashMap, HashSet};
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde_json::Value;

use crate::error::Error;
use crate::fs::Fs;
use crate::loaders::Loaders;

/// A found config plus where it came from.
///
/// `config` distinguishes two JavaScript states. `None` is `undefined`, used
/// for empty files. `Some(Value::Null)` is an explicit `null` config.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchResult {
    /// The parsed config. `None` means the file was empty.
    pub config: Option<Value>,
    /// The absolute path the config came from.
    pub filepath: PathBuf,
    /// True when the matched file was empty.
    pub is_empty: bool,
}

/// Transforms a result before it is returned and cached.
///
/// Runs on every outcome, including the not-found case where it receives
/// `None`. The default is the identity transform.
pub type Transform =
    Arc<dyn Fn(Option<SearchResult>) -> Result<Option<SearchResult>, Error> + Send + Sync>;

/// Where to look for a config key inside `package.json`.
#[derive(Debug, Clone)]
pub enum PackageProp {
    /// A single key, or a dotted path checked as a literal key first.
    Single(String),
    /// An explicit list of keys to descend.
    Path(Vec<String>),
}

/// Resolved options shared by both API surfaces.
pub struct Resolved {
    /// Stop walking up once this directory is reached.
    pub stop_dir: PathBuf,
    /// Filenames to try in each directory, in order.
    pub search_places: Vec<String>,
    /// Skip empty files during search instead of stopping on them.
    pub ignore_empty_search_places: bool,
    /// Whether to read and write the caches.
    pub cache: bool,
    /// The result transform.
    pub transform: Transform,
    /// Which key of `package.json` holds the config.
    pub package_prop: PackageProp,
    /// Loaders keyed by extension or `noExt`.
    pub loaders: Loaders,
    /// True for the synchronous surface. The synchronous `load` returns a
    /// `package.json` result without writing it to the load cache, so a second
    /// `load` of the same `package.json` reads the file again. Every other path
    /// caches the same on both surfaces.
    pub sync: bool,
}

/// The engine: resolved options, a filesystem, and the two caches.
pub struct Core<F: Fs> {
    opts: Resolved,
    fs: F,
    search_cache: Mutex<HashMap<PathBuf, Option<SearchResult>>>,
    load_cache: Mutex<HashMap<PathBuf, Option<SearchResult>>>,
}

impl<F: Fs> Core<F> {
    /// Builds the engine and validates that every search place has a loader.
    ///
    /// Validation runs here so a bad search place fails when the searcher is
    /// constructed, before any filesystem work, matching the eager check in the
    /// reference behavior.
    pub fn new(opts: Resolved, fs: F) -> Result<Self, Error> {
        for place in &opts.search_places {
            let key = loader_key(place);
            if !opts.loaders.contains_key(&key) {
                return Err(Error::MissingLoaderForPlace {
                    place: place.clone(),
                });
            }
        }
        Ok(Self {
            opts,
            fs,
            search_cache: Mutex::new(HashMap::new()),
            load_cache: Mutex::new(HashMap::new()),
        })
    }

    /// Empties the load cache. A no-op when caching is off.
    pub fn clear_load_cache(&self) {
        if self.opts.cache {
            self.load_cache.lock().unwrap().clear();
        }
    }

    /// Empties the search cache. A no-op when caching is off.
    pub fn clear_search_cache(&self) {
        if self.opts.cache {
            self.search_cache.lock().unwrap().clear();
        }
    }

    /// Empties both caches. A no-op when caching is off.
    pub fn clear_caches(&self) {
        if self.opts.cache {
            self.load_cache.lock().unwrap().clear();
            self.search_cache.lock().unwrap().clear();
        }
    }

    /// Walks up from `search_from`, returning the first qualifying config.
    ///
    /// Tries every search place in each directory before moving to the parent.
    /// Stops at `stop_dir` or the filesystem root. Returns the transformed
    /// result, which is `None` when nothing matched.
    pub fn search(&self, search_from: &Path) -> Result<Option<SearchResult>, Error> {
        let mut config: Option<Value> = None;
        let mut filepath: Option<PathBuf> = None;
        let mut is_empty = false;

        let mut visited: Vec<PathBuf> = Vec::new();
        let mut seen: HashSet<PathBuf> = HashSet::new();
        let mut dir = search_from.to_path_buf();

        'dir_loop: loop {
            if self.opts.cache {
                let hit = self.search_cache.lock().unwrap().get(&dir).cloned();
                if let Some(cached) = hit {
                    let mut cache = self.search_cache.lock().unwrap();
                    for p in &visited {
                        cache.insert(p.clone(), cached.clone());
                    }
                    return Ok(cached);
                }
                if seen.insert(dir.clone()) {
                    visited.push(dir.clone());
                }
            }

            for place in &self.opts.search_places {
                let candidate = dir.join(place);
                if self.fs.access(&candidate).is_err() {
                    continue;
                }
                let content = read_text(&self.fs, &candidate)?;
                let key = loader_key(place);

                if place == "package.json" {
                    let loader = self.loader_for(&key)?;
                    let pkg = loader(&candidate, &content)?;
                    let found = get_package_prop(&self.opts.package_prop, &pkg)?;
                    // Match on anything except null. Falsy-but-defined values
                    // from the fast path (0, false, "") still count as a match.
                    if !found.is_null() {
                        config = Some(found);
                        filepath = Some(candidate);
                        break 'dir_loop;
                    }
                    continue;
                }

                let empty = is_blank(&content);
                if empty && self.opts.ignore_empty_search_places {
                    continue;
                }

                if empty {
                    is_empty = true;
                    config = None;
                } else {
                    let loader = self.loader_for(&key)?;
                    config = Some(loader(&candidate, &content)?);
                }
                filepath = Some(candidate);
                break 'dir_loop;
            }

            let parent = parent_dir(&dir);
            if dir == self.opts.stop_dir || dir == parent {
                break 'dir_loop;
            }
            dir = parent;
        }

        let result: Option<SearchResult> = filepath.map(|path| SearchResult {
            config,
            filepath: path,
            is_empty,
        });
        let transformed = (self.opts.transform)(result)?;

        if self.opts.cache {
            let mut cache = self.search_cache.lock().unwrap();
            for p in &visited {
                cache.insert(p.clone(), transformed.clone());
            }
        }

        Ok(transformed)
    }

    /// Loads one config file by path, resolved against `cwd`.
    ///
    /// Empty files yield `config: None` with `is_empty` set. A `package.json`
    /// extracts the configured prop. Errors propagate from the loader and the
    /// filesystem.
    pub fn load(&self, cwd: &Path, filepath: &Path) -> Result<Option<SearchResult>, Error> {
        if filepath.as_os_str().is_empty() {
            return Err(Error::EmptyFilePath);
        }
        let abs = resolve(cwd, filepath);

        if self.opts.cache {
            if let Some(cached) = self.load_cache.lock().unwrap().get(&abs).cloned() {
                return Ok(cached);
            }
        }

        let base = abs
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let key = loader_key(&base);
        let loader = self.loader_for(&key)?;
        let content = read_text(&self.fs, &abs)?;

        if base == "package.json" {
            let pkg = loader(&abs, &content)?;
            // load keeps the extracted value as-is, including a null config.
            let config = get_package_prop(&self.opts.package_prop, &pkg)?;
            let result = Some(SearchResult {
                config: Some(config),
                filepath: abs.clone(),
                is_empty: false,
            });
            let transformed = (self.opts.transform)(result)?;
            // The synchronous surface does not cache the package.json result, so
            // a repeat load re-reads the file. The asynchronous surface caches.
            if self.opts.sync {
                return Ok(transformed);
            }
            return Ok(self.emplace(abs, transformed));
        }

        let empty = is_blank(&content);
        if empty && self.opts.ignore_empty_search_places {
            let transformed = (self.opts.transform)(Some(SearchResult {
                config: None,
                filepath: abs.clone(),
                is_empty: true,
            }))?;
            return Ok(self.emplace(abs, transformed));
        }

        let result = if empty {
            SearchResult {
                config: None,
                filepath: abs.clone(),
                is_empty: true,
            }
        } else {
            let config = loader(&abs, &content)?;
            SearchResult {
                config: Some(config),
                filepath: abs.clone(),
                is_empty: false,
            }
        };
        let transformed = (self.opts.transform)(Some(result))?;
        Ok(self.emplace(abs, transformed))
    }

    fn emplace(&self, key: PathBuf, value: Option<SearchResult>) -> Option<SearchResult> {
        if self.opts.cache {
            self.load_cache.lock().unwrap().insert(key, value.clone());
        }
        value
    }

    fn loader_for(&self, key: &str) -> Result<&crate::loaders::Loader, Error> {
        self.opts
            .loaders
            .get(key)
            .ok_or_else(|| Error::NoLoaderForExtension {
                key: key.to_string(),
            })
    }
}

fn read_text<F: Fs>(fs: &F, path: &Path) -> Result<String, Error> {
    fs.read_to_string(path).map_err(|source| Error::Io {
        path: path.to_path_buf(),
        source,
    })
}

/// Computes the loader key for a filename: its extension, or `noExt`.
///
/// Matches Node's `path.extname`: a leading-dot name with no other dot has no
/// extension (`.foorc` -> `noExt`), while `.foorc.json` keys on `.json`.
pub fn loader_key(name: &str) -> String {
    match extname(name) {
        Some(ext) => ext,
        None => "noExt".to_string(),
    }
}

/// Returns the extension including the leading dot, or `None`.
///
/// Follows Node's `path.extname` rules rather than Rust's `Path::extension`:
/// the search runs on the basename, a leading dot does not start an extension,
/// and a trailing dot yields `.`.
fn extname(name: &str) -> Option<String> {
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name);
    let bytes = base.as_bytes();
    let mut last_dot: Option<usize> = None;
    for (i, &b) in bytes.iter().enumerate() {
        if b == b'.' {
            last_dot = Some(i);
        }
    }
    match last_dot {
        // No dot, or the only dot is the leading one: no extension.
        None | Some(0) => None,
        Some(idx) => Some(base[idx..].to_string()),
    }
}

/// Returns the parent directory, falling back to the path separator at the root.
///
/// Mirrors `path.dirname(p) || path.sep`. On Unix a final `dirname` of `/build`
/// yields the root rather than an empty string. The root is the fixed point
/// where `parent_dir(dir) == dir`.
pub fn parent_dir(p: &Path) -> PathBuf {
    match p.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.to_path_buf(),
        _ => PathBuf::from(std::path::MAIN_SEPARATOR.to_string()),
    }
}

/// Resolves `path` against `base` to an absolute, lexically normalized path.
///
/// Mirrors `path.resolve(cwd, filepath)`: an absolute input is kept, a relative
/// input is joined onto `base`, and `.`/`..` segments are folded without
/// touching the filesystem.
pub fn resolve(base: &Path, path: &Path) -> PathBuf {
    let joined = if path.is_absolute() {
        path.to_path_buf()
    } else {
        base.join(path)
    };
    normalize(&joined)
}

fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::ParentDir => {
                if !out.pop() && !out.has_root() {
                    out.push("..");
                }
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Reports whether trimming `content` leaves nothing.
///
/// JavaScript `String.prototype.trim` strips the Unicode White_Space set plus
/// the BOM U+FEFF. Rust's `str::trim` covers White_Space but not the BOM, so
/// strip it explicitly to match.
pub fn is_blank(content: &str) -> bool {
    content
        .trim_matches(|c: char| c.is_whitespace() || c == '\u{FEFF}')
        .is_empty()
}

/// Extracts the config value from a parsed `package.json`.
///
/// Returns a JavaScript value: a real config, or `Value::Null` for "absent".
/// A single string prop is first checked as a literal key. When present, its
/// value is returned raw, even when falsy. That lets a `package.json` prop of
/// `0` or `false` count as a match during search. Otherwise the prop is split
/// on dots and each segment is descended, and the final value is run through
/// the `|| null` coercion so any falsy result becomes `Value::Null`. A missing
/// intermediate short-circuits to null. Descending into an explicit `null` is
/// an error.
fn get_package_prop(prop: &PackageProp, obj: &Value) -> Result<Value, Error> {
    let keys: Vec<String> = match prop {
        PackageProp::Single(s) => {
            if let Value::Object(map) = obj {
                if let Some(v) = map.get(s) {
                    // Fast path: literal key present, returned raw.
                    return Ok(v.clone());
                }
            }
            s.split('.').map(|p| p.to_string()).collect()
        }
        PackageProp::Path(parts) => parts.clone(),
    };

    // Reduce over the path. `None` models a missing (undefined) intermediate,
    // which short-circuits. An explicit null with a remaining key is an error.
    let mut acc: Option<&Value> = Some(obj);
    for key in &keys {
        match acc {
            None => break,
            Some(Value::Null) => {
                return Err(Error::NullInPropPath { key: key.clone() });
            }
            Some(Value::Object(map)) => {
                acc = map.get(key);
            }
            Some(_) => {
                // Reading a key off a non-object yields undefined in JS.
                acc = None;
            }
        }
    }

    // Final `|| null`: any falsy result becomes null.
    Ok(match acc {
        Some(v) if is_truthy(v) => v.clone(),
        _ => Value::Null,
    })
}

/// Reports JavaScript truthiness for the values a JSON document can hold.
fn is_truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::String(s) => !s.is_empty(),
        Value::Number(n) => n.as_f64().map(|f| f != 0.0 && !f.is_nan()).unwrap_or(true),
        Value::Array(_) | Value::Object(_) => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn extname_matches_node_rules() {
        // Leading dot with no other dot has no extension.
        assert_eq!(loader_key(".foorc"), "noExt");
        assert_eq!(loader_key(".config/foorc"), "noExt");
        // A later dot wins.
        assert_eq!(loader_key(".foorc.json"), ".json");
        assert_eq!(loader_key("foo.config.js"), ".js");
        assert_eq!(loader_key("noExtension"), "noExt");
        assert_eq!(loader_key("package.json"), ".json");
        // Trailing dot.
        assert_eq!(loader_key("file."), ".");
    }

    #[test]
    fn is_blank_handles_whitespace_and_bom() {
        assert!(is_blank(""));
        assert!(is_blank("   \n\t  "));
        assert!(is_blank("\u{FEFF}"));
        assert!(is_blank("\u{00A0}"));
        assert!(!is_blank("x"));
        assert!(!is_blank("  a  "));
    }

    #[test]
    fn parent_dir_reaches_root() {
        let root = PathBuf::from(std::path::MAIN_SEPARATOR.to_string());
        assert_eq!(parent_dir(&root), root);
        let p = PathBuf::from("/a/b");
        assert_eq!(parent_dir(&p), PathBuf::from("/a"));
    }

    #[test]
    fn resolve_folds_dot_segments() {
        let base = Path::new("/work");
        assert_eq!(resolve(base, Path::new("a/b")), PathBuf::from("/work/a/b"));
        assert_eq!(
            resolve(base, Path::new("./a/../b")),
            PathBuf::from("/work/b")
        );
        assert_eq!(resolve(base, Path::new("/abs")), PathBuf::from("/abs"));
    }

    #[test]
    fn package_prop_fast_path_returns_literal_key() {
        let obj = json!({"a.b": 1, "a": {"b": 2}});
        let v = get_package_prop(&PackageProp::Single("a.b".to_string()), &obj).unwrap();
        assert_eq!(v, json!(1));
    }

    #[test]
    fn package_prop_dotted_path_collapses_falsy() {
        let obj = json!({"a": {"b": 0}});
        let v = get_package_prop(&PackageProp::Single("a.b".to_string()), &obj).unwrap();
        assert_eq!(v, Value::Null);
    }

    #[test]
    fn package_prop_missing_intermediate_is_null() {
        let obj = json!({"a": {}});
        let v = get_package_prop(&PackageProp::Single("a.x.y".to_string()), &obj).unwrap();
        assert_eq!(v, Value::Null);
    }

    #[test]
    fn package_prop_null_intermediate_errors() {
        let obj = json!({"a": null});
        let err = get_package_prop(&PackageProp::Single("a.b".to_string()), &obj).unwrap_err();
        assert!(matches!(err, Error::NullInPropPath { .. }));
    }
}
