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

日常验证：`npm run check:rust` · `npm run test:rust`

## 文档

完整架构、模块说明与 120+ 条技术问答见 **[docs/README.md](docs/README.md)**。

## 目录一览

| 路径 | 职责 |
|------|------|
| `src/` | React 前端（主窗口 + 桌宠） |
| `src-tauri/` | Rust 后端（采集、SQLite、托盘、AI） |
| `personas/` · `prompts/` | 人设与提示词 |
| `public/assets/pet/` | 内置桌宠模型 |
| `docs/questions/` | 技术问答归档 |
