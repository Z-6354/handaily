//! 全局键鼠监控（低级别 Hook）

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::{self, JoinHandle};

use crate::state::AppState;
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetKeyboardState, ToUnicode};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetMessageW, SetWindowsHookExW, UnhookWindowsHookEx,
    HC_ACTION, KBDLLHOOKSTRUCT, MSG, WH_KEYBOARD_LL, WH_MOUSE_LL, WM_KEYDOWN,
    WM_LBUTTONDOWN, WM_MBUTTONDOWN, WM_RBUTTONDOWN, WM_SYSKEYDOWN,
};

const MAX_TEXT_BUF: usize = 4096;

/// 内存中的输入计数（定期 flush 到 DB）
pub struct InputStatsShared {
    pub mouse_clicks: AtomicU64,
    pub key_strokes: AtomicU64,
    pub keyboard_text: Mutex<String>,
    pub files_created: AtomicU64,
    pub files_modified: AtomicU64,
    mouse_pending: AtomicU64,
    key_pending: AtomicU64,
    text_pending: Mutex<String>,
}

impl InputStatsShared {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            mouse_clicks: AtomicU64::new(0),
            key_strokes: AtomicU64::new(0),
            keyboard_text: Mutex::new(String::new()),
            files_created: AtomicU64::new(0),
            files_modified: AtomicU64::new(0),
            mouse_pending: AtomicU64::new(0),
            key_pending: AtomicU64::new(0),
            text_pending: Mutex::new(String::new()),
        })
    }

    /// 取出待写入 DB 的增量并清零 pending
    pub fn take_flush_delta(&self) -> (u64, u64, String, u64, u64) {
        let mouse = self.mouse_pending.swap(0, Ordering::Relaxed);
        let keys = self.key_pending.swap(0, Ordering::Relaxed);
        let text = self.text_pending.lock().unwrap().drain(..).collect();
        let created = self.files_created.swap(0, Ordering::Relaxed);
        let modified = self.files_modified.swap(0, Ordering::Relaxed);
        (mouse, keys, text, created, modified)
    }

    /// 当前累计（DB + 内存，供 IPC 展示）
    pub fn live_totals(&self, db_base: &crate::db::metrics::DailyMetrics) -> crate::db::metrics::DailyMetrics {
        let mouse = db_base.mouse_clicks
            + self.mouse_clicks.load(Ordering::Relaxed)
            + self.mouse_pending.load(Ordering::Relaxed);
        let keys = db_base.key_strokes
            + self.key_strokes.load(Ordering::Relaxed)
            + self.key_pending.load(Ordering::Relaxed);
        let mut text = db_base.keyboard_text.clone();
        text.push_str(&self.keyboard_text.lock().unwrap());
        text.push_str(&self.text_pending.lock().unwrap());
        if text.len() > 500 {
            text = text.chars().rev().take(500).collect::<String>().chars().rev().collect();
        }
        crate::db::metrics::DailyMetrics {
            date: db_base.date.clone(),
            mouse_clicks: mouse,
            key_strokes: keys,
            keyboard_text: text,
            files_created: db_base.files_created + self.files_created.load(Ordering::Relaxed),
            files_modified: db_base.files_modified + self.files_modified.load(Ordering::Relaxed),
        }
    }
}

static HOOK_STATS: OnceLock<Arc<InputStatsShared>> = OnceLock::new();
static HOOK_ENABLED: AtomicBool = AtomicBool::new(true);

pub fn set_input_enabled(enabled: bool) {
    HOOK_ENABLED.store(enabled, Ordering::Relaxed);
}

pub fn spawn_input_monitor(state: Arc<AppState>) -> JoinHandle<()> {
    let _ = HOOK_STATS.set(state.input_stats.clone());
    thread::spawn(move || {
        unsafe {
            let mouse_hook = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), None, 0);
            let key_hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_proc), None, 0);
            if mouse_hook.is_err() || key_hook.is_err() {
                eprintln!(
                    "xiaohan-daily: input hooks failed, key/mouse metrics disabled (mouse={}, keyboard={})",
                    mouse_hook.is_err(),
                    key_hook.is_err()
                );
                return;
            }
            let mouse_hook = mouse_hook.unwrap();
            let key_hook = key_hook.unwrap();

            let mut msg = MSG::default();
            while !state.stop_flag.load(Ordering::Relaxed) {
                let ret = GetMessageW(&mut msg, None, 0, 0);
                if ret.0 == 0 || ret.0 == -1 {
                    break;
                }
                let _ = DispatchMessageW(&msg);
            }

            let _ = UnhookWindowsHookEx(mouse_hook);
            let _ = UnhookWindowsHookEx(key_hook);
        }
    })
}

unsafe extern "system" fn mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code as u32 == HC_ACTION && HOOK_ENABLED.load(Ordering::Relaxed) {
        let wp = wparam.0 as u32;
        if wp == WM_LBUTTONDOWN || wp == WM_RBUTTONDOWN || wp == WM_MBUTTONDOWN {
            if let Some(stats) = HOOK_STATS.get() {
                stats.mouse_clicks.fetch_add(1, Ordering::Relaxed);
                stats.mouse_pending.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
    CallNextHookEx(None, code, wparam, lparam)
}

unsafe extern "system" fn keyboard_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code as u32 == HC_ACTION && HOOK_ENABLED.load(Ordering::Relaxed) {
        let wp = wparam.0 as u32;
        if wp == WM_KEYDOWN || wp == WM_SYSKEYDOWN {
            if let Some(stats) = HOOK_STATS.get() {
                stats.key_strokes.fetch_add(1, Ordering::Relaxed);
                stats.key_pending.fetch_add(1, Ordering::Relaxed);

                let kb = *(lparam.0 as *const KBDLLHOOKSTRUCT);
                let mut state = [0u8; 256];
                if GetKeyboardState(&mut state).is_ok() {
                    let mut buf = [0u16; 8];
                    let n = ToUnicode(kb.vkCode, kb.scanCode, Some(&state), &mut buf, 0);
                    if n == 1 {
                        if let Some(ch) = char::from_u32(buf[0] as u32) {
                            if !ch.is_control() {
                                let mut pending = stats.text_pending.lock().unwrap();
                                if pending.len() < MAX_TEXT_BUF {
                                    pending.push(ch);
                                }
                                let mut main = stats.keyboard_text.lock().unwrap();
                                if main.len() < MAX_TEXT_BUF {
                                    main.push(ch);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    CallNextHookEx(None, code, wparam, lparam)
}
