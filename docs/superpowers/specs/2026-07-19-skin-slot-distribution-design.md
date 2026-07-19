# 皮肤分发包 · 下载站 · 客户端导入 — 设计

**日期**: 2026-07-19  
**状态**: 阶段①本地打/解包已实现；②③待做  
**域名**: `wannian.fun`  
**实现顺序（用户指定）**: **① 本地打/解包走通 → ② 服务器 → ③ hanpet 完善**

## 1. 背景与目标

在开发机用 hanimport 将「皮肤槽」（必有桌宠 pet，舰娘 skin 可选）打成自描述 zip，上传到阿里云站；用户在网页勾选下载（单槽 / 多选打包）；hanpet 便携目录导入后落盘并可播（先以桌宠为主，舰娘随包落盘）。

**试跑**: 约 5 个皮肤槽联调；**功能按完整规格实现**（不以砍功能当 MVP）。

### 1.1 非目标（本期）

- hantransfer 打通  
- 人设 persona、音频  
- 启用 zip 密码（**预留配置，默认关**）  
- CDN / 高防；模型打进客户端更新包  
- 扩展旧格式 `handaily-roster-pack`（采用**新格式**，见 §3）

### 1.2 与现有包关系

| 现有 | 本期 |
|------|------|
| hanimport `export-pack`（几乎仅 sqlite） | 不复用 |
| hanpet `handaily-roster-pack`（人格+pet，无舰娘/头像） | **不混用**；新导入器认新格式 |
| hantransfer AB 批次 | 本期不做 |

---

## 2. 架构总览

```
[开发机 data/pet|skin + roster sqlite]
        │ ① 本地：校验 → 打 .slot.zip →（可先本地 round-trip 解包测）
        ▼
[hanimport 上传 API] ──Token──► [wannian.fun]
                                    │ 索引 DB + 原样存 .slot.zip
                                    │ 下载页 / 多选异步打包 / 客户端 releases
                                    ▼
                            [用户浏览器 / 日后下载器]
                                    │ ② 服务器走通
                                    ▼
                            [hanpet 便携目录] ③ 导入 + 自动更新客户端
```

**服务器实现**: 本仓只提供规格 MD（见姊妹文档）；由服务器侧 AI 按 MD 编码。  
**本仓实现**: 包格式库、hanimport 打包装与上传、hanpet 导入与自动更新。

---

## 3. 皮肤分发包格式（`.slot.zip`）

### 3.1 定义

**一个皮肤槽 = 一个上传/计数单元** = `pet` 必有 + `skin` 可选 + 元数据（头像、台词 JSON、角色/槽位字段）。

**内层包文件名（规范）:**

```text
{character_id}__{skin_id}.slot.zip
```

`skin_id` 与 roster 一致（如 `{cid}-default`、`{cid}-oath`、`{cid}-skin1`）。文件名中若含路径分隔符则拒绝。

### 3.2 内层布局（上传时由本地打好；服务器不解开模型）

```text
{character_id}__{skin_id}.slot.zip
├── manifest.json      # 见 §3.3
├── avatar.webp        # 推荐；无则依赖已有库头像（导入可不补）
├── lines.json         # 该槽台词；无台词则为 []
├── pet/{pet_slug}/…   # Spine 树（必有；至少能通过现有 pet 完整性启发式）
└── skin/{skin_slug}/… # Cubism 树（可选；有则整目录打入）
```

`pet_slug` = `skins.pet_model_id`；`skin_slug` = `skins.kanmusu_dir`（可空）。

### 3.3 `manifest.json`（规范字段）

```json
{
  "format": "handaily-skin-slot",
  "format_version": 1,
  "character": {
    "id": "cheshire",
    "name_zh": "柴郡",
    "name_en": "Cheshire",
    "faction": "皇家",
    "wiki_title": "柴郡"
  },
  "skin": {
    "id": "cheshire-default",
    "name_zh": "默认皮肤",
    "is_default": true,
    "is_oath": false,
    "pet_model_id": "xianzun",
    "kanmusu_dir": "xianzun",
    "has_pet": true,
    "has_kanmusu": true
  },
  "lines": { "path": "lines.json" },
  "packed_at": "2026-07-19T02:00:00Z"
}
```

- `has_pet` / `has_kanmusu`：与目录是否存在一致；上传前本地校验。  
- 导入**只信包内 manifest + 文件**，不联网补全。

### 3.4 `lines.json`

数组，元素对齐本地 `skin_lines` 语义（字段可多不可少核心项）：

```json
[
  {
    "wiki_key": "main_1",
    "label": "主界面",
    "lang": "zh",
    "text": "……",
    "animation": "",
    "sort_order": 1
  }
]
```

字段对齐本地 `skin_lines`（`wiki_key/label/lang/text/animation/sort_order`）；**不包含** `audio_url` / `audio_relpath`。空槽：`[]`。

### 3.5 本地打包校验（hanimport）

| 条件 | 行为 |
|------|------|
| 皮肤未绑定（无可用 `pet_model_id`） | **跳过** + 提示 |
| 无 pet 目录 / pet 不完整 | **跳过** + 提示 |
| 仅有 skin、无 pet | **问题，跳过** + 提示 |
| 有 pet，无 skin | **允许**（`has_kanmusu=false`） |
| 同 id 再上传 | 服务器 **覆盖** |

