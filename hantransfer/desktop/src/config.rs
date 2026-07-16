use std::path::PathBuf;

use uuid::Uuid;

use crate::paths;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const PLATFORM: &str = "windows";
pub const DEFAULT_PORT: u16 = 7822;

#[derive(Debug, Clone)]
pub struct Config {
    pub device_name: String,
    pub device_id: Uuid,
    pub port: u16,
    pub lan_ipv4: Option<String>,
    pub inbox_dir: PathBuf,
    pub history_dir: PathBuf,
    pub temp_dir: PathBuf,
    pub outbox_dir: PathBuf,
}

impl Config {
    pub fn load() -> Result<Self, String> {
        let device_id = load_or_create_device_id()?;
        let port = std::env::var("HANTRANSFER_PORT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT_PORT);
        let device_name = std::env::var("HANTRANSFER_DEVICE_NAME")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(default_hostname);

        Ok(Self {
            device_name,
            device_id,
            port,
            lan_ipv4: env_lan_ipv4().or_else(crate::netutil::primary_lan_ipv4),
            inbox_dir: env_path("HANTRANSFER_INBOX_DIR").unwrap_or_else(paths::model_inbox),
            history_dir: env_path("HANTRANSFER_HISTORY_DIR").unwrap_or_else(paths::transfer_history),
            temp_dir: env_path("HANTRANSFER_TEMP_DIR").unwrap_or_else(paths::transfer_temp),
            outbox_dir: env_path("HANTRANSFER_OUTBOX_DIR").unwrap_or_else(paths::transfer_outbox),
        })
    }

    pub fn features(&self) -> Vec<&'static str> {
        vec!["send", "receive", "azurlane-import"]
    }

    pub fn listen_addr(&self) -> String {
        format!("0.0.0.0:{}", self.port)
    }

    pub fn mdns_txt(&self) -> Vec<(&'static str, String)> {
        let mut entries = vec![
            ("name", self.device_name.clone()),
            ("platform", PLATFORM.to_string()),
            ("version", VERSION.to_string()),
            ("device_id", self.device_id.to_string()),
            (
                "features",
                self.features().join(","),
            ),
        ];
        if let Some(ip) = &self.lan_ipv4 {
            entries.push(("lan_ip", ip.clone()));
        }
        entries
    }

    pub fn ensure_data_dirs(&self) -> Result<(), String> {
        for dir in [
            &self.inbox_dir,
            &self.history_dir,
            &self.temp_dir,
            &self.outbox_dir,
        ] {
            paths::ensure_dir(dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
        }
        Ok(())
    }
}

fn default_hostname() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "HAN-PC".to_string())
}

fn env_lan_ipv4() -> Option<String> {
    std::env::var("HANTRANSFER_LAN_IP")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn env_path(name: &str) -> Option<PathBuf> {
    std::env::var(name)
        .ok()
        .filter(|s| !s.trim().is_empty())
        .map(PathBuf::from)
}

fn load_or_create_device_id() -> Result<Uuid, String> {
    let config_dir = app_config_dir();
    let id_path = config_dir.join("device_id");
    if id_path.is_file() {
        let raw = std::fs::read_to_string(&id_path).map_err(|e| e.to_string())?;
        return Uuid::parse_str(raw.trim()).map_err(|e| e.to_string());
    }
    paths::ensure_dir(&config_dir).map_err(|e| e.to_string())?;
    let id = Uuid::new_v4();
    std::fs::write(&id_path, id.to_string()).map_err(|e| e.to_string())?;
    Ok(id)
}

pub fn app_config_dir() -> PathBuf {
    if let Ok(appdata) = std::env::var("APPDATA") {
        return PathBuf::from(appdata).join("hantransfer");
    }
    paths::project_root().join(".local/hantransfer")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mdns_txt_has_required_keys() {
        let cfg = Config {
            device_name: "HAN-PC".into(),
            device_id: Uuid::new_v4(),
            port: 7822,
            lan_ipv4: Some("192.168.1.10".into()),
            inbox_dir: paths::model_inbox(),
            history_dir: paths::transfer_history(),
            temp_dir: paths::transfer_temp(),
            outbox_dir: paths::transfer_outbox(),
        };
        let keys: Vec<_> = cfg.mdns_txt().into_iter().map(|(k, _)| k).collect();
        assert!(keys.contains(&"name"));
        assert!(keys.contains(&"features"));
        assert!(keys.contains(&"lan_ip"));
    }
}
