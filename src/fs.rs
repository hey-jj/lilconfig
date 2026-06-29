//! Filesystem access behind a trait so callers can supply their own backing.
//!
//! Search and load only need two operations: check whether a path exists, and
//! read a file to text. The searcher takes its filesystem through the [`Fs`]
//! trait, so a caller can swap in an in-memory or sandboxed implementation.
//! [`RealFs`] hits the disk.

use std::io;
use std::path::Path;

/// The two filesystem operations search and load depend on.
pub trait Fs: Send + Sync {
    /// Reports whether `path` exists.
    ///
    /// `Ok` means the path is present. An `Err` means the search skips this
    /// place and moves on. A present but unreadable file reports `Ok` here, so
    /// the read that follows surfaces the permission error rather than skipping
    /// the file.
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
        // Test existence only. A present but unreadable file passes here, so the
        // later read surfaces the permission error instead of being skipped.
        std::fs::metadata(path).map(|_| ())
    }

    fn read_to_string(&self, path: &Path) -> io::Result<String> {
        let bytes = std::fs::read(path)?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }
}
