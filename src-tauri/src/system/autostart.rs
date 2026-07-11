//! Windows 开机自启动（当前用户 Run 注册表项）

use std::path::{Path, PathBuf};

use tauri::AppHandle;

const SETTING_KEY: &str = "autostart_enabled";
const REG_RUN_SUBKEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const REG_VALUE_NAME: &str = "XiaohanDaily";
pub const TRAY_LAUNCH_ARG: &str = "--tray";

pub fn is_tray_launch() -> bool {
    std::env::args().any(|a| a == TRAY_LAUNCH_ARG)
}

pub fn platform_supported() -> bool {
    std::env::consts::OS == "windows"
}

pub fn is_enabled(db: &rusqlite::Connection) -> bool {
    crate::db::get_setting(db, SETTING_KEY).as_deref() == Some("1")
}

pub fn set_enabled(app: &AppHandle, db: &rusqlite::Connection, enabled: bool) -> Result<(), String> {
    let prev = is_enabled(db);
    crate::db::set_setting(db, SETTING_KEY, if enabled { "1" } else { "0" })
        .map_err(|e| e.to_string())?;
    if let Err(e) = apply_registry(app, enabled) {
        let _ = crate::db::set_setting(db, SETTING_KEY, if prev { "1" } else { "0" });
        return Err(e);
    }
    Ok(())
}

/// 启动时把注册表与 DB 设置对齐（用户手动改注册表时以 DB 为准写回）
pub fn sync_on_startup(app: &AppHandle, db: &rusqlite::Connection) {
    let enabled = is_enabled(db);
    if let Err(e) = apply_registry(app, enabled) {
        crate::log::warn(format!("autostart sync failed: {e}"));
    }
}

fn exe_path(_app: &AppHandle) -> Result<PathBuf, String> {
    std::env::current_exe().map_err(|e| e.to_string())
}

fn registry_command_value(exe: &Path) -> String {
    let exe_str = exe.to_string_lossy().replace('/', "\\");
    format!("\"{exe_str}\" {TRAY_LAUNCH_ARG}")
}

#[cfg(windows)]
fn apply_registry(app: &AppHandle, enabled: bool) -> Result<(), String> {
    if enabled {
        let exe = exe_path(app)?;
        let reg_value = registry_command_value(&exe);
        win_reg::set_run_value(REG_VALUE_NAME, &reg_value)
    } else if win_reg::run_value_exists(REG_VALUE_NAME) {
        win_reg::delete_run_value(REG_VALUE_NAME)
    } else {
        Ok(())
    }
}

#[cfg(not(windows))]
fn apply_registry(_app: &AppHandle, enabled: bool) -> Result<(), String> {
    if enabled {
        Err("当前系统暂不支持开机自启动".into())
    } else {
        Ok(())
    }
}

#[cfg(windows)]
mod win_reg {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    use windows::core::PCWSTR;
    use windows::Win32::Foundation::ERROR_FILE_NOT_FOUND;
    use windows::Win32::System::Registry::{
        RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW,
        RegSetValueExW, HKEY, HKEY_CURRENT_USER, KEY_READ, KEY_SET_VALUE,
        REG_OPTION_NON_VOLATILE, REG_SAM_FLAGS, REG_SZ,
    };

    fn to_wide(s: &str) -> Vec<u16> {
        OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
    }

    fn map_reg_err(op: &str, err: windows::core::Error) -> String {
        format!("{op}失败（{err}）")
    }

    fn open_run_key(access: REG_SAM_FLAGS) -> Result<HKEY, String> {
        unsafe {
            let subkey = to_wide(super::REG_RUN_SUBKEY);
            let mut hkey = HKEY::default();
            RegOpenKeyExW(
                HKEY_CURRENT_USER,
                PCWSTR(subkey.as_ptr()),
                None,
                access,
                &mut hkey,
            )
            .ok()
            .map_err(|e| map_reg_err("打开开机自启动注册表", e))?;
            Ok(hkey)
        }
    }

    pub fn run_value_exists(name: &str) -> bool {
        read_run_value(name).is_some()
    }

    fn read_run_value(name: &str) -> Option<String> {
        unsafe {
            let hkey = open_run_key(KEY_READ).ok()?;
            let name_w = to_wide(name);
            let mut kind = REG_SZ;
            let mut size = 0u32;
            if RegQueryValueExW(
                hkey,
                PCWSTR(name_w.as_ptr()),
                None,
                Some(&mut kind),
                None,
                Some(&mut size),
            )
            .is_err()
                || size < 2
            {
                let _ = RegCloseKey(hkey);
                return None;
            }
            let mut buf = vec![0u16; (size as usize / 2).max(1)];
            if RegQueryValueExW(
                hkey,
                PCWSTR(name_w.as_ptr()),
                None,
                Some(&mut kind),
                Some(buf.as_mut_ptr() as *mut u8),
                Some(&mut size),
            )
            .is_err()
            {
                let _ = RegCloseKey(hkey);
                return None;
            }
            let _ = RegCloseKey(hkey);
            let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
            Some(String::from_utf16_lossy(&buf[..len]))
        }
    }

    pub fn set_run_value(name: &str, value: &str) -> Result<(), String> {
        unsafe {
            let subkey = to_wide(super::REG_RUN_SUBKEY);
            let mut hkey = HKEY::default();
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                PCWSTR(subkey.as_ptr()),
                None,
                PCWSTR::null(),
                REG_OPTION_NON_VOLATILE,
                KEY_SET_VALUE,
                None,
                &mut hkey,
                None,
            )
            .ok()
            .map_err(|e| map_reg_err("创建开机自启动注册表项", e))?;

            let name_w = to_wide(name);
            let value_w = to_wide(value);
            let bytes = std::slice::from_raw_parts(
                value_w.as_ptr() as *const u8,
                value_w.len() * 2,
            );
            RegSetValueExW(
                hkey,
                PCWSTR(name_w.as_ptr()),
                None,
                REG_SZ,
                Some(bytes),
            )
            .ok()
            .map_err(|e| map_reg_err("写入开机自启动注册表", e))?;
            let _ = RegCloseKey(hkey);
        }
        Ok(())
    }

    pub fn delete_run_value(name: &str) -> Result<(), String> {
        unsafe {
            let hkey = open_run_key(KEY_SET_VALUE)?;
            let name_w = to_wide(name);
            let rc = RegDeleteValueW(hkey, PCWSTR(name_w.as_ptr()));
            let _ = RegCloseKey(hkey);
            if rc.is_ok() || rc == ERROR_FILE_NOT_FOUND {
                return Ok(());
            }
            Err(map_reg_err(
                "删除开机自启动注册表",
                rc.into(),
            ))
        }
    }
}
