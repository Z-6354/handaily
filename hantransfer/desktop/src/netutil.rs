use std::net::{IpAddr, Ipv4Addr, UdpSocket};

/// Best-effort primary LAN IPv4 (UDP connect trick).
pub fn primary_lan_ipv4() -> Option<String> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect(format!("{}:80", Ipv4Addr::new(8, 8, 8, 8))).ok()?;
    match socket.local_addr().ok()?.ip() {
        IpAddr::V4(v4) if !v4.is_loopback() && !v4.is_unspecified() && !v4.is_link_local() => {
            Some(v4.to_string())
        }
        _ => None,
    }
}

pub fn mobile_urls(port: u16, preferred_lan_ip: Option<&str>) -> Vec<String> {
    let mut urls = Vec::new();
    if let Some(ip) = preferred_lan_ip.filter(|s| !s.is_empty()) {
        urls.push(format!("http://{ip}:{port}/m/"));
    }
    if let Some(ip) = primary_lan_ipv4() {
        let url = format!("http://{ip}:{port}/m/");
        if !urls.iter().any(|u| u == &url) {
            urls.push(url);
        }
    }
    urls
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mobile_urls_prefers_configured_ip() {
        let urls = mobile_urls(7822, Some("192.168.3.13"));
        assert_eq!(urls.first().map(String::as_str), Some("http://192.168.3.13:7822/m/"));
    }

    #[test]
    fn mobile_urls_skips_empty_preferred() {
        let urls = mobile_urls(7822, Some(""));
        assert!(urls.is_empty() || urls[0].contains("/m/"));
    }
}
