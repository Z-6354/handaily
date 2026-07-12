# 小寒日报 · 文档索引

本地活动追踪与日报 — **Tauri 2 + Rust + React**

## 快速入口

| 文档 | 说明 |
|------|------|
| [../README.md](../README.md) | 安装、开发、打包命令 |
| [questions/07-功能板块与代码结构总览-20260702.md](questions/07-功能板块与代码结构总览-20260702.md) | **架构总览**（模块表、线程、页面） |
| [questions/121-src与src-tauri及release构建目录说明-20260706.md](questions/121-src与src-tauri及release构建目录说明-20260706.md) | `src` / `src-tauri` / `target/release` 目录 |
| [questions/122-构建统一release-fast单配置-20260706.md](questions/122-构建统一release-fast单配置-20260706.md) | 打包与 release 配置 |
| [01-项目总览/04-代码规范/03-模块索引.md](01-项目总览/04-代码规范/03-模块索引.md) | 模块 ID 与规格链接 |

## 项目结构

```
HANDAILY/
├── src/                      # 前端：主窗口 + 桌宠页面（Vite → dist/）
├── src-tauri/                # Rust 后端：采集、DB、托盘、桌宠、AI
├── bundled/
│   ├── roster/               # 内置人物、人设、桌宠模型（唯一源）
│   └── prompts/              # AI 提示词模板（build.rs 嵌入）
├── public/app-icon.png       # 主界面图标
├── scripts/                  # 开发/构建/校验脚本
└── docs/questions/           # 技术问答归档（按编号检索）
```

运行时用户数据在 `%AppData%/xiaohan-daily/data/`（人设、模型、提示词副本等）。

## 问答归档

`docs/questions/` 存放开发过程中的技术 Q&A（当前 **123** 条），完整索引见 [questions/README.md](questions/README.md)。

按主题快速跳转：

| 主题 | 代表编号 |
|------|----------|
| 桌宠显示与菜单 | 118–120 |
| 自启动 | 111–115 |
| 构建与打包 | 98–99, 103–106, 121–122 |
| 输入迟滞 / 性能 | 100–102, 105–107 |
| AI / 人设 / Wiki | 85–91, 109–110 |
| 时间线 | 31–36, 47–50 |

## 规格文档

- [07-xiaohan-daily/05-模块规格/](07-xiaohan-daily/05-模块规格/) — 模块级设计草案
