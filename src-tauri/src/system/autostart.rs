//! Windows 开机自启动（当前用户 Run 注册表项）

use std::path::PathBuf;

use tauri::AppHandle;

const SETTING_KEY: &str = "autostart_enabled";
const REG_RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const REG_VALUE_NAME: &str = "XiaohanDaily";

pub fn is_enabled(db: &rusqlite::Connection) -> bool {
    crate::db::get_setting(db, SETTING_KEY).as_deref() == Some("1")
}

pub fn set_enabled(app: &AppHandle, db: &rusqlite::Connection, enabled: bool) -> Result<(), String> {
    crate::db::set_setting(db, SETTING_KEY, if enabled { "1" } else { "0" })
        .map_err(|e| e.to_string())?;
    apply_registry(app, enabled)
}

/// 启动时把注册表与 DB 设置对齐（用户手动改注册表时以 DB 为准写回）
pub fn sync_on_startup(app: &AppHandle, db: &rusqlite::Connection) {
    let enabled = is_enabled(db);
    if let Err(e) = apply_registry(app, enabled) {
        eprintln!("xiaohan-daily: autostart sync failed: {e}");
    }
}

fn exe_path(_app: &AppHandle) -> Result<PathBuf, String> {
    std::env::current_exe().map_err(|e| e.to_string())
}

#[cfg(windows)]
fn apply_registry(app: &AppHandle, enabled: bool) -> Result<(), String> {
    use std::process::Command;

    if enabled {
        let exe = exe_path(app)?;
        let exe_str = exe.to_string_lossy().replace('/', "\\");
        let status = Command::new("reg")
            .args([
                "add",
                &format!(r"HKCU\{REG_RUN_KEY}"),
                "/v",
                REG_VALUE_NAME,
                "/t",
                "REG_SZ",
                "/d",
                &exe_str,
                "/f",
            ])
            .status()
            .map_err(|e| format!("无法写入注册表: {e}"))?;
        if !status.success() {
            return Err("写入开机自启动注册表失败".into());
        }
    } else {
        let _ = Command::new("reg")
            .args([
                "delete",
                &format!(r"HKCU\{REG_RUN_KEY}"),
                "/v",
                REG_VALUE_NAME,
                "/f",
            ])
            .status();
    }
    Ok(())
}

#[cfg(not(windows))]
fn apply_registry(_app: &AppHandle, enabled: bool) -> Result<(), String> {
    if enabled {
        Err("当前系统暂不支持开机自启动".into())
    } else {
        Ok(())
    }
}
