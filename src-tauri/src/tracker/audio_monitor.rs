//! 后台音频会话检测（WASAPI）→ 写入时间线
//!
//! 仅当进程有**持续音频输出**且非当前前台时，才记为 `source_type=audio` 的 segment。
//! 微信/设置等仅挂后台无声音的应用不会写入。

use std::collections::{HashMap, HashSet};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use chrono::Local;
use windows::core::{Interface, PCWSTR, PWSTR};
use windows::Win32::Media::Audio::Endpoints::IAudioMeterInformation;
use windows::Win32::Media::Audio::{
    eConsole, eRender, AudioSessionStateActive, IAudioSessionControl,
    IAudioSessionControl2, IAudioSessionEnumerator, IAudioSessionManager2, IMMDevice,
    IMMDeviceEnumerator, MMDeviceEnumerator,
};
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED};

use crate::state::AppState;
use crate::tracker::audio_classify::{self, AudioActivity};
use crate::tracker::display_name;
use crate::tracker::writer;
use crate::tracker::{Segment, IDLE_AGG_KEY};

const POLL_SECS: u64 = 2;
const POLL_IDLE_SECS: u64 = 5;
const MIN_PEAK: f32 = 0.004;
const MIN_AUDIO_FLUSH_MS: u64 = 30_000;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct SessionKey {
    pid: u32,
    session_id: String,
}

#[derive(Debug, Clone)]
struct LiveAudioSession {
    pid: u32,
    session_id: String,
    exe_path: String,
    app_name: String,
    display_name: String,
    peak: f32,
    activity: AudioActivity,
}

pub fn spawn_audio_monitor(state: Arc<AppState>) -> JoinHandle<()> {
    thread::spawn(move || {
        crate::tracker::dampen_thread_priority();
        if !init_com() {
            eprintln!("xiaohan-daily: audio monitor COM init failed");
            return;
        }
        let mut open: HashMap<SessionKey, Segment> = HashMap::new();
        let mut tracking = state
            .tracking_enabled
            .load(Ordering::Relaxed);
        loop {
            if state.stop_flag.load(Ordering::Relaxed) {
                break;
            }
            let enabled = state.tracking_enabled.load(Ordering::Relaxed);
            if tracking && !enabled {
                flush_all_audio(&state, &mut open);
            }
            tracking = enabled;
            if enabled {
                let fg_pid = current_foreground_pid(&state);
                if let Ok(sessions) = poll_active_sessions() {
                    process_tick(&state, &mut open, fg_pid, &sessions);
                }
            }
            thread::sleep(Duration::from_secs(if enabled {
                POLL_SECS
            } else {
                POLL_IDLE_SECS
            }));
        }
        flush_all_audio(&state, &mut open);
    })
}

fn init_com() -> bool {
    unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).is_ok() }
}

fn current_foreground_pid(state: &AppState) -> Option<u32> {
    if let Ok(fg) = state.foreground.lock() {
        if let Some(ref p) = *fg {
            if p.pid != 0 {
                return Some(p.pid);
            }
        }
    }
    crate::tracker::win32::get_foreground_snapshot().map(|s| s.pid)
}

fn process_tick(
    state: &AppState,
    open: &mut HashMap<SessionKey, Segment>,
    fg_pid: Option<u32>,
    sessions: &[LiveAudioSession],
) {
    let now = Local::now().to_rfc3339();
    let mut active = HashSet::new();

    for sess in sessions {
        if sess.peak < MIN_PEAK {
            continue;
        }
        if fg_pid == Some(sess.pid) {
            continue;
        }
        if audio_classify::is_passive_background_process(&sess.exe_path, &sess.app_name) {
            continue;
        }

        let key = SessionKey {
            pid: sess.pid,
            session_id: sess.session_id.clone(),
        };
        active.insert(key.clone());

        if let Some(seg) = open.get_mut(&key) {
            seg.ended_at = Some(now.clone());
            if !sess.display_name.is_empty() && seg.window_title != sess.display_name {
                seg.window_title = sess.display_name.clone();
            }
        } else {
            open.insert(key, segment_from_audio(sess, &now));
        }
    }

    let stale: Vec<SessionKey> = open
        .keys()
        .filter(|k| !active.contains(k))
        .cloned()
        .collect();
    for key in stale {
        if let Some(mut seg) = open.remove(&key) {
            seg.ended_at = Some(now.clone());
            seg.duration_ms = writer::duration_ms(&seg.started_at, &seg.ended_at);
            maybe_flush_audio(state, &seg);
        }
    }
}