默认皮、誓约皮、换装皮：**同一套**勾选逻辑，不特殊禁止。

---

## 4. 下载外层包与命名

### 4.1 单槽下载

可直接下发已存的 `.slot.zip`（或再套一层仅含该文件 + 可选 `catalog-meta.json`）。**禁止**为换皮而解开重压模型树。

### 4.2 多选打包（服务器）

- 范围：当前筛选结果；**单次最多 100 个皮肤槽**。  
- 实现：异步任务，将已有 `.slot.zip` **原样**打入外层 zip（优先 `ZIP_STORED` / 最低压缩）。  
- 外层内为**单层**多个 `*.slot.zip`（可加一个 `catalog-meta.json`）。  
- 展示文件名：取所选角色中文名 **最多前 3 个**，再加「等 N 位…」：  
  - 例：`柴郡，安克雷奇，企业等5个角色导入包.zip`（**用阿拉伯数字 N**，避免中文数字）  
  - 不足 3 个角色名则只列出实际名称，仍带「等 N 个角色导入包」；N = **去重角色数**。

### 4.3 下载计数

- **仅子包（`.slot.zip`）每次成功下载 +1**（多选包内每个子包各 +1）。  
- 外层 zip 本身不另计「角色下载」。  
- 角色页展示的下载次数 = 其下子包计数之和（可缓存聚合）。

### 4.4 Zip 密码

- **本期默认关闭**。  
- 预留：全局当前密码 + 轮换；客户端存多版本密码列表；仅本项目使用。  
- 规格与 API 留开关字段，实现可后置。

---

## 5. 服务器（摘要；细节见服务器 MD）

- 域名：`wannian.fun`；资源 2 核 2G → 少算力：不拆模型 zip、打包异步、限并发。  
- 数据源：上传附带的索引信息 + 原样文件；头像存服务器角色库供网页展示。  
- 展示：角色（更新时间、是否有桌宠/舰娘、阵营、下载总次数、头像）；皮肤槽一行两布尔 + 名称、提交时间、下载次数。  
- 安全最低档：HTTPS、上传开发者 Token、下载短时签名 URL、限速、路径穿越与注入基础防护。  
- 客户端更新：`releases/hanpet/` + `version.json`；**不含模型**。  
- 交付物：`docs/superpowers/specs/2026-07-19-wannian-fun-server-api.md`。

---

## 6. 本仓 · 阶段 ① 本地（先做）

目标：**不依赖服务器**，打 `.slot.zip` ↔ 解包 round-trip 正确。

| 交付 | 说明 |
|------|------|
| 打包库 | 从 roster sqlite + `data/pet`/`data/skin`/`avatars` 生成 `.slot.zip` |
| CLI 或 hanimport 页内「导出皮肤槽」 | 多选、跳过原因列表、进度 |
| 本地解包校验器 | 解到临时/测试目录，断言 pet/skin/manifest/lines |
| 契约测试 | fixture 小模型；非法槽跳过 |

本地解包落盘约定（与日后 hanpet 对齐）：`{root}/pet/{slug}`、`{root}/skin/{slug}`、角色元数据写入可测试的 sqlite 或 JSON sidecar（阶段 ③ 再接 hanpet 正式库）。

---

## 7. 本仓 · 阶段 ② 对接服务器

- hanimport：分片/断点续传上传（推荐 TUS 或等价）；进度 UI；Token 配置。  
- 联调：上传覆盖、列表字段、单下、多选 ≤100、计数。  
- 服务器按姊妹 MD 部署后联调。

---

## 8. 本仓 · 阶段 ③ hanpet

- **数据根**：便携安装目录（exe 旁），默认不写 C: AppData（本功能路径）；旧数据迁移策略在实现计划中单列。  
- 导入：识别单 `.slot.zip` / 外层多子包；解压合并；台词入本地；不联网。  
- 自动更新：读 `version.json`，更大则下载客户端包并提示安装；模型仍手下。  
- 密码：配置预留，默认关。

---

## 9. 上传协议（阶段 ②）

- 鉴权：单一开发者 Token（环境变量 / 本地配置），不做多用户账号。  
- 传输：HTTPS；**进度 + 断点续传**（大文件必做）。  
- 粒度：每皮肤槽一个 `.slot.zip`。  
- 冲突：覆盖。

---

## 10. 测试与验收

| 阶段 | 验收 |
|------|------|
| ① | 5 个槽打包；跳过规则提示正确；本地解开后 pet 可被现有探针识别；lines/manifest 完整 |
| ② | 上传后网页可见头像与字段；单下/多选；计数按子包；2c2g 打包不拖死（排队） |
| ③ | 便携目录导入后桌宠可用；客户端更新检测链路通 |

---

## 11. 风险

| 风险 | 缓解 |
|------|------|
| 多选 100 大包撑满磁盘 | 任务配额、临时目录 TTL、STORED 压缩 |
| 便携路径与旧 AppData 双根 | 阶段 ③ 明确优先级与迁移提示 |
| Zip 密码后置 | API/配置先留位，避免返工格式 |

---

**审阅通过后**：用 writing-plans 写 `docs/superpowers/plans/2026-07-19-skin-slot-local-pack.md`（仅阶段 ①），再实现。
