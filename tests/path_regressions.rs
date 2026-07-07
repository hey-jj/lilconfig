use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};

use lilconfig::SearcherBuilder;

fn cwd_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn lock_cwd() -> MutexGuard<'static, ()> {
    cwd_lock().lock().unwrap_or_else(|err| err.into_inner())
}

struct CurrentDirGuard {
    old: PathBuf,
}

impl CurrentDirGuard {
    fn change_to(path: &Path) -> Self {
        let old = env::current_dir().unwrap();
        env::set_current_dir(path).unwrap();
        Self { old }
    }
}

impl Drop for CurrentDirGuard {
    fn drop(&mut self) {
        let _ = env::set_current_dir(&self.old);
    }
}

#[test]
fn relative_stop_dir_does_not_search_parent_of_process_cwd() {
    let _lock = lock_cwd();
    let tmp = tempfile::tempdir().unwrap();
    let process_cwd = tmp.path().join("cwd");
    fs::create_dir(&process_cwd).unwrap();
    fs::write(
        tmp.path().join("package.json"),
        r#"{"x":{"above_process_cwd":true}}"#,
    )
    .unwrap();

    let _cwd = CurrentDirGuard::change_to(&process_cwd);
    let searcher = SearcherBuilder::new("x")
        .stop_dir(".")
        .search_places(["package.json"])
        .cache(false)
        .build()
        .unwrap();

    assert_eq!(searcher.search_cwd().unwrap(), None);
}

#[test]
fn absolute_parent_segments_stop_at_root() {
    let _lock = lock_cwd();
    let dir = Path::new("/tmp/lilconfig-normalize-case");
    let config = dir.join("config.json");
    fs::create_dir_all(dir).unwrap();
    fs::write(&config, r#"{"ok":true}"#).unwrap();

    let _cwd = CurrentDirGuard::change_to(Path::new("/"));
    let searcher = SearcherBuilder::new("x").build().unwrap();
    let result = searcher
        .load("/../tmp/lilconfig-normalize-case/config.json")
        .unwrap()
        .unwrap();

    assert_eq!(result.filepath, config);
}
