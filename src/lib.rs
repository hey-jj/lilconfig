//! Find and load a tool's config file by searching up the directory tree.
//!
//! This crate locates a configuration file the way cosmiconfig does. Give it a
//! tool name and it derives a set of conventional filenames, walks up from a
//! starting directory, and returns the first file that exists and parses.
//!
//! Two entry points mirror the two I/O styles. [`SearcherBuilder::new`] builds a
//! synchronous [`Searcher`]. [`AsyncSearcherBuilder::new`] builds an
//! asynchronous [`AsyncSearcher`] with the same behavior. Both searchers expose
//! `search`, `load`, and three cache-clearing methods.
//!
//! # Example
//!
//! ```no_run
//! use lilconfig::SearcherBuilder;
//!
//! let searcher = SearcherBuilder::new("myapp").build()?;
//! if let Some(found) = searcher.search_cwd()? {
//!     println!("config at {}: {:?}", found.filepath.display(), found.config);
//! }
//! # Ok::<(), lilconfig::Error>(())
//! ```
//!
//! # Loaders
//!
//! A loader turns file text into a [`serde_json::Value`]. The defaults parse
//! JSON for `.json` files and for extensionless files. Register more loaders to
//! support other formats. The JavaScript library also runs `.js`, `.cjs`, and
//! `.mjs` configs by executing them. That is not possible here, so those
//! extensions have no default loader. Supply your own if you need them.
//!
//! # Config values
//!
//! A [`SearchResult`] carries `config: Option<Value>`. `None` means the matched
//! file was empty, which mirrors a JavaScript `undefined`. `Some(Value::Null)`
//! is an explicit null config. The whole `Option<SearchResult>` is `None` when
//! the search found nothing.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod core;
mod error;
mod fs;
mod loaders;
mod options;

pub use crate::core::{PackageProp, SearchResult, Transform};
pub use crate::error::Error;
pub use crate::fs::{Fs, RealFs};
pub use crate::loaders::{default_loaders, json_loader, loader, Loader, LoaderFn, Loaders};
pub use crate::options::{AsyncSearcherBuilder, SearcherBuilder};

use std::fmt;
use std::path::Path;
use std::sync::Arc;

use crate::core::Core;

/// A synchronous searcher.
///
/// Build one with [`SearcherBuilder::build`]. The methods read and parse files
/// on the calling thread.
pub struct Searcher<F: Fs = RealFs> {
    core: Arc<Core<F>>,
    cwd: std::path::PathBuf,
}

impl<F: Fs> Searcher<F> {
    pub(crate) fn new(core: Core<F>, cwd: std::path::PathBuf) -> Self {
        Self {
            core: Arc::new(core),
            cwd,
        }
    }

    /// Walks up from `search_from`, returning the first qualifying config.
    ///
    /// A relative `search_from` is resolved against the working directory, the
    /// same way `load` resolves its path.
    pub fn search(&self, search_from: impl AsRef<Path>) -> Result<Option<SearchResult>, Error> {
        let from = crate::core::resolve(&self.cwd, search_from.as_ref());
        self.core.search(&from)
    }

    /// Searches from the working directory the searcher was built with.
    pub fn search_cwd(&self) -> Result<Option<SearchResult>, Error> {
        self.core.search(&self.cwd)
    }

    /// Loads one config file by path, resolved against the working directory.
    pub fn load(&self, filepath: impl AsRef<Path>) -> Result<Option<SearchResult>, Error> {
        self.core.load(&self.cwd, filepath.as_ref())
    }

    /// Empties the load cache.
    pub fn clear_load_cache(&self) {
        self.core.clear_load_cache();
    }

    /// Empties the search cache.
    pub fn clear_search_cache(&self) {
        self.core.clear_search_cache();
    }

    /// Empties both caches.
    pub fn clear_caches(&self) {
        self.core.clear_caches();
    }
}

impl<F: Fs> fmt::Debug for Searcher<F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Searcher").field("cwd", &self.cwd).finish()
    }
}

/// An asynchronous searcher.
///
/// Build one with [`AsyncSearcherBuilder::build`]. The methods are `async` and
/// return the same results as the synchronous searcher. The underlying
/// filesystem access is blocking, so the futures resolve without yielding.
pub struct AsyncSearcher<F: Fs = RealFs> {
    core: Arc<Core<F>>,
    cwd: std::path::PathBuf,
}

impl<F: Fs> AsyncSearcher<F> {
    pub(crate) fn new(core: Core<F>, cwd: std::path::PathBuf) -> Self {
        Self {
            core: Arc::new(core),
            cwd,
        }
    }

    /// Walks up from `search_from`, returning the first qualifying config.
    ///
    /// A relative `search_from` is resolved against the working directory, the
    /// same way `load` resolves its path.
    pub async fn search(
        &self,
        search_from: impl AsRef<Path>,
    ) -> Result<Option<SearchResult>, Error> {
        let from = crate::core::resolve(&self.cwd, search_from.as_ref());
        self.core.search(&from)
    }

    /// Searches from the working directory the searcher was built with.
    pub async fn search_cwd(&self) -> Result<Option<SearchResult>, Error> {
        self.core.search(&self.cwd)
    }

    /// Loads one config file by path, resolved against the working directory.
    pub async fn load(&self, filepath: impl AsRef<Path>) -> Result<Option<SearchResult>, Error> {
        self.core.load(&self.cwd, filepath.as_ref())
    }

    /// Empties the load cache.
    pub fn clear_load_cache(&self) {
        self.core.clear_load_cache();
    }

    /// Empties the search cache.
    pub fn clear_search_cache(&self) {
        self.core.clear_search_cache();
    }

    /// Empties both caches.
    pub fn clear_caches(&self) {
        self.core.clear_caches();
    }
}

impl<F: Fs> fmt::Debug for AsyncSearcher<F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AsyncSearcher")
            .field("cwd", &self.cwd)
            .finish()
    }
}
