# scripts/ — 项目级脚本

HANDAILY **仓库根**编排脚本。hanpet 应用专属脚本在 `hanpet/scripts/`。

## 官方入口（推荐）

| 用途 | 入口 |
|------|------|
| 桌宠开发 | `npm run tauri:dev` · `scripts/start-dev.ps1` · `scripts/start-dev.bat` |
| 桌宠已构建运行 | `scripts/start.ps1` · `scripts/start.bat` |
| 桌宠发布包 | `npm run build:release` · `scripts/build.ps1` · `scripts/build.bat` |
| 导入器网页 | `npm run hanimport:serve` · `hanimport/启动网页版.bat`（根目录 `小寒导入器.bat` 为菜单包装） |
| 传输桌面端 | `npm run hantransfer` · `scripts/start-hantransfer.ps1` · `scripts/start-hantransfer.bat` |
| 快捷通道 | `scripts/hanagent.ps1 xiaohan dev\|build\|pack\|stop` |

应用内另有 `hanpet/scripts/start-dev.bat`，会转发到仓库根 `scripts/start-dev.ps1`；日常请优先用上表根入口。

## 开发

| 脚本 | 说明 |
|------|------|
| `start-dev.ps1` / `start-dev.bat` | 启动 `npm run tauri:dev`（hanpet） |
| `restart-dev.ps1` / `restart-dev.bat` | 停止后重启开发环境 |
| `stop-dev.ps1` | 停止桌宠进程与端口 |
| `start.ps1` / `start.bat` | 运行已构建 exe |
| `start-hantransfer.ps1` / `start-hantransfer.bat` | 启动 `hantransfer-desktop` |
| `hanagent.ps1` | `xiaohan dev\|build\|pack\|stop` 快捷入口 |

## 构建与 Rust

| 脚本 | 说明 |
|------|------|
| `build.ps1` / `build.bat` | 发布打包 → `release/` |
| `build-hanimport-release.ps1` | 打包 hanimport 便携 exe |
| `build-hantransfer-apk.ps1` | 构建 hantransfer Android APK |
| `cargo-dev.ps1` | 统一 `cargo check/test/run`（并行、sccache） |
| `check-encoding.ps1` | 检测 PS1/BAT 编码 |
| `_common.ps1` | 路径、端口、进程清理共享函数 |

## 维护

| 脚本 | 说明 |
|------|------|
| `clean-project.ps1` | 清理 dist、缓存、日志；`-All` 含 cargo clean |
| `clean-stale-target.ps1` | 删除误生成的根目录 `target/` |
| `clean-hantransfer.ps1` | 清理 hantransfer 构建产物 |
| `migrate-live2d.ps1` | `live2d/` → `data/live2d/` 迁移 |
| `allow-hantransfer-firewall.ps1` | Windows 防火墙放行 hantransfer |
| `batch-regenerate-personas.ps1` | 遗留：经 Agent API 批量再生成人设（需桌宠 Agent 端口） |

## hanpet/scripts/

应用内脚本：`vite-bundled-pet.ts`、`tauri-before-build.mjs`、`generate-pet-icons.py`、`pet-test-api.mjs` 等。由 `hanpet/package.json` 引用。
