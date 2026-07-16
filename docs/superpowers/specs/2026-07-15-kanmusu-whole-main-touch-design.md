# 舰娘整体点击区 + main_* — 设计

**日期**: 2026-07-15  
**状态**: 已实现  
**前置**: `2026-07-14-kanmusu-generic-touch-design.md`

## 目标

除 `TouchSpecial` / `TouchHead` / `TouchBody` 三区外，增加整模可点范围，避免点到角色身上无反应；未命中三区时随机播放 `main_*` 动作。

## 行为

| 优先级 | 条件 | 动作 |
|--------|------|------|
| 1 | Cubism hitTest 命中三区 | 对应 `touch_*` |
| 2 | 点在模型包围盒内且未命中三区 | 随机 `main_*`；无则回退 `default_click` → `touch_body` |
| — | 模型外 | 穿透 / 无点击 |

## 实现要点

- `hitInteractiveLocal`：三区并集 **或** 模型 AABB → 可点（穿透轮询同步扩大）
- `resolveClickAnimation`：无三区命中时 `pickMainClickAnimation()`
- 不改正时随机池；不改 `touch_areas.json`；不做 Spine

## 验收

1. 三区仍区分 `touch_*`
2. 整模其它部位有反馈，多为 `main_*`
3. 无 `main_*` 皮肤回退不报错
