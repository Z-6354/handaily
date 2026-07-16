# hanimport 角色库可视化管理 — 设计

**日期**: 2026-07-15  
**状态**: 已实现（「导入 Wiki」已由 auto-pipeline 取代）
**来源**: 用户选择 — 完整库管理（C）+ 自用/自带库可切换（B）+ 同服双路由（C）+ 实现方案 1

## 目标

在 **hanimport 本地网页** 中除解包外，增加 **角色库管理** 可视化页：

1. 浏览与增删改 **角色 / 皮肤 / 台词**
2. **自用库打开即自动 Wiki 补齐**（角色 → 头像/皮肤 → 台词；已无「导入 Wiki」按钮）；另可 **同步 AppData / 发布自带库 / 补齐英文名**
3. 可切换 **自用库** 与 **自带库**；写自带库须二次确认
4. **英文名默认 = id**（空则写入 id；历史空值可一键补齐）

## 非目标

- 修改角色/皮肤主键 `id`（v1 创建后不可改）
- 头像上传、音频文件管理
- 本机开发服鉴权 / 多用户
- 进入 hanpet 用户发行版 UI

## 路由与信息架构

| 路径 | 作用 |
|------|------|
| `/` | 现有解包页；顶栏链到 `/roster` |
| `/roster` | 角色库管理页；顶栏链回 `/` |

**顶栏**

- 应用名 + 导航（解包 | 角色库）
- **库切换**：自用库 | 自带库
- 当前库绝对路径灰字；自带库醒目标签「预览库 · 写入需确认」

**主布局（三栏）**

1. **左**：角色列表 — 搜索、分页、新建；显示 `id` / 中文名 / 英文名
2. **中**：当前角色表单 + 皮肤列表（CRUD、设默认）
3. **右/下**：选中皮肤的台词表（CRUD）

**页头操作条**

- 导入 Wiki · 同步 AppData · 发布自带库 · 补齐空英文名  
- 按「操作边界」启用/禁用

**英文名 UI**

- 输入框为空失焦 → 自动填 `id`
- 列表中空英文名以 `id` 灰色占位展示

## 数据源

| `db` | 路径 |
|------|------|
| `local` | `data/roster/handaily-roster.sqlite`（`HANDAILY_ROSTER_DB` 可覆盖） |
| `bundled` | `hanpet/bundled/roster/handaily-roster.sqlite` |

表结构沿用 `data/roster/schema.sql`：`characters` / `skins` / `skin_lines` / `meta`。

## API

前缀 `/api/roster/*`；一律带 `db=local|bundled`（query 或 JSON）。

**写保护**：目标为 `bundled` 的写/删/ops 必须带 `confirm_bundled: true`，否则 `403`。

### 查询 / CRUD

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/roster/meta` | 路径、计数、当前 db |
| GET | `/api/roster/characters?q=&offset=&limit=` | 列表 |
| GET | `/api/roster/characters/{id}` | 详情 + skins |
| POST | `/api/roster/characters` | 新建 |
| PUT | `/api/roster/characters/{id}` | 更新（**不改主键**） |
| DELETE | `/api/roster/characters/{id}` | 删除（级联皮肤/台词） |
| POST | `/api/roster/skins` | 新建皮肤 |
| PUT | `/api/roster/skins/{id}` | 更新 |
| DELETE | `/api/roster/skins/{id}` | 删除 |
| GET | `/api/roster/skins/{id}/lines` | 台词列表 |
| POST | `/api/roster/skins/{id}/lines` | 新建台词 |
| PUT | `/api/roster/lines/{line_id}` | 更新台词 |
| DELETE | `/api/roster/lines/{line_id}` | 删除台词 |

### 批量 / 运维

| 方法 | 路径 | 约束 |
|------|------|------|
| POST | `/api/roster/ops/fill-english` | 空 `name_en` → `id`（角色；皮肤可选同规则） |
| POST | `/api/roster/ops/import-wiki` | **仅 local** |
| POST | `/api/roster/ops/sync-appdata` | **仅 local** |
| POST | `/api/roster/ops/publish-bundled` | **仅 local** → 写 bundled；须确认 |

运维调用复用 `roster_db.py` 内现有命令逻辑（函数抽取），**不**以裸 subprocess 拼 CLI 作为主路径（允许内部复用同一实现）。

## 英文名规则

1. 创建/更新 `characters`：`name_en` 空白 → 存为该行 `id`
2. 导入 Wiki：无可靠英文时写 `id`（不再留空）
3. `fill-english`：批量修补历史空值
4. 皮肤 `name_en`：空白 → 存为该皮肤 `id`（与角色一致）

## 操作边界

| 操作 | 自用库 | 自带库 |
|------|--------|--------|
| CRUD 角色/皮肤/台词 | ✅ | ✅ + `confirm_bundled` |
| 补齐英文名 | ✅ | ✅ + 确认 |
| 导入 Wiki | ✅ | ❌（提示切回自用库） |
| 同步 AppData | ✅ | ❌ |
| 发布自带库 | ✅（确认后覆盖 bundled） | ❌ |

**危险确认（前端）**

- 删除：展示 id + 中文名
- 写自带库：展示路径 + 「可能进入发行预览包」
- 发布：展示 allowlist 条数 + 覆盖 bundled 提示

## 实现落点（方案 1）

```
hanimport/
  scripts/serve_web.py     # 路由 /, /roster, /api/roster/*
  scripts/roster_db.py     # 抽取 connect / CRUD / ops 供 CLI+HTTP
  web/index.html           # 顶栏链
  web/roster.html          # 库管理页
  web/roster.js
  web/roster.css           # 或复用/扩展 style.css
```

技术栈保持 **stdlib HTTP + vanilla JS**，与现网解包页一致。

## 验收

1. `/` ↔ `/roster` 顶栏互跳
2. 切换 local/bundled，列表与路径正确
3. 空英文名保存后为 `id`；一键补齐对历史空值生效
4. 自用库 CRUD 立即反映到 sqlite
5. 写 bundled 无确认 → 403；确认后可写
6. 自用库上导入 Wiki / 同步 AppData / 发布可跑通（失败显示错误摘要）
7. 启动方式仍为现有「启动网页版」

## 决议摘要

| 项 | 选择 |
|----|------|
| 功能深度 | 完整管理（角色+皮肤+台词+运维按钮） |
| 双库 | 可切换；写自带库二次确认 |
| 挂载 | 同服 `/` + `/roster` |
| 实现 | 扩展 `serve_web.py` + `roster_db.py` |
