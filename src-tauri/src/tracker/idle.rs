//! 空闲检测
//!
//! `GetLastInputInfo` 取距上次键鼠输入的毫秒数，超过阈值则 idle。
//! 默认阈值 90s（修正点①：原 300s 偏高，90s 是同类产品常用值）。

use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};

/// 默认空闲阈值（秒）
pub const DEFAULT_IDLE_THRESHOLD_SECS: u64 = 90;

/// 取距上次输入的毫秒数；失败返回 0（保守认为刚有输入）
pub fn idle_ms() -> u64 {
    unsafe {
        let mut info = LASTINPUTINFO::default();
        info.cbSize = std::mem::size_of::<LASTINPUTINFO>() as u32;
        if GetLastInputInfo(&mut info).as_bool() {
            let now = windows::Win32::System::SystemInformation::GetTickCount64();
            // GetTickCount64 是 ms，info.dwTime 也是 ms（32位）
            let last = info.dwTime as u64;
            now.saturating_sub(last)
        } else {
            0
        }
    }
}

/// 判断当前是否空闲
pub fn is_idle(threshold_secs: u64) -> bool {
    idle_ms() > threshold_secs * 1000
}
