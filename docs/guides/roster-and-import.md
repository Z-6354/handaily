# 导入器 · 角色库 · Wiki

**现行手册**（2026-07-18）。整合人物/导入相关单条中的「怎么用」部分：  
[125](../questions/125-人物性格皮肤模型架构重构-20260707.md)–[143](../questions/143-人物列表与头像继续优化-20260707.md)、[128](../questions/128-碧蓝航线BWIKI舰娘MCP-20260707.md)–[131](../questions/131-live2d文件夹匹配与批量导入-20260707.md)。

架构与数据流以 [ARCHITECTURE.md](../ARCHITECTURE.md) 为准；排障细节查 `docs/questions/`。

---

## 1. 启动导入器网页

```bash
npm run hanimport:serve
# 或双击 hanimport/启动网页版.bat
```

| 路径 | 作用 |
|------|------|
| `/` | Hub |
| `/unpack` | 解包 Job |
| `/roster` | **自用库管理台**（列表 + 抽屉详情 + Wiki/AB 导入） |
| `/skins` | 全库皮肤探针 |
| `/preview-lab` | **四格预览实验**（Spine/Cubism；MCP `hanimport_preview_lab_*` 驱动） |

### `/roster` 管理台

- 路径条：自用库 / live2d / unpacked / wiki db / AB 源
- 列表：`summary=1` 返回 `kanmusu_status` / `pet_status` / `lines_status`（含 `partial`）
- 「更新角色」：`POST /api/roster/ops/wiki-sync`（兼容 `wiki-pipeline`）
- 「导入」：向导 → `POST /api/scan` 预览 → `POST /api/roster/ops/import-ab-bind`

### MCP

| MCP | 职责 |
|-----|------|
| `mcp/blhx-wiki` | 只读 BWIKI 舰娘资料 |
| `mcp/hanimport` | 写自用库 / Wiki 更新 / AB 导入 / 查 Job（需 `npm run hanimport:serve`，默认 `http://127.0.0.1:7821`） |

详见 [mcp/hanimport/README.md](../../mcp/hanimport/README.md)。

---

## 2. 双轨 roster

| 库 | 路径 | 用途 |
|----|------|------|
| 本地个人 | `data/roster/handaily-roster.sqlite` | 开发导入；**不**整文件进安装包 |
| 自带预览 | `hanpet/bundled/roster/` | allowlist 子集；`*.sqlite*` 已 gitignore |

```bash
npm run roster:import      # Wiki/内置 → 本地库（CLI；网页自用库会自动补齐）
npm run roster:sync        # 本地库 → 本机 AppData
npm run roster:publish     # allowlist → bundled
npm run roster:verify
npm run roster:repair-l2d  # 修复 BLHX 文件夹↔Wiki 皮错位 + 清理多余 L2D 孤儿
npm run roster:merge-dups  # 同名角色合并
```

禁止把个人库整文件拷进 `bundled/`。

**安全注意（2026-07-17）**

- Wiki 皮肤同步默认**保留** L2D / 已绑定皮，不会因 Wiki 列表缺项而删掉手工皮。
- 「同步 AppData」默认合并；替换需二次确认（`confirm_replace`）。
- 网页 API 路径限制在 `data/`、`mcp/blhx-wiki/data/`、AppData 相关 env 根等允许目录内。
- Wiki SQLite 默认路径统一走 `path_policy.default_wiki_db()`（`BLHX_WIKI_DB_PATH` → mcp 库 → `data/wiki/blhx.sqlite`）。
- Wiki 补齐流水线同进程单飞（含 force），避免并行写库。
- `/roster` 工具栏「更新角色」走 `POST /api/roster/ops/wiki-sync`（失败时回退 `wiki-pipeline`），进度见页底 toast；完成后列表自动刷新。

---

## 3. 解包 → 绑皮 → 人物（现行流程）

```
游戏 AB
  → hanimport unpack / transfer unpack
  → data/pet/{slug}/        # 桌宠 Spine（char）
  → data/skin/{slug}/       # 舰娘 Cubism（live2d）
  → AB 导入 / Wiki 补齐绑皮
  → data/roster/handaily-roster.sqlite
```

手机传包：`hantransfer` → `data/transfer/<YYYYMMDD-HHMMSS-id>/`（可含 `azurlane/{char|live2d}/`）→ 再解包。  
UI 源码仅改 `hantransfer/mobile-web/`（见 [dev-and-build.md §6](dev-and-build.md)）。

**文件夹命名与变体规则（META / 伊数字 / U艇 / μ / 小舰娘 / DOA / 誓约 `_h` / 特殊资源）见专篇：**  
→ [pet-folder-bind-rules.md](pet-folder-bind-rules.md)

