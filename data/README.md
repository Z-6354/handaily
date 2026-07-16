# data — 工作数据目录

开发期本地数据统一放在此目录，**大文件默认 gitignore**，仅 README 与目录结构入库。

## 子目录

| 路径 | 用途 | 版本管理 |
|------|------|----------|
| `live2d/` | 解包或整理的 Spine 模型源（`<slug>/` 含 .skel/.atlas/.png） | 忽略内容 |
| `wiki/` | BWIKI SQLite（`blhx.sqlite`）及缓存 | 忽略 `*.sqlite` |
| `roster/` | **本地个人**角色库（`handaily-roster.sqlite`）；schema / allowlist 可提交 | 忽略 sqlite 与 audio |
| `import/` | `live2d-plan.json`、staging 导出 | 忽略 `*.json` 计划文件 |
| `transfer/` | hantransfer 收件、历史、临时文件 | 忽略内容 |
| `game/` | 游戏资源路径占位或符号链接说明（可选） | 忽略全部 |

## 环境变量

| 变量 | 默认 |
|------|------|
| `HANDAILY_LIVE2D_PATH` | `<repo>/data/live2d` |
| `HANDAILY_LIVE2D_PLAN` | `<repo>/data/import/live2d-plan.json` |
| `HANDAILY_ROSTER_DB` | `<repo>/data/roster/handaily-roster.sqlite` |
| `BLHX_WIKI_DB_PATH` | `<repo>/data/wiki/blhx.sqlite` |

仍支持旧路径 `仓库根/live2d/`（兼容；本仓库已迁移至 `data/live2d/`）。

## 与 hanpet 运行时数据的区别

| | `data/`（本目录） | `%AppData%/xiaohan-daily/data/` |
|--|-------------------|----------------------------------|
| 用途 | 开发导入、解包、BWIKI 缓存 | hanpet 运行时用户数据 |
| 提交 git | 否（仅结构） | 否 |

## 典型工作流

```bash
# 1. 解包（hanimport，待实现）
hanimport unpack --input <bundle.ab> --output data/live2d

# 2. MCP 匹配 + 生成计划
# blhx_match_live2d → blhx_live2d_import_plan → data/import/live2d-plan.json

# 3. 导入 hanpet
npm run live2d:import -- --plan data/import/live2d-plan.json
```

## 从旧路径迁移

若已有 `仓库根/live2d/`（约 1500+ slug 目录）：

```powershell
npm run migrate:live2d          # 预览
npm run migrate:live2d -- -Apply  # 执行（跳过已存在于 data/live2d/ 的 slug）
```

或手动合并后删除 `live2d/`，或设置 `HANDAILY_LIVE2D_PATH` 指向原目录。
