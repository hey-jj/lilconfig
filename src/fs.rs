//! Filesystem access behind a trait so tests can count calls.
//!
//! Search and load only need two operations: check whether a path can be
//! accessed, and read a file to text. The cache tests assert exact call counts,
//! so the searcher takes its filesystem as a trait object. [`RealFs`] hits the
//! disk. [`CountingFs`] wraps another filesystem and tallies calls.

use std::io;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// The two filesystem operations search and load depend on.
pub trait Fs: Send + Sync {
    /// Reports whether `path` exists and is reachable.
    ///
    /// Mirrors `fs.access`. An `Err` means "skip this path", matching the JS
    /// behavior where any access failure continues the search.
    fn access(&self, path: &Path) -> io::Result<()>;

    /// Reads `path` as UTF-8, replacing invalid bytes.
    ///
    /// Mirrors `String(await readFile(path))`, which decodes a buffer as UTF-8
    /// and substitutes U+FFFD for invalid sequences instead of failing.
    fn read_to_string(&self, path: &Path) -> io::Result<String>;
}

/// Reads from the real filesystem.
#[derive(Debug, Default, Clone, Copy)]
pub struct RealFs;

impl Fs for RealFs {
    fn access(&self, path: &Path) -> io::Result<()> {
        // Opening for read approximates fs.access readability check.
        std::fs::File::open(path).map(|_| ())
    }

    fn read_to_string(&self, path: &Path) -> io::Result<String> {
        let bytes = std::fs::read(path)?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }
}

/// Wraps another filesystem and counts `access` and `read` calls.
///
/// The counters are shared, so clones of a `CountingFs` report the same totals.
/// Tests use this to assert the cache avoids redundant filesystem work.
#[derive(Clone)]
pub struct CountingFs<F: Fs> {
    inner: F,
    access_count: Arc<AtomicUsize>,
    read_count: Arc<AtomicUsize>,
}

impl<F: Fs> CountingFs<F> {
    /// Wraps `inner` with fresh counters.
    pub fn new(inner: F) -> Self {
        Self {
            inner,
            access_count: Arc::new(AtomicUsize::new(0)),
            read_count: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Returns how many times `access` has run.
    pub fn access_count(&self) -> usize {
        self.access_count.load(Ordering::SeqCst)
    }

    /// Returns how many times `read_to_string` has run.
    pub fn read_count(&self) -> usize {
        self.read_count.load(Ordering::SeqCst)
    }
}

impl<F: Fs> Fs for CountingFs<F> {
    fn access(&self, path: &Path) -> io::Result<()> {
        self.access_count.fetch_add(1, Ordering::SeqCst);
        self.inner.access(path)
    }

    fn read_to_string(&self, path: &Path) -> io::Result<String> {
        self.read_count.fetch_add(1, Ordering::SeqCst);
        self.inner.read_to_string(path)
    }
}