### 3.1 绑皮硬规则（防复现）

| 规则 | 说明 |
|------|------|
| **别名优先于大写** | `normalize_character_id` / `alias_redirect_id`：先查 `LIVE2D_ALIASES` + `data/wiki/live2d-aliases.json`，有别名则走权威 slug（如 `i404`→伊404）；**无别名**才把舰船代号大写（`z23`→`Z23`）。禁止把 `i404` 规范成 `I404` stub。 |
| **Wiki 皮为权威槽位** | 裸 `slug`→默认（原皮）；**`slug_2`→第一套换装 `skin1`**（无 `slug_1`）；`slug_N`(N≥2)→`skin{N-1}`。舰娘/桌宠同编号。已有 Wiki 皮时，**禁止**为多出来的文件夹新建 `*-L2D-*` 孤儿皮。 |
| **誓约仅桌宠 `_h`** | `{slug}_h` → `{cid}-oath`（例：安克雷奇 → `ankeleiqi_h`）。只写 `pet_model_id`，`kanmusu_dir` 恒空；**只扫 `data/pet`**（`bind_oath_h_pets`），不在 `data/skin` 找 `*_h`。详见 [pet-folder-bind-rules.md §2.7](pet-folder-bind-rules.md)。 |
| **默认/誓约舰娘展示** | 桌宠（pet）所有皮都应有；舰娘（skin）对**默认皮与誓约**非必需：有绑定则显示就绪/缺文件，无则显示 **「不存在」**（`absent`），不用「未绑定」。换装皮仍用「未绑定」。 |
| **多余 L2D 清理** | `bind_unpacked_models` 结束自动 `repair_blhx_skin_folder_binds`（含 `prune_unmapped_l2d_orphan_skins`）。手动：`npm run roster:repair-l2d`（含誓约 `_h` 补绑）。 |
| **同名合并** | AB 绑皮后自动 `merge_roster_duplicates_by_name`（ASCII stub 名经别名归到中文名，如 `i404`/`I404`→伊404）。手动：`npm run roster:merge-dups`。 |
| **特殊资源不录入** | `chess_` / `boss_` / `danchuan` 等见 [pet-folder-bind-rules.md §3](pet-folder-bind-rules.md)；`is_special_pet_folder` 跳过绑定。 |
| **预览** | 舰娘 Tab：`kanmusu_engine=spine` 走 Spine；否则 Cubism，失败再回退 Spine。 |

非常规罗马音别名写进 **`data/wiki/live2d-aliases.json`**（代码内 `LIVE2D_ALIASES` 为兜底）。示例：`"i404": "伊404"`。

### 3.2 推荐操作顺序

1. 解包 AB → 确认 `data/pet` / `data/skin` 有目标 slug  
2. `/roster`「更新角色」（Wiki 角色+皮槽）或等自动流水线  
3. 「导入」AB 绑皮（或等解包回调绑皮）  
4. 若皮数异常 / 桌宠未绑：`npm run roster:repair-l2d` 后硬刷新详情页  
5. 同名双开角色：`npm run roster:merge-dups`

### 3.3 四格预览实验（对话 / MCP）

设计：[preview-lab-design](../superpowers/specs/2026-07-18-preview-lab-design.md)。

1. 浏览器打开 `http://127.0.0.1:7821/preview-lab`（需 `npm run hanimport:serve`）  
2. 对话说明要对对照的目录，例如：「对比伊404 第一个皮肤的 live2d 与 unpacked」  
3. Cursor 调用 MCP `hanimport_preview_lab_set`（`root`=`live2d|unpacked`，`rel`=slug）  
4. 页面约 1 秒刷新；每格自动选 Spine 或 Cubism

---

## 4. 台词状态怎么读

| 摘要 | 含义 |
|------|------|
| 台词就绪 | 已按皮写入 |
| Wiki无该皮台词 | Wiki 未给独立面板（常见，非失败） |
| 库皮未对上 / Wiki套未对上 | 标题对不上；不会误写 |

设计规格索引：[docs/README.md](../README.md#2026-07-15-进度已落地)。

---

## 5. hanpet 导入服务器皮肤包

从 `wannian.fun` 下载的「多角色导入包」或单个 `.slot.zip`，在 **人物** 页点 **导入皮肤包**（与旧「导入角色包」并列）。

- 格式：`handaily-skin-slot`（外层可含多个 `*.slot.zip` + `catalog-meta.json`）
- 落盘：便携 `{exe}/data`（或 `HANDAILY_DATA_DIR`）下的 `pet-models/`、`kanmusu-models/`、`characters/`
- 规格：[skin-slot-distribution-design](../superpowers/specs/2026-07-19-skin-slot-distribution-design.md)
