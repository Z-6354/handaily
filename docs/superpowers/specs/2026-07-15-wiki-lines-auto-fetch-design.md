# 按皮台词自动抓取（仿头像补齐）— 设计

**日期**: 2026-07-15  
**状态**: 已实现（并入 auto-pipeline）  
**相关**: 头像 `2026-07-15-hanimport-roster-avatar-grid-design.md`；按皮台词 `2026-07-15-per-skin-wiki-lines-design.md`

## 目标

打开**自用角色库**时后台补齐 Wiki `lines_by_skin_json`（跳过已有），toast 进度可暂停；与头像 job 并列、互不解耦失败。

## 对照头像

| 头像 | 本功能 |
|------|--------|
| `avatar_fetch` + `avatar_jobs` | `wiki_lines_fetch` + `wiki_lines_jobs` |
| 缺本地文件 | Wiki 舰 `lines_by_skin_json` 空 / 无 |
| `POST .../fetch-avatars` | `POST .../fetch-wiki-lines` |
| toast 头像补齐 | toast 台词按皮补齐 |
| `missing_only=true` | 同名默认 |

## 行为

1. 队列：自用库角色的 `wiki_title`/`name_zh`，在 wiki sqlite 尚无非空 `lines_by_skin_json`  
2. 拉取：BWIKI `api.php?action=parse`（同 blhx-wiki）  
3. 解析：复用 `extractShipLinesBySkin`（Node 小脚本）  
4. 写入：更新/插入 ships 的 `lines_json` + `lines_by_skin_json`  
5. **不**自动再跑「导入 Wiki」  
6. 限速约 350ms；暂停/继续走 job_store  

## 非目标

- bundled 库自动联网  
- 自动导入自用库台词表  
- 无 Node 时硬解析（提示失败即可）
