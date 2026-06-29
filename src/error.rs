//! Error type for search and load.

use std::fmt;
use std::io;
use std::path::PathBuf;

/// Everything that can go wrong while searching for or loading a config.
///
/// Construction-time problems (a search place with no loader) surface when you
/// build a [`Searcher`](crate::Searcher), so they appear as the `Err` of the
/// factory functions. Runtime problems (missing files, parse failures, loader
/// errors, prop traversal through `null`) surface from `search` and `load`.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// `load` was called with an empty path.
    ///
    /// Message: `load must pass a non-empty string`.
    EmptyFilePath,

    /// A search place has no registered loader for its extension.
    ///
    /// Raised while building the searcher. The message interpolates the whole
    /// search place, for example `Missing loader for extension "file.coffee"`.
    MissingLoaderForPlace {
        /// The offending search place, verbatim.
        place: String,
    },

    /// No loader is registered for the extension of a file being loaded.
    ///
    /// Raised at load time. The message names the extension key, for example
    /// `No loader specified for extension ".coffee"`.
    NoLoaderForExtension {
        /// The loader key, an extension like `.coffee` or the literal `noExt`.
        key: String,
    },

    /// Reading a file failed.
    ///
    /// Wraps the underlying [`io::Error`] and keeps the path for context. A
    /// missing file shows up here with [`io::ErrorKind::NotFound`].
    Io {
        /// The path that could not be read.
        path: PathBuf,
        /// The underlying I/O error.
        source: io::Error,
    },

    /// A loader failed to parse or produce a value.
    ///
    /// Carries the path and a message. The default JSON loader puts the
    /// serde_json parse error here.
    Loader {
        /// The file the loader was given.
        path: PathBuf,
        /// What the loader reported.
        message: String,
    },

    /// A package-prop path descended into an explicit `null`.
    ///
    /// JavaScript throws a TypeError when reading a key off `null`. The same
    /// shape of traversal returns this error.
    NullInPropPath {
        /// The remaining key that could not be read off `null`.
        key: String,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::EmptyFilePath => write!(f, "load must pass a non-empty string"),
            Error::MissingLoaderForPlace { place } => {
                write!(f, "Missing loader for extension \"{place}\"")
            }
            Error::NoLoaderForExtension { key } => {
                write!(f, "No loader specified for extension \"{key}\"")
            }
            Error::Io { path, source } => {
                write!(f, "failed to read {}: {source}", path.display())
            }
            Error::Loader { path, message } => {
                write!(f, "failed to load {}: {message}", path.display())
            }
            Error::NullInPropPath { key } => {
                write!(f, "Cannot read properties of null (reading '{key}')")
            }
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}
