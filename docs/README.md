# HANDAILY · 文档索引

项目级 monorepo：**hanpet** + **hanimport** + **data**（+ 可选 hantransfer）

## 快速入口

| 文档 | 说明 |
|------|------|
| [../README.md](../README.md) | 项目总览 |
| [ARCHITECTURE.md](ARCHITECTURE.md) | **当前架构**（含双轨 roster） |
| [superpowers/specs/2026-07-14-unified-character-skins-design.md](superpowers/specs/2026-07-14-unified-character-skins-design.md) | 人物统一皮肤 |
| [superpowers/specs/2026-07-14-dual-roster-database-design.md](superpowers/specs/2026-07-14-dual-roster-database-design.md) | 本地库 / 自带库 |
| [../hanpet/README.md](../hanpet/README.md) | 小寒桌宠应用 |
| [../hanimport/README.md](../hanimport/README.md) | 小寒导入器 |
| [../data/README.md](../data/README.md) | 工作数据目录 |

## 2026-07-15 进度（已落地）

| 能力 | Spec | 说明 |
|------|------|------|
| 解包 Job + 进度 | [unpack-jobs](superpowers/specs/2026-07-15-hanimport-unpack-jobs-design.md) | `/unpack` 异步解包与生成配置 |
| 导入器 Hub | [apple-hub](superpowers/specs/2026-07-15-hanimport-apple-hub-redesign-design.md) | `/` 双入口 + 近期任务 |
| 角色库可视化 | [roster-browser](superpowers/specs/2026-07-15-hanimport-roster-browser-design.md) | `/roster` CRUD；自用/自带切换 |
| 头像网格式列表 | [avatar-grid](superpowers/specs/2026-07-15-hanimport-roster-avatar-grid-design.md) | 卡片网格 + 头像补齐 toast |
| 皮肤双栏探测 | [skin-dual-columns](superpowers/specs/2026-07-15-hanimport-skin-dual-columns-design.md) | Spine / Cubism 就绪状态；`/skins` |
| TabContainer 定皮 | [tabcontainer-skins](superpowers/specs/2026-07-15-wiki-tabcontainer-skins-design.md) | 图鉴 tab 为权威皮肤清单 |
| 按皮 Wiki 台词 | [per-skin-lines](superpowers/specs/2026-07-15-per-skin-wiki-lines-design.md) | 分面板抓取；匹配写入；`lines_status` |
| Wiki 全自动补齐 | [auto-pipeline](superpowers/specs/2026-07-15-roster-auto-pipeline-design.md) | 开自用库：角色→头像/皮肤→台词 |
| 流水线增量优化 | [pipeline-optimize](superpowers/specs/2026-07-15-wiki-pipeline-optimize-design.md) | 跳过已对齐；进度数字；清脏皮 |
| 流水线再优化 v2 | [pipeline-v2](superpowers/specs/2026-07-15-wiki-pipeline-v2-optimize-design.md) | 抓取 2 并发 · 续跑 · 跑完验收 |
| 全站苹果风对齐 | [apple-style-polish](superpowers/specs/2026-07-15-hanimport-apple-style-polish-design.md) | Hub/解包/角色库/皮肤统一 token 与控件 |
| 舰娘整体点击 | [whole-main-touch](superpowers/specs/2026-07-15-kanmusu-whole-main-touch-design.md) | 三区外整模可点 → `main_*` |
| 桌宠/舰娘布局分存 | [companion-layout](superpowers/specs/2026-07-15-companion-layout-split-design.md) | 位置/尺寸/缩放按引擎独立 |

对应实现计划均在 `superpowers/plans/`，进度标记为 **已完成（2026-07-15）**。

### 台词回执怎么读

| 数字 | 含义 |
|------|------|
| 台词就绪 | 已按皮写入 |
| Wiki无该皮台词 | Wiki 未给该换装独立台词面板（常见，非失败） |
| 库皮未对上 / Wiki套未对上 | 标题对不上；**不会误写到错误皮**；用筛选「台词需关注」排查 |

## 已废止 / 已合并

| 旧叙述 | 现状 |
|--------|------|
| 独立「舰娘」侧栏页 | 已移除；入口在 **人物 → 皮肤** |
| Wiki 直接写 AppData 为唯一流水线 | 改为 Wiki → **本地 roster sqlite** → sync / publish |
| 角色库「导入 Wiki」按钮 | 已删除；自用库打开即跑 **Wiki 补齐** 流水线 |

旧计划文件（`plans/2026-07-14-kanmusu-cubism-player.md` 等）中的「舰娘页」以 ARCHITECTURE 为准。

## 问答归档

`docs/questions/` — 历史 Q&A；路径冲突以 [ARCHITECTURE.md](ARCHITECTURE.md) 为准。
