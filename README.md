# 小寒日报

本地活动追踪与日报 — Tauri 2 + Rust + React

## 快速启动

### 前置条件
- [Rust](https://rustup.rs/) 1.77+
- [Node.js](https://nodejs.org/) 22+
- Windows 10/11（WebView2 Runtime 通常已预装）

### 开发

```bash
npm install
# 推荐：自动清理端口、加载 VS 编译环境
scripts\start-dev.bat
# 或
npm run tauri:dev
```

首次编译会下载 Rust 依赖并编译 `rusqlite`（bundled SQLite），约需 3-5 分钟。

### 打包

```bash
# 默认：release-fast + NSIS（比旧版 release 快，推荐日常导出）
npm run tauri:build

# 只要便携 exe、不要安装包（最快；仅改 Rust 时会自动跳过前端）
npm run tauri:build:exe

# 手动强制跳过 / 强制重建前端
$env:SKIP_FE_BUILD=1; npm run tauri:build:exe
$env:FORCE_FE_BUILD=1; npm run tauri:build:exe

# 正式发布（LTO + tsc 严格检查）
npm run tauri:build:full

# 或使用脚本（自动检测，无需 -SkipFe）
scripts\build.ps1 -ExeOnly
```

日常验证优先 `npm run check:rust` 或 `npm run tauri:build:debug`。
需要更小体积时用 `npm run tauri:build:small`。

打包前需：
1. 在 `src-tauri/tauri.conf.json` 中设 `"bundle.active": true`
2. 在 `src-tauri/icons/` 放图标（`32x32.png`、`128x128.png`、`icon.ico`）

## 项目结构

```
src-tauri/src/
├── main.rs          # bin 入口（委托 lib）
├── lib.rs           # Tauri Builder + setup + 退出处理
├── state.rs         # Arc<AppState>：DB + 聚合缓存 + 停机标志
├── tracker/
│   ├── mod.rs       # Snapshot / Segment 类型
│   ├── win32.rs     # GetForegroundWindow + exe_path
│   ├── idle.rs      # GetLastInputInfo
│   ├── poller.rs    # 采样循环 + segment 合并/切分 + 跨日
│   └── writer.rs    # 延迟 flush + 短片段合并 + checkpoint
├── db/
│   ├── mod.rs       # 连接 + migrate + insert/settings
│   └── stats.rs     # TodayAggregator + rebuild + timeline 查询
├── ipc/
│   └── commands.rs  # #[tauri::command] 处理器
└── tray.rs          # 托盘图标 + 菜单 + show/hide

src/
├── App.tsx          # 主布局（侧边栏 + 三页切换）
├── pages/
│   ├── TodayDashboard.tsx  # 概览卡片 + Top5 条形图
│   ├── TimelineView.tsx    # 时间线分页表格
│   └── SettingsPanel.tsx   # 设置（idle 阈值 / 采集开关）
├── lib/xiaohan.ts   # invoke 封装 + formatDuration
└── styles.css       # 浅色日报风格
```
