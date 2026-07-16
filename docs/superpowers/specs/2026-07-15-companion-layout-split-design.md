# 桌宠 / 舰娘布局分存 — 设计

**日期**: 2026-07-15  
**状态**: 已实现

## 目标

Spine 桌宠与舰娘 Cubism 的窗位置、窗大小、缩放、模型偏移各自独立；切引擎时立刻跳到该引擎上次保存的布局。

## Settings 键

| 含义 | Spine | 舰娘 |
|------|-------|------|
| 窗位置 | `pet_x` / `pet_y` | `kanmusu_x` / `kanmusu_y` |
| 窗宽高 | `pet_width` / `pet_height` | `kanmusu_width` / `kanmusu_height` |
| 缩放 | `pet_scale` | `kanmusu_scale` |
| 模型偏移 | `pet_offset_*`（已有） | `kanmusu_offset_*`（已有） |

## 行为

- 读写一律按当前 `companion_engine`
- `apply_companion_engine` / 重建窗后：读目标引擎布局并 `set_pet_window_bounds` + scale
- 舰娘键缺失时：窗位/尺寸从 `pet_*` 拷贝；**缩放默认 0.8，不拷贝 `pet_scale`**（避免桌宠 1.5 带过去像「共用」）
- `companion_layout_seed_v=2`：若发现 `kanmusu_scale == pet_scale`（v1 错误拷贝特征），一次性改回 `0.8`

## 验收

1. 桌宠 scale=1.5 → 切舰娘若未调过可用拷贝初值，但之后改舰娘 scale 不影响桌宠
2. 改舰娘后切回桌宠，桌宠仍为原布局
