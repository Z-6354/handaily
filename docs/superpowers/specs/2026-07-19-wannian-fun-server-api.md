# wannian.fun — 皮肤分发与客户端更新 · 服务器实现规格

**日期**: 2026-07-19  
**状态**: 待用户审阅 · **供服务器侧 AI 直接按本文实现**  
**配套总设计**: `docs/superpowers/specs/2026-07-19-skin-slot-distribution-design.md`  
**域名**: `https://wannian.fun`（假设已上 HTTPS）  
**主机**: 阿里云约 2 核 2G — **禁止解开 `.slot.zip` 内模型重压**；打包用 STORED/低压缩 + 异步队列  

> 本仓不实现服务器代码。客户端（hanimport / hanpet）将严格按本文 API 联调。

---

## 0. 实现顺序提示

总项目顺序是：本地打包走通 → **再部署本服务器** → 再完善 hanpet。  
服务器可先实现「索引 + 上传 + 单文件下载」，再实现「多选打包任务」与「客户端 releases」。

---

## 1. 目录布局（建议）

```text
/var/handaily/   # 或等价数据根；勿放系统盘紧张分区
├── config/
│   ├── upload_token          # 开发者上传 Token（文件权限 600）
│   └── download_signing_secret
├── data/
│   ├── index.sqlite          # 或等价 DB
│   ├── slots/
│   │   └── {character_id}__{skin_id}.slot.zip
│   ├── avatars/
│   │   └── {character_id}.webp
│   ├── pack_jobs/
│   │   └── {job_id}/
│   │       ├── status.json
│   │       └── out.zip       # 多选外层包；TTL 后删除
│   └── releases/
│       └── hanpet/
│           ├── version.json
│           └── {version}/
│               └── handaily-setup.zip   # 或平台相关产物名
└── logs/
```

---

## 2. 数据模型（SQLite 示意）

### 2.1 `characters`

| 列 | 类型 | 说明 |
|----|------|------|
| id | TEXT PK | 与包内 `character.id` 一致 |
| name_zh | TEXT | |
| name_en | TEXT | |
| faction | TEXT | |
| wiki_title | TEXT | |
| avatar_path | TEXT | 相对 `avatars/` |
| updated_at | TEXT ISO8601 | 任意槽覆盖上传时刷新 |
| download_count_cached | INTEGER | 可选缓存 = Σ slots.download_count |

### 2.2 `slots`

| 列 | 类型 | 说明 |
|----|------|------|
| skin_id | TEXT PK | |
| character_id | TEXT FK | |
| name_zh | TEXT | 皮肤名 |
| is_default | INT | |
| is_oath | INT | |
| has_pet | INT | |
| has_kanmusu | INT | |
| pet_model_id | TEXT | |
| kanmusu_dir | TEXT | |
| file_name | TEXT | 磁盘上的 `.slot.zip` 名 |
| file_size | INTEGER | |
| submitted_at | TEXT | 上传/覆盖时间 |
| download_count | INTEGER | **仅子包成功下载 +1** |
| manifest_json | TEXT | 上传时的 manifest 备份（便于展示，勿靠解压） |

唯一文件路径：`data/slots/{character_id}__{skin_id}.slot.zip`（覆盖即替换文件并更新行）。

### 2.3 `pack_jobs`（多选打包）

| 列 | 说明 |
|----|------|
| id | job id |
| status | `queued` / `running` / `done` / `failed` / `expired` |
| skin_ids_json | 请求的子包列表（≤100） |
| out_path | 外层 zip 路径 |
| display_name | 下载文件名（中文） |
| error | |
| created_at / finished_at / expires_at | |

---

## 3. 鉴权与安全（最低档）

1. **HTTPS only**（HTTP 重定向）。  
2. **上传**：`Authorization: Bearer <UPLOAD_TOKEN>`；Token 与配置文件比对；失败 401。  
3. **下载**：短时签名 URL（HMAC，默认有效期 10–15 分钟；`exp` + `path` + `sig`）；或一次性 ticket。禁止裸永久直链挂论坛。  
4. **限速**：按 IP 限制下载并发与上传速率；打包任务全局同时跑 **1** 个（2c2g）。  
5. **输入**：所有 id/文件名仅允许 `[A-Za-z0-9._-]` 与约定的 `__`；拒绝 `..`、绝对路径、超长名。  
6. **上传体大小上限**：建议单文件可配置（如 512MB），超限 413。  
7. **不解析** zip 内恶意路径以外的内容；若需校验，只读中央目录文件名白名单：`manifest.json`、`avatar.webp`、`lines.json`、`pet/`、`skin/`。  
8. Zip 密码：**默认关闭**；配置预留 `zip_password_enabled` / `zip_password_current` / `zip_password_history[]`，本期可不实现加密逻辑。

**防 DDoS**：应用层限流即可；不承诺硬抗大流量。

---

## 4. HTTP API

Base：`https://wannian.fun/api/v1`

### 4.1 健康检查

`GET /health` → `{ "ok": true }`

### 4.2 上传皮肤槽（覆盖）

`PUT /slots/{character_id}/{skin_id}`  
或 `POST /slots/upload` + 表单字段。

**推荐支持断点续传**：实现 **TUS 1.0**（或分片协议，须在响应中写明选用哪一种，客户端将跟从）。

元数据（multipart 字段或完成回调 JSON）：