fn flush_all_audio(state: &AppState, open: &mut HashMap<SessionKey, Segment>) {
    let now = Local::now().to_rfc3339();
    for (_, mut seg) in open.drain() {
        seg.ended_at = Some(now.clone());
        seg.duration_ms = writer::duration_ms(&seg.started_at, &seg.ended_at);
        maybe_flush_audio(state, &seg);
    }
}

fn maybe_flush_audio(state: &AppState, seg: &Segment) {
    if seg.duration_ms >= MIN_AUDIO_FLUSH_MS {
        writer::flush_audio_segment(state, seg);
    }
}

fn segment_from_audio(sess: &LiveAudioSession, started_at: &str) -> Segment {
    let agg = crate::tracker::derive_aggregation_key(&sess.exe_path, &sess.app_name, "");
    let title = if sess.display_name.trim().is_empty() {
        format!("后台{}", sess.activity.label())
    } else {
        sess.display_name.clone()
    };
    Segment {
        started_at: started_at.to_string(),
        ended_at: None,
        duration_ms: 0,
        app_name: sess.app_name.clone(),
        exe_path: sess.exe_path.clone(),
        window_title: title,
        is_idle: false,
        aggregation_key: if agg.is_empty() {
            IDLE_AGG_KEY.to_string()
        } else {
            agg
        },
        icon: None,
        source_type: "audio".into(),
        audio_activity: sess.activity.as_str().into(),
        activity_label: Some(sess.activity.label().to_string()),
    }
}

fn poll_active_sessions() -> Result<Vec<LiveAudioSession>, String> {
    unsafe {
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
                .map_err(|e| format!("WASAPI enumerator: {e}"))?;
        let device: IMMDevice = enumerator
            .GetDefaultAudioEndpoint(eRender, eConsole)
            .map_err(|e| format!("default audio device: {e}"))?;
        let session_manager: IAudioSessionManager2 = device
            .Activate(CLSCTX_ALL, None)
            .map_err(|e| format!("session manager: {e}"))?;
        let session_enum: IAudioSessionEnumerator = session_manager
            .GetSessionEnumerator()
            .map_err(|e| format!("session enum: {e}"))?;
        let count = session_enum
            .GetCount()
            .map_err(|e| format!("session count: {e}"))?;

        let mut out = Vec::new();
        for i in 0..count {
            let Ok(session) = session_enum.GetSession(i) else {
                continue;
            };
            let control2: IAudioSessionControl2 = match session.cast() {
                Ok(c) => c,
                Err(_) => continue,
            };
            if control2.IsSystemSoundsSession().is_ok() {
                continue;
            }
            let control: IAudioSessionControl = match control2.cast() {
                Ok(c) => c,
                Err(_) => continue,
            };
            let state = control
                .GetState()
                .unwrap_or(windows::Win32::Media::Audio::AudioSessionStateInactive);
            if state != AudioSessionStateActive {
                continue;
            }
            let pid = control2.GetProcessId().unwrap_or(0);
            if pid == 0 {
                continue;
            }
            let meter: IAudioMeterInformation = match session.cast() {
                Ok(m) => m,
                Err(_) => continue,
            };
            let peak = match meter.GetPeakValue() {
                Ok(v) => v,
                Err(_) => continue,
            };
            let session_id = read_session_id(&control2);
            let display_name = read_display_name(&control);
            let exe_path = crate::tracker::win32::get_process_exe_path(pid).unwrap_or_default();
            let app_name = display_name::friendly_name(&exe_path, "", &display_name);
            let activity = audio_classify::classify_audio(&exe_path, &app_name, &display_name);
            out.push(LiveAudioSession {
                pid,
                session_id,
                exe_path,
                app_name,
                display_name,
                peak,
                activity,
            });
        }
        Ok(out)
    }
}

unsafe fn read_session_id(control: &IAudioSessionControl2) -> String {
    control
        .GetSessionIdentifier()
        .ok()
        .map(|pw| pwstr_to_string(pw))
        .unwrap_or_default()
}

unsafe fn read_display_name(control: &IAudioSessionControl) -> String {
    control
        .GetDisplayName()
        .ok()
        .map(|pw| pwstr_to_string(pw))
        .unwrap_or_default()
}

fn pwstr_to_string(pw: PWSTR) -> String {
    if pw.is_null() {
        return String::new();
    }
    unsafe { PCWSTR(pw.0).to_string().unwrap_or_default() }
}
