# hanimport 前端 CSS 分层重组 — 设计

**日期**: 2026-07-16  
**状态**: 已实现  
**来源**: 用户「优化代码」→ 前端 → 去重+视觉 → 方案 B（激进重组）  
**相关**: `2026-07-15-hanimport-apple-style-polish-design.md`、`2026-07-15-hanimport-apple-hub-redesign-design.md`

## 目标

将四页 CSS 收敛为 **base → components → layouts → pages**，消除交叉引用与伪共享文件；顺带统一页头 / 控件语言并向苹果风留白再靠拢一档。**不改业务 API 与 JS 语义**（仅允许对齐 class 的最小 HTML 改动）。

## 分层与文件

| 层 | 文件 | 职责 |
|----|------|------|
| base | `design-system/tokens.css` | 色板、圆角、阴影、间距、motion |
| layout | `shell.css` | body、顶栏、`.page-title` / `.page-sub`、`.muted` |
| components | `components.css` | 按钮（含 primary/ghost/secondary）、表单、`.card`/`.panel`、banner、进度条、空态、表、列表、badge、probe、pager、form、log |
| pages | `pages/hub.css` `pages/unpack.css` `pages/roster.css` `pages/skins.css` | 仅该页布局与专属控件 |

**`<link>` 顺序（每页固定）**: tokens → shell → components → 当前 `pages/*.css`（一页一个）。

## 迁移动作

- 删除根级伪共享 `style.css`；解包专属 → `pages/unpack.css`
- `hub.css` → `pages/hub.css`
- `roster.css` 中跨页块（表 / item-list / panel / pager / form / `#log`）→ `components.css`；专属留 `pages/roster.css`
- `skins.css` → `pages/skins.css`；**skins 不再引用 roster**
- 页头统一用 shell 的 `.page-title` + `.page-sub`（HTML 改 class）
- `serve_web.py` 静态路由改指向 `pages/*`；旧路径可删或 404（无外部缓存要求）

## 视觉对齐（轻量）

- 页头下边距略增；surface radius / 字距继续用既有 token
- `entry-card` 等 hub 专属圆角可改为 `--radius-lg` 或新增 `--radius-xl`，避免硬编码分叉
- 不改色板主色，不引入暗色模式

## 非目标

SPA、构建打包器、改 pipeline/API、暗色模式、JS 大重构。

## 成功标准

1. 四页 page CSS **互不交叉引用**
2. 根目录无 `style.css` / 旧页级 css
3. skins 仅依赖 tokens+shell+components+`pages/skins.css`
4. 四页控件与页头视觉一致；浏览器目视回归 hub / unpack / roster / skins

## Spec 自检

- 无 TBD / 占位符
- 与「不改 API」无冲突
- 范围不含后端脚本优化
