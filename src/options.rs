//! Builders that resolve options and construct a searcher.
//!
//! [`Lilconfig`] configures a synchronous searcher. [`AsyncLilconfig`] configures
//! an asynchronous one. They share every option. Each `build` resolves defaults,
//! validates that every search place has a loader, and returns the searcher.

use std::path::PathBuf;

use crate::core::{Core, LilconfigResult, PackageProp, Resolved, Transform};
use crate::error::Error;
use crate::fs::{Fs, RealFs};
use crate::loaders::{default_loaders, default_loaders_sync, Loader, Loaders};
use crate::{AsyncSearcher, Searcher};

/// Builds the default search places for `name`.
///
/// The skeleton matches the conventional cosmiconfig layout: `package.json`,
/// dot-rc files, files under `.config/`, and a `name.config.*` file. Only
/// extensions with a default loader appear, so this lists `.json`, the
/// extensionless `.config/{name}rc`, but not `.js`, `.cjs`, or `.mjs`.
fn default_search_places(name: &str) -> Vec<String> {
    vec![
        "package.json".to_string(),
        format!(".{name}rc.json"),
        format!(".config/{name}rc"),
        format!(".config/{name}rc.json"),
    ]
}

/// Shared option state for both builders.
struct Builder {
    name: String,
    cwd: Option<PathBuf>,
    stop_dir: Option<PathBuf>,
    search_places: Option<Vec<String>>,
    ignore_empty_search_places: bool,
    cache: bool,
    transform: Option<Transform>,
    package_prop: Option<PackageProp>,
    user_loaders: Loaders,
}

impl Builder {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            cwd: None,
            stop_dir: None,
            search_places: None,
            ignore_empty_search_places: true,
            cache: true,
            transform: None,
            package_prop: None,
            user_loaders: Loaders::new(),
        }
    }

    fn resolve(self, base_loaders: Loaders) -> (Resolved, PathBuf) {
        let cwd = self
            .cwd
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_else(|| PathBuf::from("."));

        let stop_dir = self
            .stop_dir
            .or_else(dirs::home_dir)
            .unwrap_or_else(|| PathBuf::from("/"));

        let search_places = self
            .search_places
            .unwrap_or_else(|| default_search_places(&self.name));

        let transform: Transform = self
            .transform
            .unwrap_or_else(|| std::sync::Arc::new(|r: LilconfigResult| Ok(r)));

        let package_prop = self
            .package_prop
            .unwrap_or_else(|| PackageProp::Single(self.name.clone()));

        // Default loaders stay present for keys the caller did not override.
        let mut loaders = base_loaders;
        for (k, v) in self.user_loaders {
            loaders.insert(k, v);
        }

        let resolved = Resolved {
            stop_dir,
            search_places,
            ignore_empty_search_places: self.ignore_empty_search_places,
            cache: self.cache,
            transform,
            package_prop,
            loaders,
        };
        (resolved, cwd)
    }
}

/// Configures and builds a synchronous [`Searcher`].
pub struct Lilconfig {
    inner: Builder,
}

/// Configures and builds an asynchronous [`AsyncSearcher`].
pub struct AsyncLilconfig {
    inner: Builder,
}

macro_rules! shared_setters {
    () => {
        /// Sets the working directory used to resolve `load` paths and the
        /// default search origin. Defaults to the process working directory.
        pub fn cwd(mut self, cwd: impl Into<PathBuf>) -> Self {
            self.inner.cwd = Some(cwd.into());
            self
        }

        /// Sets the directory where the upward walk stops. Defaults to the
        /// user's home directory.
        pub fn stop_dir(mut self, dir: impl Into<PathBuf>) -> Self {
            self.inner.stop_dir = Some(dir.into());
            self
        }

        /// Replaces the list of filenames tried in each directory.
        pub fn search_places<I, S>(mut self, places: I) -> Self
        where
            I: IntoIterator<Item = S>,
            S: Into<String>,
        {
            self.inner.search_places = Some(places.into_iter().map(Into::into).collect());
            self
        }

        /// Sets whether empty files are skipped during search. Default is true.
        pub fn ignore_empty_search_places(mut self, ignore: bool) -> Self {
            self.inner.ignore_empty_search_places = ignore;
            self
        }

        /// Turns the two caches on or off. Default is on.
        pub fn cache(mut self, on: bool) -> Self {
            self.inner.cache = on;
            self
        }

        /// Sets the key in `package.json` that holds the config. Default is the
        /// tool name. A single string is first checked as a literal key.
        pub fn package_prop(mut self, prop: PackageProp) -> Self {
            self.inner.package_prop = Some(prop);
            self
        }

        /// Registers or overrides a loader for one extension key.
        ///
        /// The key is an extension with a leading dot, like `.toml`, or the
        /// literal `noExt` for extensionless files.
        pub fn loader(mut self, key: impl Into<String>, loader: Loader) -> Self {
            self.inner.user_loaders.insert(key.into(), loader);
            self
        }
    };
}

impl Lilconfig {
    /// Starts configuring a synchronous searcher for `name`.
    pub fn new(name: impl AsRef<str>) -> Self {
        Self {
            inner: Builder::new(name.as_ref()),
        }
    }

    shared_setters!();

    /// Sets the result transform.
    ///
    /// The transform runs on every outcome, including the not-found case where
    /// it receives `None`.
    pub fn transform<T>(mut self, transform: T) -> Self
    where
        T: Fn(LilconfigResult) -> Result<LilconfigResult, Error> + Send + Sync + 'static,
    {
        self.inner.transform = Some(std::sync::Arc::new(transform));
        self
    }

    /// Builds the searcher on the real filesystem.
    ///
    /// Fails if a search place has no registered loader.
    pub fn build(self) -> Result<Searcher<RealFs>, Error> {
        self.build_with_fs(RealFs)
    }

    /// Builds the searcher on a supplied filesystem.
    pub fn build_with_fs<F: Fs>(self, fs: F) -> Result<Searcher<F>, Error> {
        let (resolved, cwd) = self.inner.resolve(default_loaders_sync());
        let core = Core::new(resolved, fs)?;
        Ok(Searcher::new(core, cwd))
    }
}

impl AsyncLilconfig {
    /// Starts configuring an asynchronous searcher for `name`.
    pub fn new(name: impl AsRef<str>) -> Self {
        Self {
            inner: Builder::new(name.as_ref()),
        }
    }

    shared_setters!();

    /// Sets the result transform.
    ///
    /// The transform runs on every outcome, including the not-found case where
    /// it receives `None`.
    pub fn transform<T>(mut self, transform: T) -> Self
    where
        T: Fn(LilconfigResult) -> Result<LilconfigResult, Error> + Send + Sync + 'static,
    {
        self.inner.transform = Some(std::sync::Arc::new(transform));
        self
    }

    /// Builds the searcher on the real filesystem.
    ///
    /// Fails if a search place has no registered loader.
    pub fn build(self) -> Result<AsyncSearcher<RealFs>, Error> {
        self.build_with_fs(RealFs)
    }

    /// Builds the searcher on a supplied filesystem.
    pub fn build_with_fs<F: Fs>(self, fs: F) -> Result<AsyncSearcher<F>, Error> {
        let (resolved, cwd) = self.inner.resolve(default_loaders());
        let core = Core::new(resolved, fs)?;
        Ok(AsyncSearcher::new(core, cwd))
    }
}
