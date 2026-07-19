# 桌宠文件夹 ↔ 角色绑定规则

**现行手册**（2026-07-18）。实现主要在：

| 位置 | 职责 |
|------|------|
| `hanimport/scripts/roster/folder_rules.py` | 纯规则：`strip_skin`、META/younv/idol/数字后缀 |
| `hanimport/scripts/roster/aliases.py` | `LIVE2D_ALIASES`、`enrich_alias_map_from_roster` |
| `hanimport/scripts/roster/bind_pipeline.py` | 绑皮扫描 / repair / `cmd_repair_l2d_binds` |
| `hanimport/scripts/roster/db.py` | 兼容 re-export（旧 `import roster.db` 仍可用） |
| `hanimport/scripts/common/path_policy.py` | 路径根、`is_special_pet_folder` |
| `data/wiki/live2d-aliases.json` | 显式「文件夹 → 中文名」别名表 |
| `data/roster/handaily-roster.sqlite` | `skins.pet_model_id` / `kanmusu_dir` |

数据根目录：

| 游戏 AB | 解包根 | 说明 |
|---------|--------|------|
| `AssetBundles/char` | **`data/pet/{slug}/`** | 桌宠 Spine（文件夹名无前缀） |
| `AssetBundles/live2d` | **`data/skin/{slug}/`** | 舰娘 Cubism |
| 传输包 | `data/transfer/.../azurlane/{char\|live2d}/` | hantransfer 收件后再解包 |

绑定判定：**UI「未绑定」= 默认皮肤 `pet_model_id` 为空**。磁盘有文件夹不等于已绑。

---

## 1. 通用绑皮

| 规则 | 说明 |
|------|------|
| 别名优先 | `LIVE2D_ALIASES` + `live2d-aliases.json` → 中文名 → 权威 slug；无别名再处理舰船代号大写 |
| Wiki 皮槽 | **裸 `slug`→默认（原皮也算皮肤）**；**`slug_2`→第一套换装 (Wiki skin1)**；`slug_N`(N≥2)→skin{N-1}；**无 `slug_1`** |
| 已有 Wiki 皮 | **禁止**为多出来的文件夹新建 `*-L2D-*` 孤儿皮 |
| 权威拼音冲突 | `ALIAS_PRIMARY_BY_CN`：如 埃吉尔→`aijiang`，大凤→`dafeng`，可怖→`kubo` |
| 显式别名 | 非常规罗马音 / 联动缩写写入 `data/wiki/live2d-aliases.json` |

`strip_skin` 会从文件夹名剥下后缀（数字 / `hx` / `doa` / `idol` / `younv` / `alter` 等），得到 `(base, suffix)` 再解析角色与皮槽。

---

## 2. 模式规则（代码自动）

### 2.1 META → `{base}_alter`

| 显示名 | 文件夹 |
|--------|--------|
| 声望·META | `shengwang_alter` |
| 伊丽莎白女王·META | `yilishabai_alter`（优先成人设别名，非全拼音） |
| U-556·META | `u556_alter`（去连字符） |

- 去掉尾部 `·META` / `META` 得成人设名，再取成人设权威 slug + `_alter`
- `enrich_alias_map_from_roster` 第二遍写入 `*_alter` → META 中文名

### 2.2 伊+纯数字 → `i{数字}`

| 显示名 | 文件夹 | 备注 |
|--------|--------|------|
| 伊56 | `i56` | **不是**拼音 `yi56` |
| 伊404 | `i404` | |
| 伊势 / 伊吹 | — | 不走本规则 |

### 2.3 U 艇 → `u{数字}`

| 显示名 | 文件夹 |
|--------|--------|
| U-101 | `u101` |
| U-556 | `u556` |

### 2.4 μ兵装 → `{adult}_idol`

| 显示名 | 文件夹 |
|--------|--------|
| 光辉(μ兵装) | `guanghui_idol` |
| 希佩尔海军上将(μ兵装) | `xipeier_idol`（短 id，见 `IDOL_SLUG_BY_ADULT_CN`） |

绑定：`guanghui_idol` → 解析成人设「光辉」→ **改绑到**「光辉(μ兵装)」角色，不绑到成人设。

### 2.5 小舰娘 → `{adult}_younv`

| 显示名 | 文件夹 |
|--------|--------|
| 小企业 | `qiye_younv` |
| 小贝法 | `beierfasite_younv`（昵称→贝尔法斯特） |
| 小埃吉尔 | `aijier_younv`（younv 用 `aijier` 非 primary `aijiang`） |
| 小安克雷奇 | `ankeleiqi_younv` |
| 小欧根 / 小齐柏林 | 见昵称表 |

**硬规则：** `{slug}_younv` **只绑「小XX」角色**（`variant_character_name_for_suffix`），禁止写到成人设默认皮。`npm run roster:repair-l2d` 会清掉成人设上的误绑 younv/idol。

昵称映射（`ADULT_CN_FOR_XIAO_NICK` / `XIAO_DISPLAY_BY_ADULT_CN`）：

| 小 X | 成人设 |
|------|--------|
| 贝法 | 贝尔法斯特 |
| 斯佩 | 斯佩伯爵海军上将 |
| 腓特烈 | 腓特烈大帝 |
| 欧根 | 欧根亲王 |
| 齐柏林 | 齐柏林伯爵 |

