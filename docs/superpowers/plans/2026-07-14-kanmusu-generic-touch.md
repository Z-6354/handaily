# 舰娘通用三区点击 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 配置驱动：Cubism drawable 命中 `TouchSpecial`/`TouchHead`/`TouchBody` 并播放对应 `touch_*` 动作。

**Architecture:** `build_cubism_config.py` 写出带 `priority` + `hit_mode:drawable` 的 `touch_areas.json`，并在有 `model3.json` 时注入 `HitAreas`；Rust IPC 透传 `priority`/`attachments`；`KanmusuInteractor` 用 `model.hit` + 优先级匹配播动作。

**Tech Stack:** Python 3 / UnityPy、Rust (serde)、TypeScript、pixi-live2d-display Cubism4

## Global Constraints

- 对齐 `assistantinfo.lua` 三区映射；不做 `ship_l2d` / Body 随机池 / Spine
- `priority`：Special=2 > Head=1 > Body=0
- 命中以 drawable/`model.hit` 为准，不以 bounds 为准
- 无三区时回退 default click，不抛错
- 本轮不自动 git commit（除非用户明确要求）

## File map

| File | Responsibility |
|------|----------------|
| `hanimport/scripts/build_cubism_config.py` | 生成 touch_areas + 可选注入 model3 HitAreas |
| `hanpet/src-tauri/src/kanmusu/mod.rs` | `KanmusuTouchArea` 字段 + `read_touch_areas` |
| `hanpet/src/kanmusu-player/interact.ts` | drawable 优先级命中解析 |
| `hanpet/src/kanmusu-player/main.ts` | payload 类型对齐 |

---

### Task 1: hanimport 写出 drawable 三区配置

**Files:**
- Modify: `hanimport/scripts/build_cubism_config.py`
- Test: inline `python -c` / 小函数断言（同文件或脚本自测）

**Interfaces:**
- Produces: `build_touch(touches, anims, click) -> dict` 含 `coordinate_space: "drawable"`, `logic.hit_mode: "drawable"`, `logic.mode: "priority_first"`, areas 含 `priority`/`attachments`/`click_animation`
- Produces: `ensure_model3_hit_areas(model_dir, areas)` 可选写回 model3 `HitAreas`

- [x] **Step 1: 抽出标准 AL 区优先级常量与 `click_for` 保持**

在 `build_cubism_config.py` 增加：

```python
AL_TOUCH_PRIORITY = {
    "special": 2,
    "head": 1,
    "body": 0,
}
LOGIC_CLICK = {
    "special": "touch_special",
    "head": "touch_head",
    "body": "touch_body",
}
```

- [x] **Step 2: 重写 `build_touch`**

对每个 touch 名：`zone = zone_for(name)`，`priority = AL_TOUCH_PRIORITY[zone]`，`click_animation = click_for(...) or LOGIC_CLICK[zone]`，`attachments = [name]`，**不要求 bounds 参与命中**（可省略或写空占位）。顶层：

```python
{
  "version": 1,
  "coordinate_space": "drawable",
  "default_click_animation": click or "touch_body",
  "areas": areas,
  "logic": {
    "hit_mode": "drawable",
    "mode": "priority_first",
    "on_click_busy": "ignore",
    "description": "AL generic Touch* via Cubism drawable hitTest; bounds unused",
  },
}
```

若 `touches` 为空，仍写入三区 stub（`TouchSpecial`/`TouchHead`/`TouchBody`）以便命中游戏模型里常见 drawable；`click_animation` 用 `click_for` 对逻辑名解析。

- [x] **Step 3: `ensure_model3_hit_areas`**

若目录内存在 `*.model3.json`：合并 `FileReferences` 旁的 `HitAreas` 数组，对每个 area 写入 `{"Name": id, "Id": id}`（已存在则跳过）。使 `model.hit` 能返回 Touch* 名。

在 `process_slug` 写完 touch 后调用。

- [x] **Step 4: 自测 build_touch**

Run:

