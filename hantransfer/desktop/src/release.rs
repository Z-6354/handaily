use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::paths;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppRelease {
    pub version_name: String,
    pub build: u32,
    pub display: String,
    pub filename: String,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseListResponse {
    pub latest: Option<AppRelease>,
    pub files: Vec<AppRelease>,
    pub release_dir: String,
}

#[derive(Debug, Deserialize)]
pub struct SetLatestRequest {
    pub filename: String,
    pub version_name: Option<String>,
    pub build: Option<u32>,
}

pub fn apk_release_dir() -> PathBuf {
    paths::apk_release_dir()
}

pub fn ensure_release_dir() -> Result<PathBuf, String> {
    let dir = apk_release_dir();
    paths::ensure_dir(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

pub fn find_latest_apk() -> Option<AppRelease> {
    let dir = apk_release_dir();
    let json_path = dir.join("latest.json");
    if json_path.is_file() {
        if let Ok(text) = fs::read_to_string(&json_path) {
            let text = text.strip_prefix('\u{FEFF}').unwrap_or(&text);
            if let Ok(parsed) = serde_json::from_str::<AppRelease>(text) {
                let file = dir.join(&parsed.filename);
                if file.is_file() {
                    return Some(refresh_size(parsed, &file));
                }
            }
        }
    }
    scan_highest_build_apk(&dir)
}

pub fn list_all_apks() -> Result<ReleaseListResponse, String> {
    let dir = ensure_release_dir()?;
    let latest = find_latest_apk();
    let mut files = Vec::new();
    for entry in fs::read_dir(&dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("apk") {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| "invalid apk filename".to_string())?;
        let info = apk_info_from_path(name, &path)?;
        files.push(info);
    }
    files.sort_by(|a, b| b.build.cmp(&a.build).then_with(|| b.filename.cmp(&a.filename)));
    Ok(ReleaseListResponse {
        latest,
        files,
        release_dir: dir.display().to_string(),
    })
}

pub fn set_latest_apk(req: SetLatestRequest) -> Result<AppRelease, String> {
    let dir = ensure_release_dir()?;
    let filename = sanitize_apk_filename(&req.filename)?;
    let path = dir.join(&filename);
    if !path.is_file() {
        return Err(format!("APK not found: {filename}"));
    }
    let version_name = req
        .version_name
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| infer_version_name(&filename));
    let build = req.build.unwrap_or_else(|| next_build_number(&dir, &filename));
    let release = write_latest_manifest(&dir, &path, &filename, &version_name, build)?;
    copy_latest_alias(&dir, &path)?;
    Ok(release)
}

pub fn save_uploaded_apk(bytes: &[u8], original_name: &str, set_latest: bool) -> Result<AppRelease, String> {
    let dir = ensure_release_dir()?;
    let filename = sanitize_apk_filename(original_name)?;
    let path = dir.join(&filename);
    fs::write(&path, bytes).map_err(|e| format!("write apk failed: {e}"))?;
    if set_latest {
        return set_latest_apk(SetLatestRequest {
            filename,
            version_name: None,
            build: None,
        });
    }
    apk_info_from_path(&filename, &path)
}

fn refresh_size(mut info: AppRelease, path: &Path) -> AppRelease {
    if let Ok(meta) = fs::metadata(path) {
        info.size = meta.len();
    }
    info
}

fn write_latest_manifest(
    dir: &Path,
    path: &Path,
    filename: &str,
    version_name: &str,
    build: u32,
) -> Result<AppRelease, String> {
    let size = fs::metadata(path).map_err(|e| e.to_string())?.len();
    let release = AppRelease {
        version_name: version_name.to_string(),
        build,
        display: version_name.to_string(),
        filename: filename.to_string(),
        size,
    };
    let json = serde_json::to_string_pretty(&release).map_err(|e| e.to_string())?;
    fs::write(dir.join("latest.json"), json).map_err(|e| e.to_string())?;
    Ok(release)
}

fn copy_latest_alias(dir: &Path, src: &Path) -> Result<(), String> {
    let alias = dir.join("hantransfer-latest-debug.apk");
    fs::copy(src, alias).map_err(|e| e.to_string())?;
    Ok(())
}

fn apk_info_from_path(name: &str, path: &Path) -> Result<AppRelease, String> {
    if let Some(parsed) = parse_apk_filename(name, path) {
        return Ok(parsed);
    }
    let size = fs::metadata(path).map_err(|e| e.to_string())?.len();
    let version_name = infer_version_name(name);
    Ok(AppRelease {
        display: format!("{version_name} (?)"),
        version_name,
        build: 0,
        filename: name.to_string(),
        size,
    })
}

fn scan_highest_build_apk(dir: &Path) -> Option<AppRelease> {
    let mut best: Option<AppRelease> = None;
    let entries = fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("apk") {
            continue;
        }
        let name = path.file_name()?.to_str()?;
        let Some(parsed) = parse_apk_filename(name, &path) else {
            continue;
        };
        if best.as_ref().map(|b| parsed.build > b.build).unwrap_or(true) {
            best = Some(parsed);
        }
    }
    best
}

fn parse_apk_filename(name: &str, path: &Path) -> Option<AppRelease> {
    let mid = name.strip_prefix("hantransfer-")?.strip_suffix("-debug.apk")?;
    if let Some((version_name, build_str)) = mid.rsplit_once("-build") {
        let build = build_str.parse().ok()?;
        let size = fs::metadata(path).ok()?.len();
        return Some(AppRelease {
            display: format!("{version_name} ({build})"),
            version_name: version_name.to_string(),
            build,
            filename: name.to_string(),
            size,
        });
    }
    if semver_to_code(mid).is_some() {
        let version_name = mid.to_string();
        let build = semver_to_code(&version_name)?;
        let size = fs::metadata(path).ok()?.len();
        return Some(AppRelease {
            display: version_name.clone(),
            version_name,
            build,
            filename: name.to_string(),
            size,
        });
    }
    None
}

fn semver_to_code(version: &str) -> Option<u32> {
    let parts: Vec<u32> = version.split('.').filter_map(|p| p.parse().ok()).collect();
    if parts.len() != 3 {
        return None;
    }
    Some(parts[0] * 10000 + parts[1] * 100 + parts[2])
}

fn infer_version_name(filename: &str) -> String {
    let stem = filename.strip_suffix(".apk").unwrap_or(filename);
    if stem.starts_with("hantransfer-") {
        return crate::config::VERSION.to_string();
    }
    stem.to_string()
}

fn next_build_number(dir: &Path, filename: &str) -> u32 {
    if let Some(parsed) = parse_apk_filename(filename, &dir.join(filename)) {
        return parsed.build;
    }
    let mut max_build = 0u32;
    if let Ok(text) = fs::read_to_string(dir.join("latest.json")) {
        if let Ok(m) = serde_json::from_str::<AppRelease>(&text) {
            max_build = max_build.max(m.build);
        }
    }
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("apk") {
                continue;
            }
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if let Some(info) = parse_apk_filename(name, &path) {
                    max_build = max_build.max(info.build);
                }
            }
        }
    }
    max_build + 1
}

