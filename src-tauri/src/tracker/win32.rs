//! Win32 前台窗口采集
//!
//! 通过 `windows` crate（编译期链接，非运行时 FFI）调用：
//! - `GetForegroundWindow` — 取前台 HWND
//! - `GetWindowTextW` — 取窗口标题
//! - `GetWindowThreadProcessId` — 取 PID
//! - `OpenProcess` + `QueryFullProcessImageNameW` — 取 exe 路径
//!
//! feature 路径以前置验证实测为准（计划 §9 已标注）。

use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::path::Path;

use std::sync::LazyLock;

use chrono::Local;
use windows::core::PWSTR;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST};
use windows::Win32::System::ProcessStatus::GetProcessImageFileNameW;
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetClassNameW, GetForegroundWindow, GetWindowRect, GetWindowTextLengthW, GetWindowTextW,
    GetWindowThreadProcessId,
};

use super::Snapshot;

static SELF_PID: LazyLock<u32> = LazyLock::new(std::process::id);

/// 取进程 exe 完整路径（供音频检测等模块使用）
pub fn get_process_exe_path(pid: u32) -> Option<String> {
    get_exe_path(pid)
}

/// 取前台窗口快照；无前台窗口、本应用自身或 System Idle 时返回 None
pub fn get_foreground_snapshot() -> Option<Snapshot> {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0.is_null() {
            return None;
        }

        let pid = get_pid(hwnd)?;
        if pid == *SELF_PID {
            return None;
        }

        let window_title = get_window_text(hwnd);
        let exe_path = get_exe_path(pid).unwrap_or_default();
        let mut app_name = exe_stem(&exe_path);
        if app_name.is_empty() {
            if let Some(from_title) = super::title_parse::app_name_from_title(&window_title) {
                app_name = from_title;
            }
        }

        if is_desktop(hwnd, &app_name, &window_title, &exe_path) {
            return Some(desktop_snapshot(pid));
        }

        if is_system_idle(&app_name) {
            return None;
        }

        Some(Snapshot {
            pid,
            exe_path,
            app_name,
            window_title,
            captured_at: Local::now(),
            is_idle: false, // idle 由 idle.rs 判断，这里固定 false
        })
    }
}

/// 取窗口 PID
fn get_pid(hwnd: HWND) -> Option<u32> {
    unsafe {
        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == 0 {
            None
        } else {
            Some(pid)
        }
    }
}

/// 取窗口标题
fn get_window_text(hwnd: HWND) -> String {
    unsafe {
        let len = GetWindowTextLengthW(hwnd);
        if len <= 0 {
            return String::new();
        }
        let mut buf = vec![0u16; (len as usize) + 1];
        let copied = GetWindowTextW(hwnd, &mut buf);
        if copied <= 0 {
            return String::new();
        }
        let len = copied as usize;
        let os = OsString::from_wide(&buf[..len]);
        os.to_string_lossy().into_owned()
    }
}

/// 取进程 exe 完整路径（Win32 路径优先，失败时回退设备路径）
fn get_exe_path(pid: u32) -> Option<String> {
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        if QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            PWSTR(buf.as_mut_ptr()),
            &mut len,
        )
        .is_ok()
        {
            let path = wide_to_string(&buf[..len as usize]);
            let _ = windows::Win32::Foundation::CloseHandle(handle);
            if !path.is_empty() {
                return Some(path);
            }
        }
        let mut dev_buf = [0u16; 1024];
        let copied = GetProcessImageFileNameW(handle, &mut dev_buf);
        let _ = windows::Win32::Foundation::CloseHandle(handle);
        if copied > 0 {
            let path = wide_to_string(&dev_buf[..copied as usize]);
            if !path.is_empty() {
                return Some(path);
            }
        }
        None
    }
}

fn wide_to_string(wide: &[u16]) -> String {
    let os = OsString::from_wide(wide);
    os.to_string_lossy().into_owned()
}

