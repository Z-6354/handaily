# 全屏时任务栏闪现 — 修复说明

**日期**: 2026-07-14  
**状态**: 已落地

## 现象

其它应用全屏时，点击/拖动桌宠（含舰娘）可能导致 Windows 任务栏出现。

## 根因（共用 pet 窗，非单套模型）

1. 「始终置顶」下不再做全屏抑制，心跳仍 `SHOWWINDOW` + `TOPMOST`
2. 拖动用 Tauri `setPosition`，会激活窗口抢前台

## 修复

1. 沉浸式全屏（`tracker::win32::is_foreground_fullscreen` 且非本进程）→ 撤 TOPMOST 并 hide；心跳跳过 force-show
2. 退出全屏 → show + topmost + `pet-resume`
3. 拖动/`pet_save_position` 校正位置 → `pet_move_noactivate`（`SWP_NOACTIVATE`）
