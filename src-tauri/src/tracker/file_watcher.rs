//! 监视用户目录文件创建/修改（notify）

use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};

use crate::state::AppState;

pub fn spawn_file_watcher(state: Arc<AppState>) -> JoinHandle<()> {
    thread::spawn(move || {
        crate::tracker::dampen_thread_priority();
        let dirs = watch_dirs();
        if dirs.is_empty() {
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        let mut watcher = match RecommendedWatcher::new(
            move |res| {
                let _ = tx.send(res);
            },
            notify::Config::default(),
        ) {
            Ok(w) => w,
            Err(_) => return,
        };
        for dir in &dirs {
            let _ = watcher.watch(dir, RecursiveMode::Recursive);
        }
        let stats = state.input_stats.clone();
        while !state.stop_flag.load(Ordering::Relaxed) {
            if !state.tracking_enabled.load(Ordering::Relaxed) {
                while rx.try_recv().is_ok() {}
                thread::sleep(Duration::from_millis(500));
                continue;
            }
            match rx.recv_timeout(Duration::from_secs(1)) {
                Ok(Ok(event)) => match event.kind {
                    EventKind::Create(_) => {
                        stats.files_created.fetch_add(1, Ordering::Relaxed);
                    }
                    EventKind::Modify(_) => {
                        stats.files_modified.fetch_add(1, Ordering::Relaxed);
                    }
                    _ => {}
                },
                _ => {}
            }
        }
    })
}

fn watch_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(user) = std::env::var("USERPROFILE") {
        for sub in ["Desktop", "Documents", "Downloads"] {
            let p = PathBuf::from(&user).join(sub);
            if p.is_dir() {
                dirs.push(p);
            }
        }
    }
    dirs
}
