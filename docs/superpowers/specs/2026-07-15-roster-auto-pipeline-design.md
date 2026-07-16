# 自用库 Wiki 全自动补齐流水线 — 设计

**日期**: 2026-07-15  
**状态**: 已实现  
**来源**: 用户选全自动 B（Wiki 全量同步角色）· 删除「导入 Wiki」· 顺序角色→头像/皮肤→台词  
**相关**:  
`2026-07-15-hanimport-roster-avatar-grid-design.md` ·  
`2026-07-15-wiki-tabcontainer-skins-design.md` ·  
`2026-07-15-per-skin-wiki-lines-design.md` ·  
`2026-07-15-wiki-lines-auto-fetch-design.md`

## 目标

1. **/roster 自用库**打开（或刷新）即自动跑完整补齐，**无需**再点「导入 Wiki」  
2. **删除**「导入 Wiki」按钮及相关手动触发入口（CLI `import-wiki` 可保留给调试）  
3. 硬顺序：**角色 → 头像 + 皮肤 → 台词**（不可头像/台词抢跑在皮肤对齐之前写台词）  
4. 皮肤以 Wiki `skins_json` 为权威整角色替换；解包只挂模型，禁止文件夹名脏皮

## 非目标

- bundled 库自动联网  
- 改 hanpet 播放逻辑  
- 居中模态进度（沿用右下角 toast）  
- 自动「发布自带库」

## 触发

| 条件 | 行为 |
|------|------|
| `db=local` 且列表加载成功 | 启动（或附着已有）编排 job `kind=roster-wiki-pipeline` |
| 已有同 kind 在 `running`/`paused` | 不重启，UI 附着轮询 |
| `db=bundled` | 不启动 |

可选：解包完成回调同样可入队同一编排（与打开页共用，去重）。

## 编排阶段

### Phase 1 — 角色（Wiki → roster）

- 源：`mcp/blhx-wiki/data/blhx.sqlite`（及现有 fallback 路径）`ships` 全表  
- 行为：复用现有 `run_import_wiki` 中 **角色 upsert** 部分（阵营/CV/稀有度/`wiki_title` 等）  
- **本阶段不写台词**；皮肤可与 Phase 2 合并，但禁止在皮肤未替换完成前写 `skin_lines`  
- 推荐：`import_modes = characters_only | skins_bind | lines` 拆分现有 import，或编排器按 phase 调专用函数  

完成判据：本批目标舰船均已 upsert 进 `characters`（或记 fail 后继续）。

### Phase 2 — 头像 + 皮肤

可并行子任务（同一 phase 内）：

| 子任务 | 行为 |
|--------|------|
| 头像 | 现有 `fetch-avatars`（`missing_only`）；入队本批/全库缺图角色 |
| 皮肤 | 按 `skins_json` `_upsert_skins_from_slots` + `_delete_skins_not_in`；无 `skins_json` 则抓取缺页（复用 parse CLI，同时写 `skins_json`）或 legacy assets 并清理脏皮 |
| 模型绑定 | 遍历解包目录：将 `pet_model_id` / `kanmusu_dir` **匹配挂到**已有权威皮（按 ordinal / 标签），**禁止** `upsert_skin(id=folder)` 新建 |

完成判据：皮肤替换对本批角色已执行；头像 job 已结束或与编排合流计入本 phase 完成（允许部分头像 fail，不阻塞台词）。

### Phase 3 — 台词

1. Wiki 缺 `lines_by_skin_json`（或无 `skins_json`）→ 先抓取（现 `wiki_lines_fetch`）  
2. 再按皮匹配写入 roster `skin_lines` + `meta_json.lines_import`  
3. **仅**在该角色 Phase 2 皮肤已对齐后执行  

完成判据：本批尝试完毕；回执计数 `skins_lines_ok` / `empty` / unmatched（仅报告，不写错皮）。

## Job / API

| API | 作用 |
|-----|------|
| `POST /api/roster/ops/wiki-pipeline` | 启动编排；body: `{ force?: bool }`；返回 `{ ok, job_id }` |
| `GET /api/jobs/{id}` | `phase`: `characters` \| `avatars_skins` \| `lines` \| `done`；进度字段沿用 job_store |
| pause / resume | 阶段边界或当前子任务间隙尊重暂停 |

打开页前端：`maybeStartWikiPipeline()` 替代分别 `maybeStartAvatarFetch` + `maybeStartWikiLinesFetch`。  
**删除** `btn-import-wiki` 及对 `/api/roster/ops/import-wiki` 的 UI 调用。  
服务端 `import-wiki` op 可标 deprecated 或改为内部调用编排（外部 UI 不可见）。

## Toast UI

单一浮层「Wiki 补齐」：

- 标题随 phase 变：`同步角色` → `头像与皮肤` → `导入台词` → `完成`  
- 进度条 + 成功/跳过/失败计数  
- 暂停 / 关闭（关闭仅藏 UI，不取消 job，与头像 toast 一致）

不再并行出两个独立 auto toast（头像、台词抢跑）。

## 与旧设计关系

| 旧 | 新 |
|----|-----|
| 手动「导入 Wiki」 | 删除；逻辑并入 Phase 1–3 |
| 打开页并行头像 + 台词 fetch | 编排列队，台词最后 |
| 台词 fetch 不写 roster | Phase 3 **写入** roster 台词 |
| 解包 `id=folder` 建皮 | 禁止；只绑定 |

## 验收

- [x] 自用库页无「导入 Wiki」按钮  
- [x] 打开自用库自动出现「Wiki 补齐」toast 并跑完三阶段  
- [x] 角色列表出现 Wiki 舰船；皮肤为 TabContainer 权威 id，无 `aijier_3` 类脏皮新建  
- [x] 台词在皮肤阶段之后写入；回执中 `roster_unmatched` 不为解包脏皮主导  
- [x] bundled 不自动联网  

## 实现提示（非规范约束）

优先拆 `roster_db.run_import_wiki` 为可 phase 调用的纯函数，编排放 `wiki_pipeline_jobs.py`；复用 `avatar_jobs` / `wiki_lines_fetch` 作为 Phase 2/3 子步骤。
