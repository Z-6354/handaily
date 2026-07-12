//! 内置资源：由 build.rs 从 `bundled/` 生成，编译期嵌入

include!(concat!(env!("OUT_DIR"), "/embedded.rs"));