fn sanitize_apk_filename(name: &str) -> Result<String, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() || trimmed.contains("..") || trimmed.contains('/') || trimmed.contains('\\') {
        return Err("invalid apk filename".into());
    }
    if !trimmed.to_ascii_lowercase().ends_with(".apk") {
        return Err("file must be .apk".into());
    }
    Ok(trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_release_dir() -> PathBuf {
        let id = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("hantransfer-release-test-{id}"));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn find_latest_apk_in_repo() {
        let dir = apk_release_dir();
        assert!(
            find_latest_apk().is_some(),
            "expected APK under {} (dir exists={})",
            dir.display(),
            dir.is_dir()
        );
    }

    #[test]
    fn parse_semver_apk_name() {
        let dir = std::env::temp_dir();
        let path = dir.join("hantransfer-0.1.2-debug.apk");
        fs::write(&path, b"fake").unwrap();
        let info = parse_apk_filename("hantransfer-0.1.2-debug.apk", &path).unwrap();
        assert_eq!(info.version_name, "0.1.2");
        assert_eq!(info.build, 102);
        assert_eq!(info.display, "0.1.2");
        let _ = fs::remove_file(path);
    }

    #[test]
    fn parse_versioned_apk_name() {
        let dir = std::env::temp_dir();
        let path = dir.join("hantransfer-0.1.0-build3-debug.apk");
        fs::write(&path, b"fake").unwrap();
        let info = parse_apk_filename("hantransfer-0.1.0-build3-debug.apk", &path).unwrap();
        assert_eq!(info.version_name, "0.1.0");
        assert_eq!(info.build, 3);
        assert_eq!(info.display, "0.1.0 (3)");
        let _ = fs::remove_file(path);
    }

    #[test]
    fn set_latest_writes_manifest() {
        let dir = temp_release_dir();
        let apk = dir.join("hantransfer-0.1.0-build9-debug.apk");
        fs::write(&apk, b"apk-bytes").unwrap();
        let release =
            write_latest_manifest(&dir, &apk, "hantransfer-0.1.0-build9-debug.apk", "0.1.0", 9)
                .unwrap();
        assert_eq!(release.build, 9);
        let latest = fs::read_to_string(dir.join("latest.json")).unwrap();
        assert!(latest.contains("build9"));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn next_build_increments() {
        let dir = temp_release_dir();
        let apk1 = dir.join("hantransfer-0.1.0-build5-debug.apk");
        fs::write(&apk1, b"a").unwrap();
        write_latest_manifest(&dir, &apk1, "hantransfer-0.1.0-build5-debug.apk", "0.1.0", 5)
            .unwrap();
        assert_eq!(next_build_number(&dir, "custom.apk"), 6);
        let _ = fs::remove_dir_all(dir);
    }
}
