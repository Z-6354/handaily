# 小寒日报

本地活动追踪与日报 — Tauri 2 + Rust + React

## 快速启动

**前置：** [Rust](https://rustup.rs/) 1.77+ · [Node.js](https://nodejs.org/) 22+ · Windows 10/11

```bash
npm install
scripts\start-dev.bat    # 或 npm run tauri:dev
```

## 打包

```bash
npm run tauri:build          # NSIS 安装包 + exe
npm run tauri:build:exe      # 仅便携 exe
scripts\build.ps1 -ExeOnly
```

产物：`src-tauri\target\release\xiaohan-daily.exe`

日常验证：`npm run check:rust`（仅 lib，最快）· `npm run check:rust:bin`（含主程序链接前检查）· `npm run test:rust`

> 编译慢时：勿并行开多个 `cargo`/`tauri dev`（会抢 build 目录锁）；可选安装 [sccache](https://github.com/mozilla/sccache) 加速重复编译；CLI 工具用 `--release` 运行（如 `roster:export`）。

## 文档

完整架构、模块说明与 120+ 条技术问答见 **[docs/README.md](docs/README.md)**。

## 目录一览

| 路径 | 职责 |
|------|------|
| `src/` | React 前端（主窗口 + 桌宠） |
| `src-tauri/` | Rust 后端（采集、SQLite、托盘、AI） |
| `bundled/` | 内置资源唯一源（roster、prompts、桌宠模型、图标模板） |
| `public/app-icon.png` | 主界面图标（icons 脚本生成） |
| `scripts/` | 开发/构建/校验脚本 |
| `docs/questions/` | 技术问答归档 |

桌宠 Spine 走 npm `@pixi-spine`；模型文件在 `bundled/roster/pet-models/`，经 Vite 插件映射为 `/assets/pet/`（dev）并写入 `dist/`（build）。
