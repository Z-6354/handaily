# Wiki 流水线优化（增量 · 进度 · 清脏皮）— 设计

**日期**: 2026-07-15  
**状态**: 已批准（用户选全部 A+B+C · 开始实现）  
**相关**: `2026-07-15-roster-auto-pipeline-design.md`

## 目标

1. **A 增量**：缺什么补什么；`force` 才全量重跑  
2. **B 进度**：toast 按当前阶段队列 `current/total`  
3. **C 清脏皮**：删除非 `{cid}-default|skinN|oath` 的解包式脏皮（如 `aijier_3`）

## 行为摘要

| 阶段 | 增量规则 |
|------|----------|
| 角色 | 仍快速 upsert（本地 SQLite 成本低） |
| 皮肤 | 权威 keep 已与 `skins_json` 一致则跳过该角色 |
| 头像 | `missing_only`（已有） |
| 台词抓取 | 缺 wiki 分组则抓（已有） |
| 台词写入 | 仅对「刚抓取成功」或「本地仍有 empty/unmatched 皮」的角色 |

Phase 2 开始前：`purge_folder_like_skins` 删除 `id` 不以 `{cid}-` 为前缀的皮肤行。

进度：`total`/`current` 绑阶段队列长度；`phase` + `current_item` 不变。

## 非目标

恢复「导入 Wiki」按钮；bundled 自动联网。
