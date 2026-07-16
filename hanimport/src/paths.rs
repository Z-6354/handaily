use std::path::{Path, PathBuf};

const REPO_DATA_LIVE2D: &str = "data/live2d";
const REPO_LEGACY_LIVE2D: &str = "live2d";
const REPO_DATA_IMPORT_PLAN: &str = "data/import/live2d-plan.json";
const REPO_DATA_WIKI_DB: &str = "data/wiki/blhx.sqlite";

/// HANDAILY monorepo root (contains `hanpet/` and `data/`).
pub fn project_root() -> PathBuf {
    if let Ok(p) = std::env::var("HANDAILY_ROOT") {
        let path = PathBuf::from(p.trim());
        if path.is_dir() {
            return path;
        }
    }
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest.join("..")
}

pub fn default_model_unpacked_output() -> PathBuf {
    project_root().join("data/model/unpacked")
}

/// Pick output root from input location when caller did not override `--output`.
pub fn resolve_unpack_output(input: &Path, explicit: Option<PathBuf>) -> PathBuf {
    if let Some(path) = explicit {
        return path;
    }
    let input_str = input.to_string_lossy().replace('\\', "/").to_ascii_lowercase();
    if input_str.contains("data/model") || input_str.contains("/model/") {
        return default_model_unpacked_output();
    }
    default_live2d_output()
}

/// Default Spine workspace: `HANDAILY_LIVE2D_PATH` → `data/live2d` → legacy `live2d/`.
pub fn default_live2d_output() -> PathBuf {
    if let Ok(p) = std::env::var("HANDAILY_LIVE2D_PATH") {
        let path = PathBuf::from(p.trim());
        if !path.as_os_str().is_empty() {
            return path;
        }
    }
    let root = project_root();
    for rel in [REPO_DATA_LIVE2D, REPO_LEGACY_LIVE2D] {
        let candidate = root.join(rel);
        if candidate.is_dir() {
            return candidate;
        }
    }
    root.join(REPO_DATA_LIVE2D)
}

/// Default import plan: `HANDAILY_LIVE2D_PLAN` → `data/import/live2d-plan.json`.
pub fn default_import_plan_path() -> PathBuf {
    if let Ok(p) = std::env::var("HANDAILY_LIVE2D_PLAN") {
        let path = PathBuf::from(p.trim());
        if !path.as_os_str().is_empty() {
            return path;
        }
    }
    project_root().join(REPO_DATA_IMPORT_PLAN)
}

/// Default BWIKI SQLite: `BLHX_WIKI_DB_PATH` → `data/wiki/blhx.sqlite` → mcp fallback.
#[allow(dead_code)]
pub fn default_wiki_db_path() -> PathBuf {
    if let Ok(p) = std::env::var("BLHX_WIKI_DB_PATH") {
        let path = PathBuf::from(p.trim());
        if !path.as_os_str().is_empty() {
            return path;
        }
    }
    let root = project_root();
    for rel in [REPO_DATA_WIKI_DB, "mcp/blhx-wiki/data/blhx.sqlite"] {
        let candidate = root.join(rel);
        if candidate.is_file() {
            return candidate;
        }
    }
    root.join(REPO_DATA_WIKI_DB)
}

pub fn blhx_wiki_dir() -> PathBuf {
    project_root().join("mcp/blhx-wiki")
}

pub fn ensure_dir(path: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(path)
}

pub fn ensure_parent_dir(path: &Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_root_contains_hanpet() {
        let root = project_root();
        assert!(root.join("hanpet/package.json").is_file());
        assert!(root.join("data").is_dir());
    }

    #[test]
    fn resolve_unpack_output_for_model_inbox() {
        let root = project_root();
        let input = root.join("data/model/azurlane/custom");
        assert_eq!(
            resolve_unpack_output(&input, None),
            root.join("data/model/unpacked")
        );
    }

    #[test]
    fn default_import_plan_under_data() {
        let root = project_root();
        assert_eq!(
            default_import_plan_path(),
            root.join("data/import/live2d-plan.json")
        );
    }

    #[test]
    fn default_wiki_db_resolves_sqlite() {
        let root = project_root();
        let path = default_wiki_db_path();
        assert!(path.starts_with(&root));
        assert!(path.ends_with("blhx.sqlite"));
    }
}
