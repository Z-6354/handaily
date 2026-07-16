# 舰娘桌面桌宠（替代预览窗）— 设计

**日期**: 2026-07-14  
**状态**: 已实现（共用 pet 窗）；产品入口改为人物皮肤「用舰娘上桌」，不再有独立舰娘页  
**翻转**: 旧 spec「独立带边框预览窗」日常路径废止；产品路径走桌宠 `pet` 窗

## 目标

- 舰娘 Cubism 与 Spine 桌宠**互斥**，共用同一 `pet` 窗口壳（透明、无边框、置顶、穿透）
- 人物皮肤「用舰娘上桌」直接顶替当前桌宠，不弹确认
- 右键菜单完整对齐桌宠；舰娘模式下「切换人物/模型」读统一 characters 皮肤（`kanmusu_dir`）
- 台词继续挂在**皮肤**（roster / `manifest` → `skin.lines`）
- 点击/穿透热区仅 Touch* drawable，禁止整模 AABB

## 非目标（本轮）

- Spine 与 Cubism 同屏双开
- 单页合并双引擎
- `ship_l2d` 彩蛋 / Body 随机池
- 重写桌宠菜单视觉

## 架构

```
主窗舰娘页「放到桌面」
    → companion_engine=kanmusu + character_id/skin_id
    → pet 窗 navigate → kanmusu-player.html
    → Cubism + 皮肤台词 + drawable 热区

菜单/设置切回 Spine 模型
    → companion_engine=spine
    → pet 窗 navigate → pet.html
    → 现有 pet-reload
```

设置键（SQLite settings）：

| key | 值 |
|-----|-----|
| `companion_engine` | `spine` \| `kanmusu` |
| `kanmusu_active_character_id` | 角色 id |
| `kanmusu_active_skin_id` | 皮肤 id |

## 交互

| 行为 | 舰娘模式 |
|------|----------|
| 拖拽 | 超阈值拖 OS 窗口（对齐桌宠） |
| 点击 | Touch* 优先播 `touch_*` + 皮肤台词气泡 |
| 穿透 | 未命中 Touch*（或调试并集矩形）则 `setIgnoreCursorEvents(true)` |
| 滚轮 | 仅「编辑范围」内缩放模型（对齐桌宠） |
| 双击 | 对齐桌宠（打开主窗） |
| 右键 | 打开 pet-menu |
| 编辑范围 | 布置模式：拖模型偏移（`kanmusu_offset_*`）+ 滚轮缩放 + 拖边框改窗大小 |
| 皮肤热切换 | 已在舰娘页时不销毁 pet 窗，只换模型 |
| 闲置生命感 | boot 动作 + random 池定时播放（来自 animations.meta） |

## 验收

1. 「放到桌面」后透明 Cubism，无边框预览窗
2. 切回 Spine → 同位置显示 Spine
3. 空白穿透；Touch* 可点且有动作/台词
4. 菜单齐全；舰娘模式下列表为 kanmusu
5. 气泡开关对舰娘台词生效
