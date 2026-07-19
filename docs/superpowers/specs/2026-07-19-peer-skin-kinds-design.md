# 桌宠 / 舰娘皮肤平级选择 — 设计

**日期**: 2026-07-19  
**状态**: Phase 1–4 已实现（P5–P6 路线图待做）  
**前置结论**: 方案 1（选择皮肤区顶栏双类）；问答 B（本期仍只播 Spine；独立「舰娘皮肤」功能关闭）

---

## 0. 架构原则（含更长远）

### 0.1 三层模型（稳定核）

过时的不是「一个皮肤槽」，而是把 **内容 / 呈现 / 宿主** 缠在一起，以及把舰娘埋进「动作与台词」。

| 层 | 含义 | 今日落点 | 长期不变点 |
|----|------|----------|------------|
| **Skin slot（内容）** | 哪一套皮：id、名称、台词、头像、资源绑定 | `CharacterSkinMeta` | **继续单一槽**；slot pack 按槽分发 |
| **Kind（呈现）** | 桌宠 Spine **或** 舰娘 Cubism（平级） | UI Tab + `companion` 参数 | 种类可扩展；筛选是槽的投影，不拆库 |
| **Player host（宿主）** | 始终一个桌宠窗 | `PET_LABEL` + 换 `pet.html` / `kanmusu-player.html` | **不**为舰娘再开常驻第三窗；预览窗可废弃或降级 |

```
[ Skin slot ] ──投影──► [ Kind: 桌宠 | 舰娘 ]
                              │
                              ▼ Strategy
                    [ Player host = 桌宠窗 ]
                         ├─ Spine page
                         └─ Cubism page
```

### 0.2 设计模式与软件思想

| 思想 | 用法 |
|------|------|
| **Strategy** | 播放器策略挂在同一 host；切 kind = 换策略，不换窗 |
| **Facade** | UI / 菜单只认 `select(skinId, kind)`；少用含糊的 `auto` |
| **View / Projection** | 桌宠列表 = `model_ready` 投影；舰娘列表 = `kanmusu_*` 投影；**同一数据源** |
| **正交状态** | `active_skin_id`（选了哪槽）⊥ `companion_engine` / kind（用哪种呈现） |
| **YAGNI 分期** | Phase 1 只理 IA + 强制 Spine；不改 schema、不先做 Cubism 同窗 |
| **单一真相渐进收敛** | 长期以 character manifest 上的槽为权威；`kanmusu/manifest` 降为派生缓存或导入副产物 |

### 0.3 今日架构：保留 vs 过时

**保留**

- 一槽双资源（`model_id` + `kanmusu_dir`）+ `skin_index` 合并  
- 运行时 `model_ready` / `kanmusu_ready`  
- 单窗换页播放  
- slot pack 作为分发原子  

**过时 / 应淘汰的形状**

- 舰娘入口挂在「动作与台词」下（发现路径不对等）  
- `companion: "auto"` + 全局引擎泄漏（切皮意外开 Cubism）  
- 人物详情扁平列表 vs 右键菜单双列（筛选逻辑双份）  
- 同步入口三处（详情 Tab / 设置页 / 导入副作用）  
- 双 manifest 长期双写而不标明主从  

### 0.4 演进路线（长远）

| 阶段 | 主题 | 行为 | 架构动作 |
|------|------|------|----------|
| **P1（本期）** | 平级发现 + 行为冻结 | Tab 桌宠/舰娘；点选一律 Spine；关「舰娘皮肤」Tab | UI 投影；`set_skin(..., spine)`；抽共享 filter（详情≈菜单） |
| **P2** | 真分流 | 舰娘 Tab → 同窗 Cubism；桌宠 Tab → Spine | **已实现**：点选分支 `spine`/`kanmusu`；菜单双列 preferEngine 对齐 |
| **P3** | 偏好正交 | 每角色记住「上次 kind」；冷启动可跟 companion 引擎对齐 | **已实现（轻量）**：`localStorage` 按角色记 Tab；无记录时读 `pet_get_companion_engine`。全局播放引擎仍用既有 `companion_engine` DB |
| **P4** | 资源真相 | 导入/同步只写 character 槽；kanmusu catalog 派生 | **已实现**：sync 只拷磁盘；attach 写 character；lookup/list/台词 character-first；旧 kanmusu/manifest 仅 legacy 回退 |
| **P5（可选）** | 宿主统一 | Cubism / Spine 共享交互壳（拖窗、穿透、菜单、编辑范围） | 抽 `CompanionShell`；页面只实现渲染+触区；减少两套 `main.ts` 分叉 |
| **P6（可选）** | 种类扩展 | 如「静态立绘 / 语音包」等新 kind | 新 Strategy + 新投影；**仍不拆 skin slot** |

P2 成功标准：用户心智是「选皮 → 选类 → 桌宠窗播」；不再有「小人皮肤 / 舰娘皮肤」两套产品语义。

### 0.5 刻意不做的长远岔路

