# scripts/ — 项目级脚本

HANDAILY **仓库根**编排脚本。hanpet 应用专属脚本在 `hanpet/scripts/`。

## 开发

| 脚本 | 说明 |
|------|------|
| `start-dev.ps1` / `start-dev.bat` | 启动 `npm run tauri:dev` |
| `restart-dev.ps1` | 停止后重启开发环境 |
| `stop-dev.ps1` | 停止桌宠进程与端口 |
| `start.ps1` / `start.bat` | 运行已构建 exe |
| `start-hantransfer.ps1` / `start-hantransfer.bat` | 启动 `hantransfer-desktop`（开发控制台） |
| `hanagent.ps1` | `xiaohan dev\|build\|pack\|stop` 快捷入口 |

## 构建与 Rust

| 脚本 | 说明 |
|------|------|
| `build.ps1` / `build.bat` | 发布打包 → `release/` |
| `cargo-dev.ps1` | 统一 `cargo check/test/run`（并行、sccache） |
| `check-encoding.ps1` | 检测 PS1/BAT 编码（UTF-16、缺 BOM、换行损坏） |
| `_common.ps1` | 路径、端口、进程清理共享函数 |

## 维护

| 脚本 | 说明 |
|------|------|
| `clean-project.ps1` | 清理 dist、缓存、日志；`-All` 含 cargo clean |
| `clean-stale-target.ps1` | 删除误生成的根目录 `target/` |
| `migrate-live2d.ps1` | `live2d/` → `data/live2d/` 迁移 |

## hanpet/scripts/

应用内脚本：`vite-bundled-pet.ts`、`tauri-before-build.mjs`、`generate-pet-icons.py`、`pet-test-api.mjs` 等。由 `hanpet/package.json` 引用。开发启动请用仓库根 `scripts/start-dev.ps1`。
