use std::process::Command;

const RULE_PREFIX: &str = "hantransfer TCP";

pub fn ensure_inbound_rule(port: u16) {
    let rule_name = format!("{RULE_PREFIX} {port}");
    if rule_exists(&rule_name) {
        tracing::debug!(rule = %rule_name, "firewall rule already present");
        return;
    }

    tracing::warn!(
        port,
        "no firewall rule for hantransfer — phones on LAN may be blocked"
    );
    tracing::warn!(
        "run as Administrator: powershell -File scripts/allow-hantransfer-firewall.ps1"
    );

    let output = Command::new("netsh")
        .args([
            "advfirewall",
            "firewall",
            "add",
            "rule",
            &format!("name={rule_name}"),
            "dir=in",
            "action=allow",
            "protocol=TCP",
            &format!("localport={port}"),
            "profile=any",
        ])
        .output();

    match output {
        Ok(out) if out.status.success() => {
            tracing::info!(port, "firewall inbound rule added");
        }
        Ok(out) => {
            let err = String::from_utf8_lossy(&out.stderr);
            if err.contains("拒绝访问") || err.contains("Access is denied") || err.contains("requires elevation") {
                tracing::warn!(
                    port,
                    "could not add firewall rule (need Administrator). \
                     Phone may not connect until you run scripts/allow-hantransfer-firewall.ps1"
                );
            } else {
                tracing::warn!(port, stderr = %err.trim(), "firewall rule add failed");
            }
        }
        Err(err) => tracing::warn!(port, error = %err, "failed to invoke netsh"),
    }
}

fn rule_exists(name: &str) -> bool {
    Command::new("netsh")
        .args([
            "advfirewall",
            "firewall",
            "show",
            "rule",
            &format!("name={name}"),
        ])
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rule_name_format() {
        assert!(format!("{RULE_PREFIX} 7822").starts_with("hantransfer"));
    }
}