- 拆成两套皮肤库（pet skins / kanmusu skins）——与 slot pack、台词、收藏冲突  
- 舰娘永久独立 OS 窗当主路径——与「都用桌宠播放器」冲突  
- 为 kind 新建 zip 格式——继续 `handaily-skin-slot` 一槽双可选资源即可  

---

## 1. 目标（Phase 1）

人物详情「选择皮肤」将 **桌宠** 与 **舰娘** 提升为平级分类（同一套皮肤槽、两种视图），整体交互与现网一致：点选切皮、桌宠窗播放。本期 **不改变** Spine 上桌行为；关闭「动作与台词 → 舰娘皮肤」整块（同步 / 预览 / 上桌）。

## 1.1 非目标（Phase 1）

- 桌宠窗内播 Cubism（→ P2）  
- 恢复独立舰娘预览窗、舰娘上桌入口  
- 改皮肤包格式 / roster schema  
- 设置页 `KanmusuSkinSettings`：隐藏或 disabled（播放器接入前暂关）  
- P3–P6（写入本节作路线图，不在本期施工）  

## 2. 信息架构（Phase 1）

```
人物详情
├── 选择皮肤
│   ├── [桌宠] [舰娘]     ← 平级 Tab（默认「桌宠」）
│   └── 皮肤卡片网格       ← 按当前 Tab 筛选同一 CharacterSkin 列表
└── 动作与台词
    ├── 动作分配
    ├── 台词
    └── 台词导入
    （移除「舰娘皮肤」Tab）
```

右键菜单已有双列思路时，Phase 1 起与详情共用同一套 kind 过滤语义（可先抽 TS helper，P2 再考虑 Rust 侧 view DTO）。

## 3. 筛选与点选规则（Phase 1）

| 分类 | 列表条件 | 点选行为（本期） |
|------|----------|------------------|
| 桌宠 | `model_ready`；未就绪可显示 incomplete 卡 | `characters_set_skin(..., companion: "spine")` |
| 舰娘 | `kanmusu_dir` 非空 **或** `kanmusu_ready` | 仍强制 `"spine"`；不上 Cubism |

说明：

- 同一 `skin.id` 可同时出现在两类；高亮按 `active_skin_id`，与 Tab 无关。  
- 舰娘 Tab 为空：提示「当前角色暂无舰娘皮肤资源」，无同步按钮。  
- 卡片：桌宠 Tab 可用「小人」类提示；舰娘 Tab 不重复打「舰娘」徽标。  

## 4. 状态与 IPC（Phase 1）

- 切皮 / 刷新 / 删除 / 导入：现有 IPC。  
- 人物详情点选一律 `"spine"`；`PersonaPanel.switchSkin` 默认由 `"auto"` 改为 `"spine"`（防全局引擎泄漏）。  
- 若当前引擎为 kanmusu，spine 切皮应回到 Spine 页（走现有 `set_active_model` 路径）。  
- Tab 记忆：session / localStorage 即可。  
- **P2 预留**：舰娘 Tab 点选改为 `"kanmusu"` 单行变更，列表结构不动。  

## 5. 关闭范围（Phase 1）

从 `CharacterPetSettings` 移除「舰娘皮肤」Tab 及同步 / 预览 / 上桌。  
设置页舰娘同步入口：隐藏或 disabled。  
帮助文案：改为「选择皮肤 → 舰娘（本期仍播桌宠）」。

## 6. UI 要点

- 复用 `pet-tab` / `pet-model-card`；皮肤区增加 kind Tab。  
- 导入皮肤、分页挂在「选择皮肤」，两类共用。  
- 不显示内部 model / character id。  

## 7. 验收（Phase 1）

1. 桌宠 Tab：点选 → Spine 上桌，与改前一致。  
2. 舰娘 Tab：仅有舰娘资源的皮；点选仍 Spine，无 Cubism 上桌 / 预览。  
3. 「动作与台词」无「舰娘皮肤」。  
4. 无舰娘资源：舰娘 Tab 空态，不报错。  
5. 导入皮肤包后两类列表按就绪态更新。  

## 8. 实现落点（Phase 1）

| 文件 | 变更 |
|------|------|
| `CharacterSkinPicker.tsx` | kind Tab + 筛选 + 点选强制 spine |
| 新建小模块（建议）`skinKindFilter.ts` | `filterSkinsByKind` / 与菜单对齐的判定，避免双份逻辑 |
| `CharacterPetSettings.tsx` | 移除 kanmusu 段 |
| `PersonaPanel.tsx` | `switchSkin` 默认 spine |
| `helpContent.ts` | 文案 |
| `KanmusuSkinSettings` 引用处 | 隐藏 |

## 9. Phase 2+ 验收意向

- P4：**已实现** — sync 只拷磁盘；character 绑定权威；lookup/list/台词 character-first。  
- P5：拖窗 / 穿透 / 菜单在两种引擎下行为一致。  
- P6：更多 kind（可选）。  