```powershell
cd D:\0HAN\HANDAILY
python -c "from hanimport.scripts.build_cubism_config import build_touch; t=build_touch(['TouchHead','TouchBody'], ['idle','touch_head','touch_body'], 'touch_body'); assert t['logic']['hit_mode']=='drawable'; assert max(a['priority'] for a in t['areas'] if 'Head' in a['id'])==1; print('ok', t)"
```

Expected: `ok` 与含 priority 的 dict（若 import 路径失败，改用 `sys.path` 插入 `hanimport/scripts`）。

---

### Task 2: Rust IPC 透传 priority / attachments

**Files:**
- Modify: `hanpet/src-tauri/src/kanmusu/mod.rs` (`KanmusuTouchArea`, `read_touch_areas`)

**Interfaces:**
- Consumes: `touch_areas.json` areas 项
- Produces: `KanmusuTouchArea { id, zone, click_animation, priority, attachments, bounds }`

- [x] **Step 1: 扩展结构体**

```rust
pub struct KanmusuTouchArea {
    pub id: String,
    pub zone: String,
    pub click_animation: Option<String>,
    #[serde(default)]
    pub priority: i32,
    #[serde(default)]
    pub attachments: Vec<String>,
    #[serde(default)]
    pub bounds: KanmusuTouchBounds,
}
```

`KanmusuTouchBounds` 给 `Default`（0.2/0.2/0.6/0.6）。`read_touch_areas`：**bounds 缺失时用 Default**，不要 `?` 丢弃整条；读 `priority`（缺省按 zone：special=2,head=1,else=0）；读 `attachments`（缺省 `[id]`）。

- [x] **Step 2: `cargo check -p hanpet`（或 workspace 对应 package）**

Expected: 编译通过。

---

### Task 3: 播放器 drawable 优先级命中

**Files:**
- Modify: `hanpet/src/kanmusu-player/interact.ts`
- Modify: `hanpet/src/kanmusu-player/main.ts`

**Interfaces:**
- Consumes: `TouchArea { id, zone, click_animation?, priority?, attachments?, bounds }`
- Produces: `resolveClickAnimation(x,y): string | null` 按 hit ∩ areas priority

- [x] **Step 1: 扩展 `TouchArea` 类型**

```typescript
export interface TouchArea {
  id: string;
  zone: string;
  click_animation?: string | null;
  priority?: number;
  attachments?: string[];
  bounds: TouchAreaBound;
}
```

`InteractMeta` 增加可选 `defaultClickAnimation?: string | null`。

- [x] **Step 2: 重写 `resolveClickAnimation`**

逻辑：

1. `hits = model.hit?.(x,y) ?? []`（字符串数组，tolower）
2. 对每个 area，归一化 `attachments ?? [id]`，若任一 attachment/id 与 hits 某项相等或相互包含（大小写不敏），记为候选
3. 候选中取 `priority` 最大（缺省：special=2, head=1, body=0 via zone）→ 返回其 `click_animation`
4. 无候选：返回 `defaultClickAnimation ?? clickAnimation ?? animations.find(a => a.toLowerCase().includes('touch_body')) ?? null`
5. **不再**优先用 bounds 归一化命中（可删旧 bounds 分支或整块去掉）

- [x] **Step 3: `main.ts` payload 类型** 增加 `priority?`、`attachments?`；attach 时传入 `defaultClickAnimation: payload.click_animation`（或将来从 touch 文件顶层字段透传；若 Rust 未传顶层 default，用 click_animation 即可）。

- [ ] **Step 4: 手工验收清单**

1. 重跑 config（有 bundle 时）或手改一份 `touch_areas.json` 为 spec 形状  
2. sync + 预览：点头/身/特殊区状态栏动作名不同  
3. 无 Touch 的模型不白屏

---

## Spec coverage

| Spec 项 | Task |
|---------|------|
| touch_areas hit_mode/priority/三区 | T1 |
| model3 HitAreas 支撑 hit | T1 ensure_model3 |
| IPC attachments/priority | T2 |
| resolveClickAnimation drawable | T3 |
| 回退链 | T3 |
| 非目标彩蛋 | 不做 |

## Placeholder scan

无 TBD。
