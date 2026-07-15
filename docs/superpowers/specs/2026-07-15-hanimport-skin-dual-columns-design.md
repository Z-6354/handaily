# hanimport 皮肤双列清单（桌宠 / 舰娘）— 设计

**日期**: 2026-07-15  
**状态**: 已实现（v1）  
**来源**: 清单展示 A · 详情摘要+全库页 3 · 实现方案 A（扩展 roster）  
**相关**: 角色库 `2026-07-15-hanimport-roster-browser-design.md`；头像网格 `2026-07-15-hanimport-roster-avatar-grid-design.md`；统一皮肤 `2026-07-14-unified-character-skins-design.md`

## 目标

1. 在网页上展示库中每条皮肤的 **桌宠 Spine**（`pet_model_id`）与 **舰娘 Live2D**（`kanmusu_dir`）两列就绪状态  
2. **角色详情**内皮肤区为双列表格摘要；另设 **`/skins`** 全库皮肤浏览页  
3. v1 **不**嵌入 Spine/Cubism 播放器，只做清单 + 本地文件探针  

## 非目标

- WebGL / Live2D / Spine 真机预览（后续可做 C）  
- 自动补齐 `pet_model_id` / `kanmusu_dir` 绑定  
- 改 schema 主键或拆表  
- 对 bundled 特殊写入  

## 数据与状态

沿用 `skins.pet_model_id`、`skins.kanmusu_dir`。

| 状态 | 条件 |
|------|------|
| `unbound` | 字段为空 |
| `missing` | 有绑定，本地无资源 |
| `ready` | 有绑定且本地存在 |

**探针约定（v1）**

- 桌宠：`data/live2d/{pet_model_id}/` 下存在 `.skel` 或 Spine `.json` + `.atlas`（与现有解包布局一致；若项目另有 `hanpet/.../pet-models` 绑定路径，探针函数集中配置，可扩展多 root）  
- 舰娘：`data/model/unpacked/{kanmusu_dir}/` 下存在 `*.model3.json` 或该目录非空且含 cubism 典型文件  

API 每皮附带：`pet_status`、`kanmusu_status`，可选 `pet_path`、`kanmusu_path`。

## API

| 路径 | 作用 |
|------|------|
| `GET /api/roster/skins?db=&character_id=&q=&status=&offset=&limit=` | 分页皮肤列表 + 双列状态；`status=missing\|ready\|unbound` 可选过滤（对两侧 OR 或参数拆分见实现） |
| `GET /api/roster/characters/{id}` 的 `skins[]` | 同样附带双列状态 |

## UI

### 顶栏

`概览 · 解包 · 角色库 · 皮肤` → `/skins`

### 角色详情摘要

皮肤区表格：皮肤名 | 桌宠 | 舰娘 | 默认；色点+短文案+等宽 id；保留 CRUD；链接「在皮肤页查看全部」。

### `/skins` 全表

筛选：角色、搜索、仅缺文件、仅双就绪；表：角色 | 皮肤 | 桌宠 | 舰娘 | 跳到角色库；分页；浅色壳。

## 验收

1. 详情皮肤表显示两列三态正确（对照本地目录 spot-check）  
2. `/skins` 可浏览全库并筛选；可跳回角色库对应角色  
3. 无播放器依赖；探针仅本机路径  
4. local / bundled 只读展示均可用  

## 决策记录

| 项 | 选择 |
|----|------|
| 深度 | A 清单 only |
| 位置 | 详情 + `/skins`（3） |
| 工程 | 扩展 roster（A） |
