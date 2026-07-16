# packages/

HANDAILY **项目级**共享 npm / Rust 包。

| 包 | 路径 | 状态 | 消费者 |
|----|------|------|--------|
| `@handaily/blhx-slug-match` | `blhx-slug-match/` | 占位，待从 `live2d.ts` 抽出 | `mcp/blhx-wiki`、`hanimport` |
| `handaily-spine-pack` | （规划 `crates/`） | 未创建 | `hanpet`、`hanimport` |

根 `package.json` 已配置 `workspaces: ["packages/*", "mcp/*"]`。

创建与抽取时机：[docs/plans/2026-07-12-handaily-project-layout.md](../docs/plans/2026-07-12-handaily-project-layout.md) Phase 4。