/// 从 exe 路径取文件名（stem，无扩展），作为 app_name
fn exe_stem(exe_path: &str) -> String {
    if exe_path.is_empty() {
        return String::new();
    }
    Path::new(exe_path)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn is_system_idle(app_name: &str) -> bool {
    let name = app_name.to_lowercase();
    name.contains("idle")
}

/// 桌面（Explorer 壳层 / Program Manager）
fn is_desktop(hwnd: HWND, app_name: &str, window_title: &str, exe_path: &str) -> bool {
    let title = window_title.to_lowercase();
    let name = app_name.to_lowercase();
    if title == "program manager" || title == "desktop" {
        return true;
    }
    if name == "explorer" && window_title.is_empty() {
        return true;
    }
    if exe_path.to_lowercase().contains("explorer.exe") && window_title.is_empty() {
        return is_shell_class(hwnd);
    }
    false
}

fn is_shell_class(hwnd: HWND) -> bool {
    unsafe {
        let mut buf = [0u16; 256];
        let n = GetClassNameW(hwnd, &mut buf);
        if n <= 0 {
            return false;
        }
        let class = OsString::from_wide(&buf[..n as usize])
            .to_string_lossy()
            .to_lowercase();
        class == "progman" || class == "workerw" || class == "shell_traywnd"
    }
}

fn desktop_snapshot(pid: u32) -> Snapshot {
    Snapshot {
        pid,
        exe_path: r"C:\Windows\explorer.exe".into(),
        app_name: "desktop".into(),
        window_title: "桌面".into(),
        captured_at: Local::now(),
        is_idle: false,
    }
}

/// 前台窗口是否为沉浸式全屏（游戏 / 视频 F11 等），不含普通最大化窗口。
pub fn is_foreground_fullscreen() -> bool {
    unsafe {
        use windows::Win32::Foundation::RECT;
        use windows::Win32::UI::WindowsAndMessaging::{
            GetWindowLongPtrW, GetWindowPlacement, GWL_STYLE, IsZoomed, WINDOWPLACEMENT, WS_CAPTION,
        };

        let hwnd = GetForegroundWindow();
        if hwnd.0.is_null() {
            return false;
        }

        // 最大化窗口（含无边框最大化）不算沉浸式全屏
        if IsZoomed(hwnd).as_bool() {
            return false;
        }

        let mut placement = WINDOWPLACEMENT {
            length: std::mem::size_of::<WINDOWPLACEMENT>() as u32,
            ..Default::default()
        };
        if GetWindowPlacement(hwnd, &mut placement).is_ok() && placement.showCmd == 3 {
            // SW_SHOWMAXIMIZED：Cursor / 浏览器等最大化不算全屏
            return false;
        }

        let style = GetWindowLongPtrW(hwnd, GWL_STYLE) as u32;
        if style & WS_CAPTION.0 != 0 {
            return false;
        }

        let mut window_rect = RECT::default();
        if GetWindowRect(hwnd, &mut window_rect).is_err() {
            return false;
        }
        let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if GetMonitorInfoW(monitor, &mut info).0 == 0 {
            return false;
        }
        let m = info.rcMonitor;
        let tol = 4;
        (window_rect.left - m.left).abs() <= tol
            && (window_rect.top - m.top).abs() <= tol
            && (window_rect.right - m.right).abs() <= tol
            && (window_rect.bottom - m.bottom).abs() <= tol
    }
}

/// 前台窗口是否属于本进程（小寒日报 main/pet 窗口前台时不应触发全屏隐藏）
pub fn is_foreground_own_process() -> bool {
    unsafe {
        use windows::Win32::System::Threading::GetCurrentProcessId;
        use windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId;

        let hwnd = GetForegroundWindow();
        if hwnd.0.is_null() {
            return false;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        pid != 0 && pid == GetCurrentProcessId()
    }
}
