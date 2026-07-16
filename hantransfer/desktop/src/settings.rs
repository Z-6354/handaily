use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};

use crate::config;
use crate::paths;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingsFile {
    #[serde(default)]
    pub inbox_dir: Option<PathBuf>,
    #[serde(default)]
    pub auto_accept: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct DesktopSettings {
    pub inbox_dir: PathBuf,
    pub auto_accept: bool,
}

#[derive(Clone)]
pub struct SettingsStore {
    inner: Arc<RwLock<DesktopSettings>>,
}

impl SettingsStore {
    pub fn load() -> Self {
        let default_inbox = std::fs::canonicalize(paths::model_inbox())
            .unwrap_or_else(|_| paths::model_inbox());
        let defaults = DesktopSettings {
            inbox_dir: default_inbox.clone(),
            auto_accept: env_auto_accept(),
        };
        let loaded = read_settings_file().unwrap_or(SettingsFile {
            inbox_dir: None,
            auto_accept: None,
        });
        let settings = DesktopSettings {
            inbox_dir: normalize_inbox_dir(
                env_inbox_dir().or(loaded.inbox_dir),
                &default_inbox,
            ),
            auto_accept: loaded.auto_accept.unwrap_or(defaults.auto_accept),
        };
        let store = Self {
            inner: Arc::new(RwLock::new(settings.clone())),
        };
        let _ = persist(&settings);
        store
    }

    pub fn snapshot(&self) -> DesktopSettings {
        self.inner.read().expect("settings lock").clone()
    }

    pub fn inbox_dir(&self) -> PathBuf {
        self.snapshot().inbox_dir
    }

    pub fn auto_accept(&self) -> bool {
        self.snapshot().auto_accept
    }

    pub fn update(&self, inbox_dir: Option<PathBuf>, auto_accept: Option<bool>) -> Result<DesktopSettings, String> {
        let mut guard = self.inner.write().expect("settings lock");
        if let Some(dir) = inbox_dir {
            validate_inbox_dir(&dir)?;
            guard.inbox_dir = dir;
        }
        if let Some(v) = auto_accept {
            guard.auto_accept = v;
        }
        persist(&guard)?;
        Ok(guard.clone())
    }

    pub fn ensure_inbox_dir(&self) -> Result<(), String> {
        paths::ensure_dir(&self.inbox_dir()).map_err(|e| format!("create inbox: {e}"))
    }
}

fn env_inbox_dir() -> Option<PathBuf> {
    std::env::var("HANTRANSFER_INBOX_DIR")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .map(PathBuf::from)
}

fn env_auto_accept() -> bool {
    std::env::var("HANTRANSFER_AUTO_ACCEPT")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

fn settings_path() -> PathBuf {
    config::app_config_dir().join("settings.json")
}

fn read_settings_file() -> Option<SettingsFile> {
    let raw = std::fs::read_to_string(settings_path()).ok()?;
    serde_json::from_str(&raw).ok()
}

fn persist(settings: &DesktopSettings) -> Result<(), String> {
    let dir = config::app_config_dir();
    paths::ensure_dir(&dir).map_err(|e| e.to_string())?;
    let file = SettingsFile {
        inbox_dir: Some(settings.inbox_dir.clone()),
        auto_accept: Some(settings.auto_accept),
    };
    let json = serde_json::to_string_pretty(&file).map_err(|e| e.to_string())?;
    std::fs::write(settings_path(), json).map_err(|e| e.to_string())
}

fn validate_inbox_dir(path: &Path) -> Result<(), String> {
    let trimmed = path.to_string_lossy().trim().to_string();
    if trimmed.is_empty() {
        return Err("收件目录不能为空".into());
    }
    if !path.is_absolute() {
        return Err("请使用绝对路径".into());
    }
    Ok(())
}

fn normalize_inbox_dir(candidate: Option<PathBuf>, default_inbox: &Path) -> PathBuf {
    let path = match candidate.filter(|p| !p.as_os_str().is_empty()) {
        Some(path) if path.is_absolute() => path,
        _ => default_inbox.to_path_buf(),
    };
    clean_path(std::fs::canonicalize(&path).unwrap_or(path))
}

fn clean_path(path: PathBuf) -> PathBuf {
    let raw = path.display().to_string();
    let trimmed = raw.strip_prefix(r"\\?\").unwrap_or(&raw);
    PathBuf::from(trimmed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_requires_absolute() {
        assert!(validate_inbox_dir(Path::new("data/model")).is_err());
        assert!(validate_inbox_dir(Path::new("C:/tmp/inbox")).is_ok());
    }

    #[test]
    fn normalize_relative_to_default() {
        let default = PathBuf::from("D:/0HAN/HANDAILY/data/model");
        let got = normalize_inbox_dir(Some(PathBuf::from("data/model")), &default);
        assert!(got.ends_with("data/model") || got.ends_with("data\\model"));
    }
}
