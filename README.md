# HANDAILY

碧蓝航线工具链 + **小寒桌宠** — 项目级 monorepo。

架构：[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)

## 应用

| 应用 | 路径 | 说明 |
|------|------|------|
| **hanpet** | [hanpet/](hanpet/) | 小寒桌宠（人物皮肤统一管理桌宠 Spine + 舰娘 Cubism） |
| **hanimport** | [hanimport/](hanimport/) | 开发导入器（解包、roster、批量入库） |
| **hantransfer** | [hantransfer/](hantransfer/) | 手机 ↔ PC 传输（可选） |

## 工作数据

[data/](data/) — `live2d/`、`model/`、`wiki/`、`roster/`（本地个人库，内容 gitignore）

## 快速启动

```bash
npm install
npm run tauri:dev
```

桌宠 / 导入器 / 传输的脚本入口一览：[scripts/README.md](scripts/README.md)。导入器网页也可用 `npm run hanimport:serve` 或 `hanimport/启动网页版.bat`。

## 常用命令

| 命令 | 说明 |
|------|------|
| `npm run tauri:dev` | 开发模式 |
| `npm run check:all` | Rust + hanimport + 前端构建检查 |
| `npm run roster:import` | Wiki/内置 → 本地个人 roster 库 |
| `npm run roster:sync` | 本地库 → 本机 AppData（不给用户） |
| `npm run roster:publish` | allowlist → 自带 bundled 预览库 |
| `npm run roster:verify` | 校验本地私有 vs 自带子集 |
| `npm run build:release` | 打包 NSIS → `release/` |
| `npm run hanimport -- --help` | 小寒导入器 |

设计规格：[docs/superpowers/specs/](docs/superpowers/specs/)（统一皮肤、双轨 roster、桌宠 Cubism）。
