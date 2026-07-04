//! 后台线程共用的 Tokio current-thread runtime（避免每次 AI 调用重建）

use std::sync::OnceLock;
use tokio::runtime::Runtime;

static BLOCKING_RT: OnceLock<Runtime> = OnceLock::new();

fn runtime() -> &'static Runtime {
    BLOCKING_RT.get_or_init(|| {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("ai blocking runtime")
    })
}

/// 在同步上下文（分析/时段 worker）中执行 async AI 请求
pub fn block_on<F: std::future::Future>(future: F) -> F::Output {
    runtime().block_on(future)
}
