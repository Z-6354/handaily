# Wiki 流水线优化（增量 · 进度 · 清脏皮）— 设计

**日期**: 2026-07-15  
**状态**: 已实现（含 Referer 抓取修复；流水线顺序改为先抓 TabContainer 再权威换皮）  
**相关**: `2026-07-15-roster-auto-pipeline-design.md`

## 目标

1. **A 增量**：缺什么补什么；`force` 才全量重跑  
2. **B 进度**：toast 按当前阶段队列 `current/total`  
3. **C 清脏皮**：删除非 `{cid}-default|skinN|oath` 的解包式脏皮（如 `aijier_3`）

## 行为摘要

| 阶段 | 增量规则 |
|------|----------|
| 角色 | 仍快速 upsert（本地 SQLite 成本低） |
| **Wiki 抓取** | 缺 `skins_json` 或 `lines_by_skin_json` 则抓；请求须带 Referer（否则 BWIKI HTTP 567） |
| 皮肤 | **在抓取之后**按 `skins_json` 整角色替换；已对齐可跳过 |
| 头像 | `missing_only`（已有） |
| 台词写入 | 抓取与权威皮对齐之后；仅对需要者写入 |

抓取失败样本写入 job 日志（最多 5 条）。

## 非目标

恢复「导入 Wiki」按钮；bundled 自动联网。