### 2.6 DOA 联动后缀

| 文件夹 | 皮槽 |
|--------|------|
| `{slug}_doa` | 该角色默认皮 |
| `{slug}_2_doa` | skin1（与 `_2` 同数字规则） |

仅当角色存在时绑定；`create_missing=False` 时不造孤儿皮。

### 2.7 誓约 → `{base}_h`（仅桌宠）

| 显示名 / 皮槽 | 文件夹 | 说明 |
|---------------|--------|------|
| 誓约（`{cid}-oath`） | `{slug}_h` | 例：安克雷奇 → `ankeleiqi_h` |

- **只有桌宠**：写 `pet_model_id={slug}_h`，`kanmusu_dir` 恒为空（无舰娘 Cubism）
- **只扫 `data/pet`**：`bind_oath_h_pets`；不在 `data/skin` 里找 `*_h`
- `strip_skin` / `resolve_bind_skin_id`：后缀 `h` → `{cid}-oath`
- **网页展示**：默认皮与誓约的舰娘列 — 有资源显示就绪，无则 **「不存在」**（非「未绑定」）；桌宠列仍对所有皮要求绑定

### 2.8 数字 / 常见后缀 → 皮槽

| suffix | 目标 |
|--------|------|
| （空）/ `0` | 默认皮（文件夹=角色名，如 `danfo`） |
| `1` | **无效** — 游戏无 `{slug}_1`；勿映射 |
| 纯数字 `N`（N≥2） | → Wiki skin{N-1}（例：`danfo_2`→skin1） |
| `h` | 誓约皮（见 §2.7） |
| `idol` / `younv` | 变体角色的默认皮（见上） |
| `doa` | 默认皮 |
| `N_idol` / `N_younv` / `N_doa` | 按数字 N 映射 |

桌宠与舰娘共用上述编号：`bind_pet_folder_models` 扫 `data/pet` 写 `pet_model_id`；舰娘仍由 `data/skin` 扫描写 `kanmusu_dir`。

---

## 3. 不录入的特殊资源

`path_policy.is_special_pet_folder` — **绑定与名单均跳过**：

| 类型 | 示例前缀/子串 |
|------|----------------|
| 棋子 | `chess_` |
| Boss / 关卡 | `boss_`、`chongying_m_`、`chongying_u_` |
| 舰队模板 | `bb_` / `ca_` / `cl_` / `cv_` / `dd_` / `ss_` / `idol_`（如 `idol_hangmu`） |
| 单船 / 玩偶 | `danchuan`、`manjuu` |
| 小游戏 / 建装等 | `xiaoyouxi`、`jianzhuang`、`_holo`、`_idom`、`_jiaotang`、`_gulite` |
| 左右拆件 | `bawu_l` / `bawu_r`（完整 `bawu` 仍可绑角色） |

无 Wiki 角色对应的文件夹：**不自动建角色录入**（`create_missing` 场景下也应避免对特殊资源建 stub）。

---

## 4. 显式别名表

文件：`data/wiki/live2d-aliases.json`（`folder_slug` → `中文名`）。

用途：

1. 拼音对不上的缩写（`zaoshen`→女灶神，`kaisa`→朱利奥·凯撒）
2. 异名后缀（`tiancheng_cv`→天城CV，`jiahezhanlie`→加贺BB）
3. 联动固定 id（`hdn101`→涅普顿，`kuangsan`→时崎狂三，`*_tolove`→ToLove 角色等）

新增非常规文件夹时：**先改 JSON 别名，再跑绑皮**；能写成模式规则的优先进 `folder_rules.py`，避免别名表无限膨胀。

---

## 5. 操作清单

```bash
# 网页
npm run hanimport:serve   # /roster 管理台

# CLI
npm run roster:import
npm run roster:repair-l2d
npm run roster:merge-dups
```

绑皮入口：`bind_unpacked_models`（解包回调 / AB 导入 / 手工脚本）。

排查未绑定：

| 文件 | 内容 |
|------|------|
| `data/roster/unbound-characters.txt` | 默认皮仍无 `pet_model_id` 的角色 |
| `data/roster/unbound-match-proposals.md` | 人工确认过的匹配记录（含已实装） |

---

## 6. 代码索引

| 符号 | 文件 |
|------|------|
| `meta_alter_slug` | `roster/aliases.py` |
| `is_meta_display_name` / `yi_num_folder_slug` / `u_boat_folder_slug` | `roster/folder_rules.py` |
| `mu_idol_folder_slug` / `xiao_younv_folder_slug` | `roster/aliases.py` |
| `variant_character_name_for_suffix` | `roster/folder_rules.py` |
| `resolve_bind_skin_id`（doa/idol/younv/h→oath） | `roster/bind_pipeline.py` |
| `bind_oath_h_pets` | `roster/bind_pipeline.py` |
| `enrich_alias_map_from_roster` | `roster/aliases.py` |
| `is_special_pet_folder` | `common/path_policy.py` |
| `default_pet` / `default_skin` | `common/path_policy.py` |

相关测试：`hanimport/tests/test_meta_alter_bind.py`、`test_yi_num_i_folder_bind.py`、`test_doa_folder_bind.py`、`test_variant_folder_bind.py`、`test_special_pet_skip.py`、`test_oath_h_pet_bind.py`。
