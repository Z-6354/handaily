# hanimport 角色库 Wiki 式头像预览 — 设计

**日期**: 2026-07-15  
**状态**: 待用户审阅（brainstorm 已批准）  
**来源**: 用户选择 — 本地落盘 C · 网格+详情混合 3 · 打开页自动补齐 C · 实现方案 A · **通知式进度（非模态）+ 暂停 · 与主流程解耦**；并强调浮层 UI 品质

**相关**: 角色库 CRUD 仍以 `2026-07-15-hanimport-roster-browser-design.md` 为准；外壳视觉以 `2026-07-15-hanimport-apple-hub-redesign-design.md` 为准。

## 目标

1. 头像下载到 **`data/roster/avatars/{character_id}.{ext}`**，角色库离线可读  
2. `/roster` 左侧改为 **Wiki 风格头像网格**；点选后右侧保留现有详情（角色/皮肤/台词）  
3. 打开**自用库**时若缺图，**后台 job** 自动补齐；进度用 **右下角通知浮层**展示（不挡主界面），带 **暂停/继续**  
4. Wiki 导入成功后的新角色一并入队下载  

## 非目标

- 不改造 hanpet AppData 头像缓存路径（可后续可选「从 AppData 复制」）  
- 不对 **bundled** 库自动联网下载（仅展示本地已有文件）  
- 居中模态进度弹窗（已否决）  
- 头像上传/手动更换 UI（v1）  

## 存储与 HTTP

| 项 | 约定 |
|----|------|
| 目录 | `data/roster/avatars/`（目录保留 `.gitkeep`；图片建议 gitignore） |
| 文件 | `{id}.jpg` / `.jpeg` / `.png` / `.webp`（保留源扩展名，常见为 jpg） |
| 有无图 | 以文件存在为准；可选 `meta_json.avatar_ext` 加速列表 |
| `GET /avatars/{id}` | 仅服务该目录；禁 `..`；404=无图；可用 `?t=mtime` 破缓存 |
| 列表 API | `GET /api/roster/characters` 每项增加 `avatar_url`: 有本地则为 `/avatars/{id}`，否则 `null` |

### URL 解析（下载源）

1. `characters.wiki_title` → wiki DB `catalog.avatar_url`  
2. 回退 `name_zh` / `catalog.display_name`  
3. 皆无 → skip（卡片首字占位）  

Wiki DB：`mcp/blhx-wiki/data/blhx.sqlite` 表 `catalog`（非 `ships.avatar_url`）。

## 后台 job（与主进程分开）

复用/扩展 `job_store`：`kind = "fetch-avatars"`。

| API | 作用 |
|-----|------|
| `POST /api/roster/ops/fetch-avatars` | body: `{ missing_only?: true, ids?: string }`；`db=local` only；启动 job，返回 `{ ok, job_id }` |
| `GET /api/jobs/{id}` | 进度：`current`/`total`/`current_item`/`ok_count`/`fail_count`/`skipped`/`status`；扩展 `phase`: `running` \| `paused` \| `done` \| `error` |
| `POST /api/jobs/{id}/pause` | 暂停队列（当前单张下载可结束后再停） |
| `POST /api/jobs/{id}/resume` | 继续 |
| （可选）`POST /api/jobs/{id}/cancel` | 清空剩余；已落盘保留 |

- `import-wiki` upsert 后，将本批 id 入队同一下载实现（可合并进进行中的 fetch job，或新开 job）  
- 服务端并发：串行或小并发（建议 **2**），避免打爆 Wiki CDN  

## UI：网格 + 详情

```
┌──────────────────────────────────────────────────┐
│ 角色库  [自用|自带]  搜索…                         │
├────────────────────┬─────────────────────────────┤
│ 头像网格            │ 详情（角色表单/皮肤/台词）     │
│ （Wiki 墙）         │ 未选中：提示点选左侧卡片        │
└────────────────────┴─────────────────────────────┘
```

- 卡片：约 72–96px 头像 + 中文名 + 灰 id；选中蓝描边  
- 无图：色块 + 名首字；下载完成后仅刷新该卡片 `img`  
- 分页默认 **48**/页；搜索逻辑保留  
- 窄屏：网格全宽，点选后详情叠下方  

## UI：通知式进度浮层（必做，非模态）

**禁止**全屏遮罩居中 dialog。

### 视觉（对齐现有浅色壳）

- 位置：**右下角**，`z-index` 高于内容、低于系统级（约 200）  
- 卡片：白底 `--surface`、`--hairline` 边、`--shadow-card`、圆角 **16px**、内边距 14–16px  
- 宽度约 **320–360px**；标题 14–15px semibold；副文 muted 12–13px  
- 进度条高度 4–6px、`--accent` 填充、圆角满；数字用 `--font-mono`  
- 主操作 **暂停 / 继续**用小胶囊按钮（次要描边或轻底，不是大 primary 抢戏）  
- 可选极小「取消剩余」文字链；**✕** = 仅隐藏浮层，**不停止** job（等同缩小到顶栏微条）  
- 入场：短 fade + 上移 8px；尊重 `prefers-reduced-motion`  

### 文案与状态

| 状态 | 展示 |
|------|------|
| running | 「头像补齐」+ `current/total` + 当前舰名 + 成功/失败计数 + **暂停** |
| paused | 「已暂停」+ **继续** |
| done | 「补齐完成」成功/失败摘要，约 3s 后自动收起 |
| error | 一行错误原因 + 可关 |

### 行为

- 不挡点击网格/表单/搜索  
- 打开自用库、`loadCharacters` 后若存在缺图 → 自动 `fetch-avatars` 并显示浮层  
- 刷新页面：若有未完成 `fetch-avatars` job，重新挂上浮层  
- bundled：不启动下载、不显示下载浮层  

## 错误处理

- 单张失败计入 `fail_count`，继续  
- 无 URL：skipped  
- 网络整体失败：job `error`，浮层提示，不中断编辑  

## 验收

1. 文件落到 `data/roster/avatars/`；`GET /avatars/{id}` 200  
2. 左网格 Wiki 式预览；点选出右侧详情  
3. 自用库缺图 → **通知浮层**进度 + **暂停/继续**；主界面可照常操作  
4. 导入 Wiki 新人队下载  
5. 离线可读已缓存头像；bundled 不自动联网下图  

## 决策记录

| 项 | 选择 |
|----|------|
| 落盘 | `data/roster/avatars/`（C） |
| 布局 | 网格 + 详情（3） |
| 触发 | 打开自用库自动补齐（C） |
| 工程 | 文件服务 + job（A） |
| 进度 UI | 右下角通知浮层 + 暂停（非模态） |
