//! 系统与应用性能快照（Windows 采样 CPU / 内存）

use serde::Serialize;
use std::thread;
use std::time::Duration;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PerformanceSnapshot {
    pub system_cpu_percent: f32,
    pub system_memory_used_bytes: u64,
    pub system_memory_total_bytes: u64,
    pub system_memory_percent: f32,
    pub app_cpu_percent: f32,
    pub app_memory_working_set_bytes: u64,
    pub app_memory_private_bytes: u64,
    pub process_name: String,
}

pub fn capture_snapshot() -> PerformanceSnapshot {
    #[cfg(windows)]
    {
        capture_windows()
    }
    #[cfg(not(windows))]
    {
        PerformanceSnapshot {
            system_cpu_percent: 0.0,
            system_memory_used_bytes: 0,
            system_memory_total_bytes: 0,
            system_memory_percent: 0.0,
            app_cpu_percent: 0.0,
            app_memory_working_set_bytes: 0,
            app_memory_private_bytes: 0,
            process_name: process_display_name(),
        }
    }
}

fn process_display_name() -> String {
    std::env::current_exe()
        .ok()
        .and_then(|p| {
            p.file_name()
                .map(|n| n.to_string_lossy().into_owned())
        })
        .unwrap_or_else(|| "xiaohan-daily".into())
}

#[cfg(windows)]
fn capture_windows() -> PerformanceSnapshot {
    use windows::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
    use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
    use windows::Win32::System::Threading::GetCurrentProcess;

    let process_name = process_display_name();

    let (mut mem_used, mut mem_total, mut mem_pct) = (0u64, 0u64, 0.0f32);
    unsafe {
        let mut status = MEMORYSTATUSEX {
            dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
            ..Default::default()
        };
        if GlobalMemoryStatusEx(&mut status).is_ok() {
            mem_total = status.ullTotalPhys;
            mem_used = status.ullTotalPhys.saturating_sub(status.ullAvailPhys);
            mem_pct = status.dwMemoryLoad as f32;
        }
    }

    let (mut ws, mut private) = (0u64, 0u64);
    unsafe {
        let h = GetCurrentProcess();
        let mut counters = PROCESS_MEMORY_COUNTERS {
            cb: std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
            ..Default::default()
        };
        if GetProcessMemoryInfo(h, &mut counters, counters.cb).is_ok() {
            ws = counters.WorkingSetSize as u64;
            private = counters.PagefileUsage as u64;
        }
    }

    let (sys_cpu, app_cpu) = sample_cpu_percent();

    PerformanceSnapshot {
        system_cpu_percent: sys_cpu,
        system_memory_used_bytes: mem_used,
        system_memory_total_bytes: mem_total,
        system_memory_percent: mem_pct,
        app_cpu_percent: app_cpu,
        app_memory_working_set_bytes: ws,
        app_memory_private_bytes: private,
        process_name,
    }
}

#[cfg(windows)]
fn sample_cpu_percent() -> (f32, f32) {
    use windows::Win32::Foundation::FILETIME;
    use windows::Win32::System::Threading::{GetCurrentProcess, GetProcessTimes};

    unsafe {
        let sys_sample = || -> Option<(u64, u64)> {
            let mut idle = FILETIME::default();
            let mut kernel = FILETIME::default();
            let mut user = FILETIME::default();
            if GetSystemTimes(&mut idle, &mut kernel, &mut user) != 0 {
                let idle = filetime_to_u64(&idle);
                let total = filetime_to_u64(&kernel) + filetime_to_u64(&user);
                Some((idle, total))
            } else {
                None
            }
        };

        let proc_sample = || -> Option<u64> {
            let h = GetCurrentProcess();
            let mut creation = FILETIME::default();
            let mut exit = FILETIME::default();
            let mut kernel = FILETIME::default();
            let mut user = FILETIME::default();
            if GetProcessTimes(h, &mut creation, &mut exit, &mut kernel, &mut user).is_ok() {
                Some(filetime_to_u64(&kernel) + filetime_to_u64(&user))
            } else {
                None
            }
        };

        let (idle1, total1) = sys_sample().unwrap_or((0, 1));
        let proc1 = proc_sample().unwrap_or(0);
        thread::sleep(Duration::from_millis(180));
        let (idle2, total2) = sys_sample().unwrap_or((0, 1));
        let proc2 = proc_sample().unwrap_or(0);

        let idle_delta = idle2.saturating_sub(idle1);
        let total_delta = total2.saturating_sub(total1);
        let sys_cpu = if total_delta == 0 {
            0.0
        } else {
            let used = total_delta.saturating_sub(idle_delta);
            (used as f64 / total_delta as f64 * 100.0) as f32
        };

        let proc_delta = proc2.saturating_sub(proc1);
        let sys_used = total_delta.saturating_sub(idle_delta);
        let app_cpu = if sys_used == 0 {
            0.0
        } else {
            (proc_delta as f64 / sys_used as f64 * 100.0) as f32
        };

        (sys_cpu, app_cpu)
    }
}

#[cfg(windows)]
#[link(name = "kernel32")]
extern "system" {
    fn GetSystemTimes(
        lp_idle_time: *mut windows::Win32::Foundation::FILETIME,
        lp_kernel_time: *mut windows::Win32::Foundation::FILETIME,
        lp_user_time: *mut windows::Win32::Foundation::FILETIME,
    ) -> i32;
}

#[cfg(windows)]
fn filetime_to_u64(ft: &windows::Win32::Foundation::FILETIME) -> u64 {
    ((ft.dwHighDateTime as u64) << 32) | ft.dwLowDateTime as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_fields_sane() {
        let snap = capture_snapshot();
        assert!(snap.system_cpu_percent >= 0.0);
        assert!(snap.app_cpu_percent >= 0.0);
        assert!(!snap.process_name.is_empty());
    }
}
