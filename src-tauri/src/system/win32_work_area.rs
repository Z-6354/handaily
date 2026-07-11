//! Windows：主窗口「最大化」= 铺满工作区（不含任务栏），拦截系统全屏最大化。

use std::sync::atomic::{AtomicBool, Ordering};

use tauri::{PhysicalPosition, PhysicalSize, Position, Size, WebviewWindow};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST};
use windows::Win32::UI::Shell::{DefSubclassProc, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::{
    GetAncestor, IsZoomed, SetWindowPos, ShowWindow, GA_ROOT, HTCAPTION, MINMAXINFO, SC_MAXIMIZE,
    SIZE_MAXIMIZED, SWP_FRAMECHANGED, SWP_NOZORDER, SWP_SHOWWINDOW, SW_RESTORE, WM_GETMINMAXINFO,
    WM_NCLBUTTONDBLCLK, WM_SIZE, WM_SYSCOMMAND,
};

const SUBCLASS_ID: usize = 0x5A48_414E;
static APPLYING_WORK_AREA: AtomicBool = AtomicBool::new(false);
static HOOK_INSTALLED: AtomicBool = AtomicBool::new(false);

/// 安装窗口子类（挂在顶层 frame HWND 上）。
pub fn install_maximize_work_area_hook(win: &WebviewWindow) -> Result<(), String> {
    if HOOK_INSTALLED.load(Ordering::Relaxed) {
        return Ok(());
    }
    let hwnd = frame_hwnd(win.hwnd().map_err(|e| e.to_string())?);
    unsafe {
        if !SetWindowSubclass(hwnd, Some(subclass_proc), SUBCLASS_ID, 0).as_bool() {
            return Err("SetWindowSubclass 失败".into());
        }
    }
    HOOK_INSTALLED.store(true, Ordering::Relaxed);
    Ok(())
}

/// 退出前移除子类，避免 HWND 销毁后仍收到窗口消息。
pub fn uninstall_maximize_work_area_hook(win: &WebviewWindow) {
    if !HOOK_INSTALLED.load(Ordering::Relaxed) {
        return;
    }
    let Ok(hwnd) = win.hwnd() else {
        HOOK_INSTALLED.store(false, Ordering::Relaxed);
        return;
    };
    let frame = frame_hwnd(hwnd);
    unsafe {
        use windows::Win32::UI::Shell::RemoveWindowSubclass;
        let _ = RemoveWindowSubclass(frame, Some(subclass_proc), SUBCLASS_ID);
    }
    HOOK_INSTALLED.store(false, Ordering::Relaxed);
}

fn frame_hwnd(hwnd: HWND) -> HWND {
    unsafe {
        let root = GetAncestor(hwnd, GA_ROOT);
        if !root.0.is_null() {
            root
        } else {
            hwnd
        }
    }
}

/// 当前窗口所在显示器的可用工作区（物理像素，已排除任务栏）。
pub fn work_area_for_hwnd(hwnd: HWND) -> Option<RECT> {
    unsafe {
        let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        if monitor.0.is_null() {
            return None;
        }
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if !GetMonitorInfoW(monitor, &mut info).as_bool() {
            return None;
        }
        Some(info.rcWork)
    }
}

fn apply_work_area_size(hwnd: HWND) -> bool {
    if APPLYING_WORK_AREA.swap(true, Ordering::SeqCst) {
        return false;
    }
    let ok = work_area_for_hwnd(hwnd)
        .map(|work| {
            let width = (work.right - work.left).max(1);
            let height = (work.bottom - work.top).max(1);
            unsafe {
                // 退出 OS maximize 状态，再按工作区定位（避免盖住任务栏）
                let _ = ShowWindow(hwnd, SW_RESTORE);
                SetWindowPos(
                    hwnd,
                    None,
                    work.left,
                    work.top,
                    width,
                    height,
                    SWP_NOZORDER | SWP_SHOWWINDOW | SWP_FRAMECHANGED,
                )
                .is_ok()
            }
        })
        .unwrap_or(false);
    APPLYING_WORK_AREA.store(false, Ordering::SeqCst);
    ok
}

/// 将窗口尺寸/位置设为工作区（非 OS maximize 状态）。
pub fn fit_window_to_work_area(win: &WebviewWindow) -> bool {
    let hwnd = match win.hwnd() {
        Ok(h) => h,
        Err(_) => return false,
    };
    let frame = frame_hwnd(hwnd);
    if !apply_work_area_size(frame) {
        return false;
    }
    // 与 Tauri 内部状态对齐
    let _ = win.unmaximize();
    if let Some(work) = work_area_for_hwnd(frame) {
        let _ = win.set_position(Position::Physical(PhysicalPosition::new(work.left, work.top)));
        let _ = win.set_size(Size::Physical(PhysicalSize::new(
            (work.right - work.left).max(1) as u32,
            (work.bottom - work.top).max(1) as u32,
        )));
    }
    true
}

/// 开局若被系统误最大化，恢复为配置默认尺寸（不铺满工作区）。
pub fn restore_default_if_zoomed(win: &WebviewWindow) {
    let Ok(hwnd) = win.hwnd() else {
        return;
    };
    let frame = frame_hwnd(hwnd);
    let zoomed = unsafe { IsZoomed(frame).as_bool() || win.is_maximized().unwrap_or(false) };
    if !zoomed {
        return;
    }
    let _ = win.unmaximize();
    unsafe {
        let _ = ShowWindow(frame, SW_RESTORE);
    }
    let _ = win.set_size(Size::Logical(tauri::LogicalSize::new(960.0, 640.0)));
    let _ = win.center();
}

/// 最大化按钮/双击标题栏后：若已进入 zoomed 状态则拉回工作区。
pub fn correct_if_zoomed(win: &WebviewWindow) {
    let Ok(hwnd) = win.hwnd() else {
        return;
    };
    let frame = frame_hwnd(hwnd);
    let zoomed = unsafe { IsZoomed(frame).as_bool() || win.is_maximized().unwrap_or(false) };
    if zoomed {
        let _ = fit_window_to_work_area(win);
    }
}

unsafe extern "system" fn subclass_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _uid: usize,
    _data: usize,
) -> LRESULT {
    match msg {
        WM_SYSCOMMAND => {
            let cmd = wparam.0 as u32 & 0xFFF0;
            if cmd == SC_MAXIMIZE {
                apply_work_area_size(hwnd);
                return LRESULT(0);
            }
        }
        WM_NCLBUTTONDBLCLK => {
            if wparam.0 as u32 == HTCAPTION {
                apply_work_area_size(hwnd);
                return LRESULT(0);
            }
        }
        WM_SIZE => {
            if wparam.0 == SIZE_MAXIMIZED as usize {
                apply_work_area_size(hwnd);
            }
        }
        WM_GETMINMAXINFO => {
            if let Some(work) = work_area_for_hwnd(hwnd) {
                let info = &mut *(lparam.0 as *mut MINMAXINFO);
                info.ptMaxPosition.x = work.left;
                info.ptMaxPosition.y = work.top;
                info.ptMaxSize.x = work.right - work.left;
                info.ptMaxSize.y = work.bottom - work.top;
            }
        }
        _ => {}
    }
    DefSubclassProc(hwnd, msg, wparam, lparam)
}
