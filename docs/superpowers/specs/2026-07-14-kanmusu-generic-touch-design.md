# 舰娘通用三区点击（配置驱动）— 设计

**日期**: 2026-07-14  
**状态**: 已批准（用户确认方案 2 + drawable 命中）  
**前置**: `assistantinfo.lua` 通用映射；彩蛋 / `ship_l2d` 后置

## 目标

舰娘 Cubism 预览在点击时，按 drawable 命中通用三区并播放对应动作：

| 优先级（高→低） | Drawable | 动作名 |
|-----------------|----------|--------|
| 1 | `TouchSpecial` | `touch_special` |
| 2 | `TouchHead` | `touch_head` |
| 3 | `TouchBody` | `touch_body` |

对齐碧蓝 `gamecfg/assistantinfo.lua` 的 `assistantTouchParts` / `assistantEvents`（不含 Body 随机 `main_*` 等彩蛋逻辑）。

## 非目标（本轮）

- `ship_l2d` 自定义 mesh / 拖拽 / `action_trigger`
- Body 点击后的 `idleRandom*` / `main_*` 权重池
- 桌宠 Spine 路径
- 本地 `scripts32` 重解密流水线
- 官方 Cubism 全量 motion3 SDK 重写

## 方案

**配置驱动（方案 2）**：

1. **hanimport** `build_cubism_config.py` 生成标准 `touch_areas.json`（含 AL 三区映射与优先级）。
2. **kanmusu-player** 读取该配置：用 Cubism `hit` 对照 `attachments`/`id`，播 `click_animation`。
3. 无配置或不含三区时：回退现有默认 `click_animation` / `touch_body`，不崩溃。

## 数据契约

### `touch_areas.json`（增量字段）

```json
{
  "version": 1,
  "coordinate_space": "drawable",
  "default_click_animation": "touch_body",
  "areas": [
    {
      "id": "TouchSpecial",
      "label": "TouchSpecial",
      "zone": "special",
      "attachments": ["TouchSpecial"],
      "priority": 2,
      "click_animation": "touch_special"
    },
    {
      "id": "TouchHead",
      "attachments": ["TouchHead"],
      "zone": "head",
      "priority": 1,
      "click_animation": "touch_head"
    },
    {
      "id": "TouchBody",
      "attachments": ["TouchBody"],
      "zone": "body",
      "priority": 0,
      "click_animation": "touch_body"
    }
  ],
  "logic": {
    "hit_mode": "drawable",
    "mode": "priority_first",
    "on_click_busy": "ignore",
    "description": "AL generic Touch* via Cubism drawable hitTest; bounds unused"
  }
}
```

约定：

- `click_animation`：优先写包内真实 clip 名（模糊匹配 `touch_head` 等）；匹配不到则仍写逻辑名，播放器再解析。
- `priority`：数值越大优先（Special=2 > Head=1 > Body=0），与碧蓝 `assistantTouchParts` 顺序一致。
- `bounds`：可省略或占位；**命中不以 bounds 为准**。
- 若 bundle 抽出的 Touch* GameObject 名有变体，仍写入 `attachments`，映射规则同上（含 `head`/`special`/`body` 关键字）。

### `animations.meta.json`

维持现有字段。`click_animation` 作为「命中模型但未命中任一配置区」时的回退。

## 播放器行为

`KanmusuInteractor.resolveClickAnimation(x, y)`：

1. 调用 `model.hit(x, y)`（或等价 API）得到命中 drawable 名列表。
2. 若 `touch_areas.logic.hit_mode === "drawable"`（或缺省且存在带 `attachments` 的 areas）：
   - 将 hit 名与各 area 的 `id`/`attachments` 做大小写不敏感匹配；
   - 在匹配成功的 areas 中取 **priority 最高** 者的 `click_animation`。
3. 否则回退：`default_click_animation` → meta.`clickAnimation` → 动画列表中含 `touch_body` 者 → `null`。
4. 播放后回 idle：沿用现有超时回 `idleAnimation` 逻辑。
5. 台词：`pickLine(anim)` 不变。

**不**在本轮实现：Body 区随机 `main_1`… 事件池。

## Sync / IPC

- 现有 sync 已带 `touch_areas` / meta 的，保持；重跑 config 后刷新本地即可。
- IPC payload 字段形状兼容：`touch_areas: TouchArea[]`；`TouchArea` 增加可选 `priority`、`attachments`。

## 验收

1. 含完整三区的皮肤：头 / 身 / 特殊区点击可区分动作（有对应 motion 时可听见/看见差异）。
2. 仅部分区或无 Touch*：不抛错；有区的区可用，否则默认 click。
3. `npm run hanimport -- config --input data/model/unpacked --force`（或等价）后，`touch_areas.json` 含 `hit_mode: drawable` 与三区 priority。
4. 预览窗日志/状态栏能显示当前触发的动作名（已有 onStatus）。

## 后续（明确后置）

- P+：Body → `assistantTouchEvents` 随机池  
- P+：读 `ship_l2d` 自定义 `draw_able_name` / `action_trigger`  
- P+：语音 key（`headtouch` / `touch` / `touch2`）接 cue
