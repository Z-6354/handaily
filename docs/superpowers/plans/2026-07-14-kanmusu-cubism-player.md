# 舰娘 Cubism 播放器 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans or implement task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 新增舰娘页 + 独立 Cubism 播放窗口 + 删除全部非内置 Spine 模型；保留现有桌宠逻辑。

**Architecture:** 独立 `kanmusu` 数据目录与 manifest；主窗口只做管理；`kanmusu-player` Tauri 窗口加载 Cubism；与 pet Spine 隔离。

**Tech Stack:** Tauri 2、React、Cubism 4 Web（via `pixi-live2d-display` Cubism4 或官方 core）、现有 IPC 模式。

## Global Constraints

- 不修改桌宠宠物窗播放逻辑（除「删除全部」入口）
- 播放器默认关闭
- UTF-8 全文
- Windows 主平台

---

## File map

| Path | Role |
|------|------|
| `hanpet/src/pages/KanmusuPanel.tsx` | 舰娘主页面 |
| `hanpet/src/kanmusu/*` | 前端 API / 类型 |
| `hanpet/kanmusu-player.html` + `src/kanmusu-player/*` | 独立播放器入口 |
| `hanpet/src-tauri/src/kanmusu/*` | Rust 数据 + 窗口 IPC |
| `hanpet/src-tauri/src/pet/models.rs` | `delete_all_user_models` |
| `hanpet/src/App.tsx` | 导航 |
| `hanpet/vite.config.ts` / tauri.conf | 多页面 / 窗口 |

---

### Task 1: 删除全部非内置 Spine 模型

**Files:** `pet/models.rs`, IPC, PersonaPanel or Settings UI

- [ ] 后端 `delete_all_user_models(data_dir, db)`：列出非 builtin，unlink skins refs，删目录
- [ ] IPC `pet_delete_all_user_models` + 确认文案
- [ ] UI 按钮 + 二次确认
- [ ] 手工/API 验证：内置仍在，用户模型清空

### Task 2: Kanmusu 数据层 + 种子

**Files:** `src-tauri/src/kanmusu/mod.rs`, `data_layout.rs`

- [ ] `kanmusu/manifest.json` schema + CRUD
- [ ] `sync_from_unpacked(repo data/model/unpacked)` → AppData `kanmusu-models/`
- [ ] 首启或「刷新本地」导入当前解包的 11 个

### Task 3: 舰娘列表页（主窗口）

**Files:** `App.tsx`, `KanmusuPanel.tsx`

- [ ] 导航「舰娘」
- [ ] 角色列表 + 简介 + 皮肤列表 + 皮肤台词编辑
- [ ] 「打开预览」按钮（不自动开窗）

### Task 4: 独立播放器窗口 + Cubism

**Files:** `kanmusu-player.html`, `src/kanmusu-player/main.ts`, tauri window

- [ ] 注册 `kanmusu-player` 窗口（decorations=true，visible=false）
- [ ] 接入 Cubism4 加载 moc3/model3/textures
- [ ] IPC：`kanmusu_player_open` / `load` / `close`
- [ ] 边框窗口放大播放单皮肤

### Task 5: 联调验收

- [ ] 从舰娘页选 aidang_2 → 预览窗播放
- [ ] 关闭预览后再打开不崩
- [ ] 删除全部 Spine 用户模型不影响 kanmusu-models
