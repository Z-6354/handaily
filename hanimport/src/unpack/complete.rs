//! Complete vs half-finished unpack output detection.
//! Hx variants (*_hx): never unpack; purge leftover output dirs.

use std::fs;
use std::path::{Path, PathBuf};

/// True when slug ends with `_hx` (case-insensitive).
pub fn is_hx_slug(slug: &str) -> bool {
    let s = slug.trim().to_ascii_lowercase();
    !s.is_empty() && s.ends_with("_hx")
}

/// Remove immediate child dirs of `output_root` whose names end with `_hx`.
pub fn purge_hx_output_dirs(output_root: &Path) -> Result<Vec<String>, String> {
    if !output_root.is_dir() {
        return Ok(Vec::new());
    }
    let mut removed = Vec::new();
    for entry in fs::read_dir(output_root).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if is_hx_slug(&name) {
            fs::remove_dir_all(&path).map_err(|e| {
                format!("failed to remove hx dir {}: {e}", path.display())
            })?;
            removed.push(name);
        }
    }
    Ok(removed)
}

fn non_empty(path: &Path) -> bool {
    fs::metadata(path)
        .map(|m| m.is_file() && m.len() > 0)
        .unwrap_or(false)
}

/// Live2D: model3.json + moc3; Spine: atlas + skel(.bytes). Half-finished → false.
pub fn is_unpack_complete(out_dir: &Path, slug: &str) -> bool {
    if !out_dir.is_dir() {
        return false;
    }
    let moc3 = out_dir.join(format!("{slug}.moc3"));
    let model3 = out_dir.join(format!("{slug}.model3.json"));
    if non_empty(&moc3) && non_empty(&model3) {
        return true;
    }
    let atlas = out_dir.join(format!("{slug}.atlas"));
    let skel = out_dir.join(format!("{slug}.skel"));
    let skel_bytes = out_dir.join(format!("{slug}.skel.bytes"));
    non_empty(&atlas) && (non_empty(&skel) || non_empty(&skel_bytes))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrepareAction {
    /// Already complete — skip unpack.
    Skip,
    /// Missing or incomplete (deleted) — ready to unpack.
    Ready,
}

/// If complete → Skip. If incomplete leftover → delete then Ready.
pub fn prepare_unpack_dir(out_dir: &Path, slug: &str) -> Result<PrepareAction, String> {
    if is_unpack_complete(out_dir, slug) {
        return Ok(PrepareAction::Skip);
    }
    if out_dir.exists() {
        fs::remove_dir_all(out_dir).map_err(|e| {
            format!(
                "failed to remove incomplete unpack dir {}: {e}",
                out_dir.display()
            )
        })?;
    }
    Ok(PrepareAction::Ready)
}

pub fn output_dir_for(output_root: &Path, slug: &str) -> PathBuf {
    output_root.join(slug)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn live2d_complete_skips() {
        let dir = tempdir().unwrap();
        let out = dir.path().join("ship_2");
        fs::create_dir_all(&out).unwrap();
        fs::write(out.join("ship_2.moc3"), b"moc").unwrap();
        fs::write(out.join("ship_2.model3.json"), b"{}").unwrap();
        assert!(is_unpack_complete(&out, "ship_2"));
        assert_eq!(prepare_unpack_dir(&out, "ship_2").unwrap(), PrepareAction::Skip);
        assert!(out.is_dir());
    }

    #[test]
    fn half_finished_is_deleted() {
        let dir = tempdir().unwrap();
        let out = dir.path().join("half");
        fs::create_dir_all(&out).unwrap();
        fs::write(out.join("half.moc3"), b"moc").unwrap();
        assert!(!is_unpack_complete(&out, "half"));
        assert_eq!(prepare_unpack_dir(&out, "half").unwrap(), PrepareAction::Ready);
        assert!(!out.exists());
    }

    #[test]
    fn spine_complete() {
        let dir = tempdir().unwrap();
        let out = dir.path().join("qiye");
        fs::create_dir_all(&out).unwrap();
        fs::write(out.join("qiye.atlas"), b"a").unwrap();
        fs::write(out.join("qiye.skel"), b"s").unwrap();
        assert!(is_unpack_complete(&out, "qiye"));
    }

    #[test]
    fn hx_slug_suffix() {
        assert!(is_hx_slug("ankeleiqi_2_hx"));
        assert!(is_hx_slug("Z23_HX"));
        assert!(!is_hx_slug("ankeleiqi_2"));
        assert!(!is_hx_slug("foo_hx_bar"));
    }

    #[test]
    fn purge_hx_dirs() {
        let dir = tempdir().unwrap();
        let keep = dir.path().join("qiye");
        let hx = dir.path().join("qiye_2_hx");
        fs::create_dir_all(&keep).unwrap();
        fs::create_dir_all(&hx).unwrap();
        let removed = purge_hx_output_dirs(dir.path()).unwrap();
        assert_eq!(removed, vec!["qiye_2_hx".to_string()]);
        assert!(keep.is_dir());
        assert!(!hx.exists());
    }
}
