//! 统一 stderr 输出：发布版仅保留 warn；调试构建额外输出 info。

#[inline]
pub fn warn(msg: impl std::fmt::Display) {
    eprintln!("xiaohan-daily: {msg}");
}

#[inline]
pub fn info(msg: impl std::fmt::Display) {
    #[cfg(debug_assertions)]
    eprintln!("xiaohan-daily: {msg}");
}
