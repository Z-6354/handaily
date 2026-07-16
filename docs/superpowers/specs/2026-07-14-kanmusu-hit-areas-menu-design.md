# 舰娘菜单：点击区域 / 双栏换模 — 设计

**日期**: 2026-07-14  
**状态**: 已落地

## 决策

- 方案 A：右键菜单开关「显示点击区域」，与「编辑范围」解耦
- 默认关；**仅本次会话**（不写 settings）
- 热区数据：解包 `touch_areas.json` + Cubism `HitAreas`（现有 player 链路）
- 「切换模型」双栏：左「桌宠」(Spine) / 右「舰娘」(Cubism)；点击带 `prefer_engine`

## 实现要点

- `pet_set_hit_areas_visible` → `pet-hit-areas-visible` → `setHitDebug`
- `pet_menu_switch_skin(..., prefer_engine)`
- 舰娘热切换：不销毁 pet 窗；同皮肤短路；先关菜单再切
