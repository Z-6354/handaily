// 小寒日报 — Tauri 2 主入口（bin crate 委托 lib crate）
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    xiaohan_daily_lib::run()
}
