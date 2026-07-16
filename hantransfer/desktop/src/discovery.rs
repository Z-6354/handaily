use std::collections::HashMap;

use mdns_sd::{ServiceDaemon, ServiceInfo};

use crate::config::Config;

const SERVICE_TYPE: &str = "_hantransfer._tcp.local.";

/// Keeps the mDNS daemon alive for the process lifetime.
pub struct MdnsRegistrar {
    _daemon: ServiceDaemon,
}

impl MdnsRegistrar {
    pub fn register(config: &Config) -> Result<Self, String> {
        let daemon = ServiceDaemon::new().map_err(|e| format!("mdns daemon: {e}"))?;

        let instance = format!(
            "{}-{}",
            sanitize_instance_name(&config.device_name),
            &config.device_id.to_string()[..8]
        );
        let host = default_hostname();

        let mut properties = HashMap::new();
        for (key, value) in config.mdns_txt() {
            properties.insert(key.to_string(), value);
        }

        let service_info = ServiceInfo::new(
            SERVICE_TYPE,
            &instance,
            &format!("{host}.local."),
            "",
            config.port,
            Some(properties),
        )
        .map_err(|e| format!("mdns service info: {e}"))?
        .enable_addr_auto();

        daemon
            .register(service_info)
            .map_err(|e| format!("mdns register: {e}"))?;

        tracing::info!(
            service = SERVICE_TYPE,
            instance = %instance,
            port = config.port,
            "mdns broadcasting"
        );

        Ok(Self { _daemon: daemon })
    }
}

fn sanitize_instance_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect();
    if cleaned.is_empty() {
        "HAN-PC".to_string()
    } else {
        cleaned
    }
}

fn default_hostname() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "han-pc".to_string())
        .to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_replaces_spaces() {
        assert_eq!(sanitize_instance_name("HAN PC"), "HAN-PC");
    }
}
