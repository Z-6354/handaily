//! 人物 / 人设 manifest 读-改-写串行化，避免并发 IPC 覆盖丢失条目。

use std::path::Path;
use std::sync::{Mutex, OnceLock};

static LOCK: OnceLock<Mutex<()>> = OnceLock::new();

pub fn with_lock<F, T>(f: F) -> T
where
    F: FnOnce() -> T,
{
    let guard = LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let _guard = guard;
    f()
}

pub fn atomic_write(path: &Path, contents: &str) -> Result<(), String> {
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, contents).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())
}
