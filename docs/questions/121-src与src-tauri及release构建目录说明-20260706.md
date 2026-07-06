# 121 · src / src-tauri / release / release-fast 目录说明

**日期**：2026-07-06  
**标签**：项目结构、Tauri、Cargo、构建

## 问题

为什么项目里同时有 `src`、`src-tauri`，以及 `release`、`release-fast` 这类目录？

## 结论速览

| 名称 | 性质 | 是否应提交 Git |
|------|------|----------------|
| `src/` | **前端源码**（React + Vite + TS） | 是 |
| `src-tauri/` | **桌面端 Rust 后端**（Tauri） | 是 |
| `target/release/`、`target/release-fast/` | **Rust 编译产物**（Cargo 输出） | 否（已在 `.gitignore`） |

`release` 与 `release-fast` **不是**与 `src` 并列的「业务源码目录」，而是 Cargo 在 `target/` 下按**编译配置（profile）**生成的输出子目录。

## 1. `src` 与 `src-tauri`：Tauri 双端结构

本项目是 **Tauri 2** 应用，官方约定前后端分离：

```
HANDAILY/
├── src/              ← 前端：主窗口 UI、设置页、时间线等（Vite 打包 → dist/）
├── src/pet/          ← 桌宠独立页面（pet.html 入口）
├── src-tauri/        ← 后端：Rust 原生能力（采集、数据库、托盘、自启动等）
│   ├── src/          ← Rust 源码（注意：是 src-tauri 内部的 src，不是根目录 src）
│   ├── Cargo.toml
│   └── tauri.conf.json
├── dist/             ← 前端构建输出（Vite，Tauri 打包时嵌入）
└── package.json      ← npm 脚本串联前后端
```

- `tauri.conf.json` 中 `frontendDist: "../dist"` 指向前端构建结果。
- `vite.config.ts` 将 `src/` 编译到 `dist/`；开发时 `tauri dev` 起 Vite + Rust。
- 根目录 `src/` 与 `src-tauri/src/` **职责不同、互不重复**：前者是 Web UI，后者是系统级 Rust 逻辑。

## 2. `release` 与 `release-fast`：Cargo 编译配置

二者定义在 `src-tauri/Cargo.toml`：

| Profile | 用途 | 特点 |
|---------|------|------|
| `release` | 正式发布（`npm run tauri:build:full`） | `thin LTO`、较低 `codegen-units`，体积小、链接慢 |
| `release-fast` | 日常导出 exe（**默认** `npm run tauri:build`） | 关闭 LTO、`codegen-units=256`，链接明显更快 |
| `release-small` | 可选更小体积 | 全 LTO + `opt-level=s` |

编译后产物路径示例：

```
src-tauri/target/release-fast/xiaohan-daily.exe
src-tauri/target/release-fast/bundle/nsis/小寒日报_0.1.0_x64-setup.exe
```

根目录若存在 `target/`（含 `debug` / `release` / `release-fast`），多为历史误在仓库根执行 `cargo build` 产生；**正确入口**应通过 `npm run tauri:build` 或 `--manifest-path src-tauri/Cargo.toml`。二者均可安全删除以释放磁盘（会重新编译）。

## 3. 与 `dist/` 的区别

| 目录 | 工具 | 内容 |
|------|------|------|
| `dist/` | Vite | 前端 HTML/JS/CSS |
| `target/*` | Cargo | Rust 二进制、依赖缓存、NSIS 安装包 |

## 相关文档

- [103 · 导出 exe 构建加速](./103-导出exe构建加速-20260705.md)
- [115 · 自启动修复需完整 Tauri 打包](./115-自启动修复需完整Tauri打包-20260705.md)
