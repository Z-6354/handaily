# Wiki 流水线再优化（加速 · 续跑 · 验收）— 设计

**日期**: 2026-07-15  
**状态**: 已实现  
**相关**: `2026-07-15-wiki-pipeline-optimize-design.md` · `2026-07-15-roster-auto-pipeline-design.md`

## 目标

1. **A 加速**：Wiki 抓取默认 2 并发（环境变量可调，上限 4），全局限速防 567  
2. **B 续跑**：缺什么补什么；Wiki DB 已有 skins+lines 则跳过；中断后再开只补缺  
3. **C 验收**：跑完自动校验覆盖率 / 皮 id 对齐 / 台词未匹配，写入 job results + 日志

## 行为

| 项 | 规则 |
|----|------|
| 并发 | `BLHX_WIKI_FETCH_CONCURRENCY` 默认 2，clamp 1–4 |
| 限速 | 共享最小间隔（默认 350ms），并发不突破总 QPS |
| 续跑 | `list_missing_line_targets` 已跳过齐套船；pipeline 非 force 附着/重启即续 |
| 验收 | `validation: { ok, skins_json_pct, aligned_pct, unmatched_skins, samples }` |

## 非目标

恢复「导入 Wiki」按钮；改 BWIKI；hanpet 播放；自动 force 重跑。

## 验收

1. 大批量缺页时墙钟时间明显短于纯串行  
2. 二次打开只抓缺失船  
3. done 日志含一行验收摘要；不过关 `validation.ok=false` 且有样例  
