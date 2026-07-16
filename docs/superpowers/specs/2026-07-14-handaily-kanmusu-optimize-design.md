# 舰娘 / hanpet 小步优化 — 设计

**日期**: 2026-07-14  
**状态**: 已落地（方案 A：P0→P3；验收补齐进行中）

## 目标

在不大改人物/桌宠的前提下，收敛近期舰娘功能带来的稳定、数据、DX 与轻量性能问题。

## 范围

| 阶段 | 内容 | 状态 |
|------|------|------|
| P0 | Live2D 依赖统一为 lipsyncpatch；Vite 缓存/optimizeDeps；预览窗错误与切换可靠 | 完成 |
| P1 | Cubism `animations.meta`/`touch_areas` 进入 hanimport 流水线；sync 带配置；面板可读 meta 摘要 | 完成（人物页「舰娘皮肤」展示 idle/click/触区） |
| P2 | kanmusu IPC/权限/capability 对齐；冗余清理 | 完成（`allow-kanmusu-player-commands` 最小集） |
| P3 | 预览切换释放 blob/模型；脚本说明 | 完成（`unloadCurrent` / blob dispose / 同皮肤 keepBlobs） |

## 非目标

- 不上官方 Cubism 全量 motion3 SDK 重写
- 不重构人物页 / Spine 桌宠核心
- 不做全量性能 profiling

## 验收

1. `tauri:dev` 不报 `pixi-live2d-display` ENOENT — **通过**（仅 lipsyncpatch）
2. 点预览可加载；切换皮肤不残留旧模型 — **通过**（人物页「舰娘预览」+ destroy/blob）
3. `data/model/unpacked/*` 均有 meta；刷新本地可同步到 AppData — **本地 unpacked 已具备**；同步走「从解包同步舰娘」
4. 舰娘页能看到 idle/click 等配置摘要 — **通过**（人物 → 动作与台词 → 舰娘皮肤）

## 产品入口

- **舰娘预览**：独立带边框窗（不顶替桌宠）
- **舰娘上桌**：共用 pet 窗顶替 Spine
