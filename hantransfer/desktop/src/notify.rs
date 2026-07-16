use std::path::Path;
use std::process::Command;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

use crate::importer::FileMetadata;

pub fn notify_file_received(meta: &FileMetadata, final_path: &Path) {
    tracing::info!(
        filename = %meta.filename,
        path = %final_path.display(),
        kind = ?meta.transfer_type,
        "file received"
    );
}

pub fn notify_trust_pending(device_name: &str, port: u16) {
    let title = "hantransfer · 新设备请求连接";
    let body = format!(
        "{device_name}\n请在浏览器打开 http://127.0.0.1:{port}/ 确认"
    );
    spawn_balloon(title, &body);
}

pub fn notify_push_queued(device_name: &str, filename: &str) {
    let title = "hantransfer · 已推送到手机";
    let body = format!("{filename}\n等待 {device_name} 在「接收」页下载");
    spawn_balloon(&title, &body);
}

pub fn notify_push_batch(device_name: &str, count: usize) {
    let title = "hantransfer · 已推送到手机";
    let body = if count == 1 {
        format!("1 个文件\n等待 {device_name} 在「接收」页下载")
    } else {
        format!("{count} 个文件\n等待 {device_name} 在「接收」页下载")
    };
    spawn_balloon(&title, &body);
}

fn spawn_balloon(title: &str, body: &str) {
    let title = shell_escape(title);
    let body = shell_escape(body);
    let script = format!(
        "Add-Type -AssemblyName System.Windows.Forms; \
         $n = New-Object System.Windows.Forms.NotifyIcon; \
         $n.Icon = [System.Drawing.SystemIcons]::Information; \
         $n.Visible = $true; \
         $n.ShowBalloonTip(8000, '{title}', '{body}', [System.Windows.Forms.ToolTipIcon]::Info); \
         Start-Sleep -Seconds 9; \
         $n.Dispose()"
    );
    std::thread::spawn(move || {
        let mut cmd = Command::new("powershell");
        cmd.args([
            "-NoProfile",
            "-NonInteractive",
            "-WindowStyle",
            "Hidden",
            "-Command",
            &script,
        ]);
        #[cfg(windows)]
        cmd.creation_flags(0x08000000);
        let _ = cmd.spawn();
    });
}

fn shell_escape(input: &str) -> String {
    input.replace('\'', "''")
}
