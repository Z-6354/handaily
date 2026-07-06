//! 全局键鼠监控：Raw Input 消息窗（无低级别 Hook，不拖慢光标/键盘）

use std::mem::size_of;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::state::AppState;
use crate::tracker::dampen_thread_priority;
use windows::core::w;
use windows::Win32::Foundation::{GetLastError, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::{
    GetRawInputData, RegisterRawInputDevices, HRAWINPUT, RAWINPUT, RAWINPUTDEVICE, RAWINPUTHEADER,
    RAWKEYBOARD, RIDEV_INPUTSINK, RIDEV_REMOVE, RID_INPUT, RIM_TYPEKEYBOARD,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, GetKeyboardState, ToUnicode, VK_LBUTTON, VK_MBUTTON, VK_RBUTTON,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, MsgWaitForMultipleObjects,
    PeekMessageW, PostQuitMessage, RegisterClassW, TranslateMessage, WNDCLASSW, HWND_MESSAGE,
    PM_REMOVE, QS_ALLINPUT, RI_KEY_BREAK, WM_DESTROY, WM_INPUT, WM_QUIT, WINDOW_EX_STYLE,
    WINDOW_STYLE,
};

const MAX_TEXT_BUF: usize = 4096;
const WAIT_MS: u32 = 16;
const RAW_BUF_BYTES: usize = 512;
/// Win32 `RAWINPUT` 需 8 字节对齐；`[u8; N]` 仅 1 字节对齐，强转引用会触发 misaligned pointer panic。
#[repr(C, align(8))]
struct RawInputBuf([u8; RAW_BUF_BYTES]);
const CLASS_NAME: windows::core::PCWSTR = w!("XiaohanDailyInputSink");
const ERROR_CLASS_ALREADY_EXISTS: u32 = 1410;

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
    pub fn live_totals(
        &self,
        db_base: &crate::db::metrics::DailyMetrics,
    ) -> crate::db::metrics::DailyMetrics {
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
            text = text
                .chars()
                .rev()
                .take(500)
                .collect::<String>()
                .chars()
                .rev()
                .collect();
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

static INPUT_STATS: OnceLock<Arc<InputStatsShared>> = OnceLock::new();
static INPUT_ENABLED: AtomicBool = AtomicBool::new(true);
static MOUSE_BTN_PREV: [AtomicBool; 3] =
    [AtomicBool::new(false), AtomicBool::new(false), AtomicBool::new(false)];

pub fn set_input_enabled(enabled: bool) {
    INPUT_ENABLED.store(enabled, Ordering::Relaxed);
}

pub fn spawn_input_monitor(state: Arc<AppState>) -> JoinHandle<()> {
    let _ = INPUT_STATS.set(state.input_stats.clone());
    INPUT_ENABLED.store(
        state
            .tracking_enabled
            .load(Ordering::Relaxed),
        Ordering::Relaxed,
    );
    thread::spawn(move || {
        dampen_thread_priority();
        unsafe {
            let hwnd = match create_message_window() {
                Ok(h) => h,
                Err(e) => {
                    crate::log::warn(format!("raw input window failed: {e}"));
                    return;
                }
            };
            if let Err(e) = register_raw_input(hwnd) {
                crate::log::warn(format!("RegisterRawInputDevices failed: {e}"));
                let _ = DestroyWindow(hwnd);
                return;
            }

            let mut msg = windows::Win32::UI::WindowsAndMessaging::MSG::default();
            while !state.stop_flag.load(Ordering::Relaxed) {
                if !INPUT_ENABLED.load(Ordering::Relaxed) {
                    thread::sleep(Duration::from_millis(500));
                    continue;
                }
                poll_mouse_clicks();
                let _ = MsgWaitForMultipleObjects(None, false, WAIT_MS, QS_ALLINPUT);
                while PeekMessageW(&mut msg, Some(hwnd), 0, 0, PM_REMOVE).as_bool() {
                    if msg.message == WM_QUIT {
                        state.stop_flag.store(true, Ordering::Relaxed);
                        break;
                    }
                    let _ = TranslateMessage(&msg);
                    let _ = DispatchMessageW(&msg);
                }
            }

            let _ = unregister_raw_input();
            let _ = DestroyWindow(hwnd);
        }
    })
}

unsafe fn create_message_window() -> Result<HWND, String> {
    let hinstance = GetModuleHandleW(None).map_err(|e| e.to_string())?;
    let class = WNDCLASSW {
        lpfnWndProc: Some(wnd_proc),
        hInstance: hinstance.into(),
        lpszClassName: CLASS_NAME,
        ..Default::default()
    };
    if RegisterClassW(&class) == 0 {
        let err = GetLastError().0;
        if err != ERROR_CLASS_ALREADY_EXISTS {
            return Err(format!("RegisterClassW failed: {err}"));
        }
    }
    CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        CLASS_NAME,
        w!(""),
        WINDOW_STYLE::default(),
        0,
        0,
        0,
        0,
        Some(HWND_MESSAGE),
        None,
        Some(hinstance.into()),
        None,
    )
    .map_err(|e| e.to_string())
}

unsafe fn register_raw_input(hwnd: HWND) -> windows::core::Result<()> {
    // 仅注册键盘：鼠标移动若走 Raw Input 会在高 DPI 下产生大量 WM_INPUT
    let devices = [RAWINPUTDEVICE {
        usUsagePage: 0x01,
        usUsage: 0x06,
        dwFlags: RIDEV_INPUTSINK,
        hwndTarget: hwnd,
    }];
    RegisterRawInputDevices(&devices, size_of::<RAWINPUTDEVICE>() as u32)
}

unsafe fn unregister_raw_input() -> windows::core::Result<()> {
    let devices = [RAWINPUTDEVICE {
        usUsagePage: 0x01,
        usUsage: 0x06,
        dwFlags: RIDEV_REMOVE,
        hwndTarget: HWND::default(),
    }];
    RegisterRawInputDevices(&devices, size_of::<RAWINPUTDEVICE>() as u32)
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_INPUT => {
            handle_wm_input(lparam);
            LRESULT(0)
        }
        WM_DESTROY => {
            let _ = PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

unsafe fn handle_wm_input(lparam: LPARAM) {
    if !INPUT_ENABLED.load(Ordering::Relaxed) {
        return;
    }
    let mut buf = RawInputBuf([0u8; RAW_BUF_BYTES]);
    let mut size = RAW_BUF_BYTES as u32;
    let read = GetRawInputData(
        HRAWINPUT(lparam.0 as *mut _),
        RID_INPUT,
        Some(buf.0.as_mut_ptr().cast()),
        &mut size,
        size_of::<RAWINPUTHEADER>() as u32,
    );
    if read == u32::MAX || size == 0 {
        return;
    }
    if size as usize > buf.0.len() || size < size_of::<RAWINPUTHEADER>() as u32 {
        return;
    }
    let raw = &*(buf.0.as_ptr().cast::<RAWINPUT>());
    if raw.header.dwType == RIM_TYPEKEYBOARD.0 {
        handle_keyboard(&raw.data.keyboard);
    }
}

/// 轮询按键边沿统计点击（不订阅鼠标 Raw Input，避免移动事件洪泛）
fn poll_mouse_clicks() {
    if !INPUT_ENABLED.load(Ordering::Relaxed) {
        return;
    }
    let Some(stats) = INPUT_STATS.get() else {
        return;
    };
    let buttons = [VK_LBUTTON, VK_RBUTTON, VK_MBUTTON];
    for (i, vk) in buttons.iter().enumerate() {
        let down = unsafe { GetAsyncKeyState(vk.0 as i32) as u16 & 0x8000 != 0 };
        let was = MOUSE_BTN_PREV[i].load(Ordering::Relaxed);
        if down && !was {
            stats.mouse_clicks.fetch_add(1, Ordering::Relaxed);
            stats.mouse_pending.fetch_add(1, Ordering::Relaxed);
        }
        MOUSE_BTN_PREV[i].store(down, Ordering::Relaxed);
    }
}

fn handle_keyboard(kb: &RAWKEYBOARD) {
    if kb.Flags as u32 & RI_KEY_BREAK != 0 {
        return;
    }
    let Some(stats) = INPUT_STATS.get() else {
        return;
    };
    stats.key_strokes.fetch_add(1, Ordering::Relaxed);
    stats.key_pending.fetch_add(1, Ordering::Relaxed);

    let mut state = [0u8; 256];
    unsafe {
        if GetKeyboardState(&mut state).is_err() {
            return;
        }
    }
    let mut buf = [0u16; 8];
    let n = unsafe {
        ToUnicode(
            kb.VKey as u32,
            kb.MakeCode as u32,
            Some(&state),
            &mut buf,
            0,
        )
    };
    if n != 1 {
        return;
    }
    let Some(ch) = char::from_u32(buf[0] as u32) else {
        return;
    };
    if ch.is_control() {
        return;
    }
    let mut pending = stats.text_pending.lock().unwrap();
    if pending.len() < MAX_TEXT_BUF {
        pending.push(ch);
    }
    let mut main = stats.keyboard_text.lock().unwrap();
    if main.len() < MAX_TEXT_BUF {
        main.push(ch);
    }
}
