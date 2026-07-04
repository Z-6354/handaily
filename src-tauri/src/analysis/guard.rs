//! 系统资源守卫：高 CPU 占用时跳过截图

use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::analysis::AnalysisSettings;

pub struct SystemGuard {
    last_screenshot: Mutex<Option<Instant>>,
}

impl SystemGuard {
    pub fn new() -> Self {
        Self {
            last_screenshot: Mutex::new(None),
        }
    }

    pub fn can_screenshot(
        &self,
        settings: &AnalysisSettings,
        exe_path: &str,
        aggregation_key: &str,
    ) -> Result<(), &'static str> {
        if !settings.screenshot_enabled {
            return Err("截图分析已关闭");
        }

        let key = aggregation_key.to_lowercase();
        let exe = exe_path.to_lowercase();
        for ex in &settings.excluded_exes {
            if !ex.is_empty() && (key.contains(ex) || exe.contains(ex)) {
                return Err("应用在排除列表中");
            }
        }

        if system_cpu_percent() >= settings.cpu_threshold_percent {
            return Err("系统 CPU 占用过高");
        }

        if let Ok(guard) = self.last_screenshot.lock() {
            if let Some(last) = *guard {
                let elapsed = last.elapsed().as_secs();
                if elapsed < settings.screenshot_min_interval_secs {
                    return Err("截图频率限制");
                }
            }
        }

        Ok(())
    }

    pub fn mark_screenshot_taken(&self) {
        if let Ok(mut g) = self.last_screenshot.lock() {
            *g = Some(Instant::now());
        }
    }
}

/// 采样两次系统 CPU 使用率（0–100）
#[cfg(windows)]
pub fn system_cpu_percent() -> f32 {
    use std::thread;
    use windows::Win32::Foundation::FILETIME;

    unsafe {
        let sample = || -> Option<(u64, u64)> {
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

        let (idle1, total1) = sample().unwrap_or((0, 1));
        thread::sleep(Duration::from_millis(180));
        let (idle2, total2) = sample().unwrap_or((0, 1));

        let idle_delta = idle2.saturating_sub(idle1);
        let total_delta = total2.saturating_sub(total1);
        if total_delta == 0 {
            return 0.0;
        }
        let used = total_delta.saturating_sub(idle_delta);
        (used as f64 / total_delta as f64 * 100.0) as f32
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

#[cfg(not(windows))]
pub fn system_cpu_percent() -> f32 {
    0.0
}

#[cfg(windows)]
fn filetime_to_u64(ft: &windows::Win32::Foundation::FILETIME) -> u64 {
    ((ft.dwHighDateTime as u64) << 32) | ft.dwLowDateTime as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_percent_in_range() {
        let p = system_cpu_percent();
        assert!(p >= 0.0 && p <= 100.0);
    }
}
