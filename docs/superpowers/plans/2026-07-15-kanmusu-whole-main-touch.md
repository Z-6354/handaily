# 舰娘整体点击 + main_* Implementation Plan

**进度**: 已完成（2026-07-15）

> **For agentic workers:** implement task-by-task; keep changes in `interact.ts`.

**Goal:** 桌宠可点范围覆盖整模；未命中三区时随机 `main_*`。

**Files:** `hanpet/src/kanmusu-player/interact.ts`（主改）

### Task 1: 扩大可点范围

`hitInteractiveLocal`：三区 AABB 并集命中 **或** `hitModelLocal` → true。

### Task 2: 未命中三区播 main_*


`pickMainClickAnimation()`：从 `meta.animations` 筛含 `main_` 的 clip，随机取；否则回退 default/`touch_body`。  
`resolveClickAnimation` 在无三区命中时调用之；`lastHitLabel` 记为 `整体`。

### Task 3: 手动验收

头/身/特殊 / 其它部位 / 无 main 皮肤；状态栏显示动作名。
