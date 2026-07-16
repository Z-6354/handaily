# 双轨角色内容库（本地个人 / 自带预览）— 设计

**日期**: 2026-07-14  
**状态**: 已实现（`hanimport/scripts/roster_db.py` + `npm run roster:*`）  

## 职责

| 层级 | 路径 | 给用户？ |
|------|------|----------|
| 本地个人库 | `data/roster/handaily-roster.sqlite`（gitignore） | 否 |
| 自带预览库 | `hanpet/bundled/roster/handaily-roster.sqlite` | 是（allowlist 子集） |
| 导出包 | `roster export-pack --ids …` | 按需 |

禁止整库拷贝个人数据到 bundled；必须白名单 [`data/roster/bundled-allowlist.json`](../../data/roster/bundled-allowlist.json)。

## Schema

见 [`data/roster/schema.sql`](../../data/roster/schema.sql)：`characters` / `skins` / `skin_lines`（含 `audio_url` / `audio_relpath`）。

**皮肤 id 规则**：SQLite `skins.id` 全局唯一。JSON/`manifest` 中的 `default` 在库内存为 `{character_id}-default`，`sync` / `publish` 导出时映射回 `default`。Cubism slug（如 `aidang_2`）原样入库。

## 命令

见 [`hanimport/scripts/roster_db.py`](../../hanimport/scripts/roster_db.py) 与 `npm run roster:*`。