- `manifest`：UTF-8 JSON（与包内 `manifest.json` 一致）  
- `avatar`：可选文件  
- 主体：`.slot.zip` 字节流  

服务端行为：

1. 校验 Token、id 字符集、`manifest.skin.has_pet === true`（否则 400）。  
2. **不**解压模型；可选轻量检查 zip 中央目录是否含 `pet/`。  
3. 原子替换 `slots/...slot.zip`；写/更新 `slots` 与 `characters`；保存头像。  
4. 返回：`{ "ok": true, "skin_id": "...", "bytes": N }`。

### 4.3 列表

`GET /characters?q=&faction=&page=&limit=`  
→ 角色摘要：id、name_zh、faction、avatar_url、updated_at、has_pet（任一槽）、has_kanmusu（任一槽）、download_count。

`GET /characters/{id}`  
→ 角色详情 + `slots[]`：name_zh、has_pet、has_kanmusu、submitted_at、download_count、skin_id。

`GET /slots?character_id=&q=&has_pet=&has_kanmusu=&page=&limit=`  
→ 皮肤槽列表（下载页筛选/全选当前页或当前筛选集由前端传 id 列表给打包接口）。

### 4.4 单槽下载

`POST /slots/{skin_id}/download-url` → `{ "url": "https://.../d/...?exp=&sig=", "filename": "....slot.zip" }`

实际 `GET` 签名 URL：流式返回文件；成功结束后 **`download_count += 1`**（注意：仅完整成功；断线取消不计）。

### 4.5 多选打包

`POST /pack-jobs`

```json
{
  "skin_ids": ["id1", "id2", "..."],
  "max": 100
}
```

规则：

- `skin_ids.length` ∈ `[1, 100]`，否则 400。  
- 去重；缺失 id → 400 或跳过缺失并在 job 中记录（**推荐：缺失则 400**，避免静默少包）。  
- 入队；返回 `{ "job_id": "..." }`。

`GET /pack-jobs/{id}` → status、progress、`download_url`（done 时）、`filename`、error。

外层 zip：

- 成员为各个 `.slot.zip` **原样拷贝写入**（STORED）。  
- 可选根级 `catalog-meta.json`：`{ "skin_ids": [...], "created_at": "..." }`。  
- **display filename**：角色中文名最多 3 个，逗号分隔，再加「等{N}个角色导入包.zip」，N = 去重角色数。例：`柴郡，安克雷奇，企业等五个角色导入包.zip`（「五」可用阿拉伯数字 `5` 以降低 i18n 复杂度；**推荐阿拉伯数字**：`…等5个角色导入包.zip`）。

打包完成后下载外层包：每含一个子包，对该子包 **`download_count += 1`**（一次打包下载成功，所有子包各 +1）。

临时文件 TTL：建议 2 小时后删 `out.zip` 并将 job 标 `expired`。

### 4.6 客户端自动更新（不含模型）

`GET /releases/hanpet/version.json`

```json
{
  "version": "0.4.0",
  "url": "https://wannian.fun/api/v1/releases/hanpet/download/0.4.0",
  "sha256": "...",
  "notes": "可选"
}
```

`PUT /releases/hanpet/{version}`（Bearer Token）：开发者上传客户端包并更新 `version.json`（仅当 version 语义更高时成为 current，或显式 `?make_current=1`）。

版本比较：semver（`major.minor.patch`）。

---

## 5. 网页下载站（服务器前端）

最低页面能力：

1. 角色列表 / 搜索 / 阵营筛选；头像、更新时间、阵营、桌宠/舰娘有无、下载总次数。  
2. 角色详情：皮肤槽表（名称、has_pet、has_kanmusu、提交时间、下载次数）；多选、全选**当前筛选结果**、单次提交打包 ≤100。  
3. 单槽下载按钮；打包任务进度与完成下载。  
4. 静态资源走同一签名或同源策略。

UI 美观度不限；先功能完整。

---

## 6. 性能约束（写进实现检查单）

- [ ] 任何 API **不得** `unzip` 出 pet/skin 纹理再 zip  
- [ ] 同时仅 1 个 pack job running  
- [ ] pack 使用 STORED 或压缩等级 0–1  
- [ ] 大文件上传用 TUS/分片，避免一次读入内存  
- [ ] 下载用 sendfile/流式  

---

## 7. 错误码约定

| HTTP | 场景 |
|------|------|
| 400 | 校验失败、无 pet、非法 id、skin_ids 超限 |
| 401 | Token 无效 |
| 404 | 槽/角色/job 不存在 |
| 409 | job 冲突（可选） |
| 413 | 文件过大 |
| 429 | 限流 |
| 500 | 打包失败等 |

错误体：`{ "ok": false, "error": "machine_code", "message": "人类可读" }`。

---

## 8. 联调检查单（给客户端）

1. 用 Token 上传 1 个 fixture `.slot.zip` → 列表可见头像与两布尔。  
2. 再传同 id → 文件与 `submitted_at` 更新。  
3. 申请 download-url → 下载成功 → `download_count` +1。  
4. 选 3 个槽打包 → 外层名含前几个角色名 → 下载后三个子包计数各 +1。  
5. `version.json` 可读；上传更高版本后 hanpet 能检测到。  

---

## 9. 明确不做

- 用户注册系统  
- 解开 slot 换皮/转码  
- 模型打进 hanpet release  
- 启用 zip 密码（仅留配置位）  
- 高防 CDN（除非运维另行加）
