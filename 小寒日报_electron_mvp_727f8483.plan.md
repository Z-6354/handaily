---
name: 小寒日报 — 项目状态与路线图
overview: 小寒日报（xiaohan-daily）是一个基于 Tauri 2 + Rust + React 的 Windows 活动追踪与 AI 日报工具。Rust 核心通过 Win32 API 后台采集前台应用/窗口标题/音频/输入/文件活动，SQLite 本地持久化，Web 前端提供今日概览、时间线、AI 时段总结、密码本、AI 人设与桌宠。MVP 已交付，当前处于 Phase 1.5（功能深化）阶段。
todos:
  - AI 日报 Markdown 生成与导出
  - activity_insights 独立展示页
  - 小时活动分布图 UI
  - 浏览器 URL 追踪
  - 桌宠 Spine 动画资源替换
  - 首次打开再创建 main WebView（节省常驻内存）
isProject: false
---

# 小寒日报 — 项目状态与路线图

> **最后更新**：2026-07-03
> **原计划**：MVP（2026-07-02 完成）→ 当前 Phase 1.5（功能深化 + 质量加固）

---

## 项目概览

小寒日报是一个 Windows 本地活动追踪工具，在后台自动记录你每天在电脑上做了什么，结合 AI 分析生成日报级别的时段总结、应用归类和自然语言描述。所有数据存于本地 SQLite，不出本机。

| 项 | 值 |
|----|-----|
| 技术栈 | Tauri 2 + Rust 核心 + React 19 (TypeScript) 前端 + PixiJS/Spine 桌宠 |
| 平台 | 仅 Windows（依赖 WebView2 + Win32 API） |
| 数据路径 | `%AppData%/xiaohan-daily/data/xiaohan.sqlite` |
| 包名 | `com.hanagent.xiaohan-daily` |
| 架构模型 | 双壳（main WebView + pet WebView）+ Rust 核心独立运行 |
| 当前版本 | 0.1.0 |

---

## MVP 交付清单（全部完成 ✅）

以下为原 MVP 计划中的所有项目，已于 2026-07-02 完成：

- ✅ Tauri 2 骨架：Rust 核心 + React 前端 + Vite 构建
- ✅ Win32 前台采集：`GetForegroundWindow` + `GetLastInputInfo`，2s 采样间隔，90s 空闲阈值
- ✅ SQLite 数据模型：`activity_segments` + `app_settings`，WAL 模式，启动兜底孤儿段
- ✅ 并发状态模型：`Arc<AppState>` + `Mutex<Connection>` + `TodayAggregator` 内存缓存
- ✅ 短片段延迟 flush：<2s 同应用合并，不丢失时长
- ✅ 跨日切换：午夜自动闭合 + 重建聚合器
- ✅ 退出安全 flush：`stop_flag` + 后台线程显式 flush + `wal_checkpoint`
- ✅ UWP 应用处理：`package_full_name` 聚合键
- ✅ 系统托盘：最小化到托盘、暂停采集、退出
- ✅ IPC 契约（32 个命令 vs 原计划 10 个）
- ✅ React 三页 UI：今日概览、时间线、设置
- ✅ Rust 单元测试
- ✅ 模块文档与索引

---

## Phase 1.5 — 已完成功能（超越 MVP）

以下功能在原 MVP 计划中列为「不做 / Phase 2」，但已在开发过程中实现：

### AI 供应商适配（`src-tauri/src/ai/`）
- 多供应商支持：OpenAI、Ollama、火山引擎、Agnes AI、OpenCode GO
- 模型目录 + 远程导入 + JSON 配置化适配器工厂
- 文本模型与多模态（视觉）模型分离
- AI 运行时管理（`runtime.rs`）与响应解析

### 混合语义分析（`src-tauri/src/analysis/`）
- 文本规则优先 → 低置信度触发截图 + 视觉 AI 分析
- CPU 守卫（防止 AI 调用过频）
- 分析协调器（`coordinator.rs`）+ 文本分析 + 视觉分析
- 时段 AI 总结（`period.rs` + `period_scheduler.rs`）：5min / 整点 / 长会话 三级触发

### 人设系统（`src-tauri/src/persona/` + `persona_builder/`）
- 人设 Markdown 定义 + 角色资料
- 人设工坊：导入、文本非结构化导入、Skill 生成
- 思考模型支持
- 人设数据库（`character_profiles`）

### 密码本（`src-tauri/src/vault/`）
- AES-GCM 加密存储 API 密钥
- 密码本面板（`VaultPanel.tsx`）：条目 CRUD、名称/密钥/网址

### 桌宠（`src-tauri/src/pet/` + `src/pet/`）
- Spine 骨骼动画 + PixiJS 渲染
- AI 气泡弹窗（从时段总结/时间线描述触发）
- 右键菜单：设置、隐藏
- 点击交互 + nudge 手势
- 省电模式 + 缩放设置
- 全屏应用自动隐藏

### 时间线增强（`src-tauri/src/timeline/`）
- AI 简介（`describe.rs`）：自然语言描述每个时间段
- 上下文丰富注入（`context_enrich.rs`）：从 Cursor/Edge 等应用中提取项目/页面上下文
- 同应用不同内容拆分（项目级/页面级）
- JSON 日志落盘（`json_log.rs`）

### 音频检测（`src-tauri/src/tracker/audio_monitor.rs` + `audio_classify.rs`）
- 后台音频会话检测（听歌/视频/聊天）

### 输入与文件统计（`src-tauri/src/tracker/input_monitor.rs` + `file_watcher.rs`）
- 键鼠 Hook（Win32 `SetWindowsHookExW`）
- 目录文件变更监视（notify）
- 日指标表（`daily_metrics`）

### 其他
- 应用显示名规范化（`display_name.rs`）
- 应用图标提取（`icon.rs`）
- 窗口标题解析（`title_parse.rs`）
- 追踪会话管理（`sessions.rs`）
- 工作类型自定义（`work_type/`）
- 三日热力图（`HeatmapStrip`）
- 双壳架构：main + pet 两个 WebView，Rust 核心独立运行

---

## 当前模块全景

### Rust 核心模块（36 个源文件）

```
src-tauri/src/
├── main.rs              # 进程入口
├── lib.rs               # Tauri Builder + setup（线程启动、托盘、窗口）
├── state.rs             # AppState 定义 + 崩溃恢复
├── tray.rs              # 系统托盘
├── ai/                  # AI 供应商适配层
│   ├── mod.rs
│   ├── adapter.rs       # 适配器工厂
│   ├── config.rs        # 供应商配置
│   ├── providers.rs     # 供应商注册表
│   ├── catalog.rs       # 模型目录
│   ├── runtime.rs       # 运行时管理
│   ├── response.rs      # 响应解析
│   ├── urls.rs          # API URL 构建
│   ├── json_util.rs     # JSON 工具
│   └── adapters/
│       ├── openai.rs
│       └── ollama.rs
├── analysis/            # 混合语义分析
│   ├── mod.rs
│   ├── coordinator.rs   # 分析协调器（队列 + worker）
│   ├── text.rs          # 文本规则分析
│   ├── vision.rs        # 截图 + 视觉 AI
│   ├── guard.rs         # CPU/频率守卫
│   ├── period.rs        # 时段 AI 总结
│   └── period_scheduler.rs  # 时段触发调度
├── db/                  # SQLite 数据层
│   ├── mod.rs           # 连接 + migrate
│   ├── stats.rs         # 聚合查询 + TodayAggregator
│   ├── sessions.rs      # 追踪会话
│   ├── periods.rs       # 时段总结
│   ├── reports.rs       # 日报/报告
│   ├── metrics.rs       # 日指标
│   ├── insights.rs      # 活动洞察
│   ├── timeline_cache.rs
│   └── character_profiles.rs
├── ipc/
│   ├── mod.rs
│   └── commands.rs      # 32 个 Tauri command
├── pet/
│   ├── mod.rs           # 桌宠窗口管理
│   └── models.rs        # 桌宠数据模型
├── persona/             # 人设管理
│   └── mod.rs
├── persona_builder/     # 人设工坊
│   └── mod.rs
├── prompts/             # 提示词模板
│   └── mod.rs
├── screenshot/          # 截图采集
│   └── mod.rs
├── timeline/            # 时间线处理
│   ├── mod.rs
│   ├── json_log.rs      # JSON 日志
│   ├── scheduler.rs     # 时间线调度
│   └── describe.rs      # AI 时间线描述
├── tracker/             # 前台活动采集
│   ├── mod.rs
│   ├── win32.rs         # Win32 前台窗口
│   ├── idle.rs          # 空闲检测
│   ├── poller.rs        # 采样线程
│   ├── writer.rs        # 写入 + 聚合器
│   ├── activity_key.rs  # 聚合键
│   ├── audio_monitor.rs # 音频检测
│   ├── audio_classify.rs
│   ├── context_enrich.rs # 上下文丰富
│   ├── display_name.rs  # 显示名规范化
│   ├── file_watcher.rs  # 文件监视
│   ├── icon.rs          # 图标提取
│   ├── input_monitor.rs # 输入统计
│   └── title_parse.rs   # 标题解析
├── vault/               # 密码本
│   └── mod.rs
└── work_type/           # 工作类型
    └── mod.rs
```

### React 前端

```
src/
├── main.tsx
├── App.tsx              # 根组件（轮询、路由、全局状态）
├── styles.css
├── components/          # 16 个组件
├── pages/
│   ├── TodayDashboard.tsx   # 今日工作（概览+热力图+排行+时段）
│   ├── TimelineView.tsx     # 工作时间线（AI 简介+筛选+分页）
│   ├── VaultPanel.tsx       # 密码本
│   └── SettingsPanel.tsx    # 设置（AI+类型+阈值+采集）
├── lib/
│   ├── xiaohan.ts       # IPC invoke 封装（32 个方法）
│   └── apiErrorMessage.ts
└── pet/                 # 桌宠（独立入口）
    ├── main.ts
    ├── spinePet.ts      # Spine 动画引擎
    ├── skeletonBinary36.ts
    └── viewerExConfig.ts
```

### 后台线程（5 条）

| 线程 | 模块 | 周期/触发 |
|------|------|-----------|
| 采样 poller | `tracker/poller.rs` | 2s |
| 输入 Hook | `tracker/input_monitor.rs` | 事件驱动 |
| 文件监视 | `tracker/file_watcher.rs` | notify |
| 语义分析 worker | `analysis/coordinator.rs` | segment 闭合 |
| 时段 AI worker | `analysis/period_scheduler.rs` | 5min / 整点 / 长会话 |

---

## SQLite 数据模型（当前）

```sql
-- 核心：活动片段
CREATE TABLE activity_segments (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  started_at TEXT NOT NULL,
  ended_at TEXT,
  duration_ms INTEGER NOT NULL DEFAULT 0,
  app_name TEXT NOT NULL,
  exe_path TEXT NOT NULL,
  window_title TEXT NOT NULL DEFAULT '',
  is_idle INTEGER NOT NULL DEFAULT 0,
  aggregation_key TEXT NOT NULL
);

-- 日指标
CREATE TABLE daily_metrics (
  date TEXT PRIMARY KEY,
  key_presses INTEGER DEFAULT 0,
  mouse_clicks INTEGER DEFAULT 0,
  mouse_distance INTEGER DEFAULT 0,
  file_changes INTEGER DEFAULT 0
);

-- 追踪会话
CREATE TABLE tracking_sessions (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  started_at TEXT NOT NULL,
  ended_at TEXT,
  reason TEXT
);

-- 时段 AI 总结
CREATE TABLE period_summaries (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  date_key TEXT NOT NULL,
  hour INTEGER NOT NULL,
  summary TEXT,
  work_types TEXT
);

-- 活动洞察（文本/视觉分析结果）
CREATE TABLE activity_insights (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  segment_id INTEGER,
  insight_type TEXT,
  content TEXT,
  confidence REAL
);

-- 密码本配置
CREATE TABLE vault_config (...);
CREATE TABLE vault_entries (...);

-- 人设
CREATE TABLE character_profiles (...);

-- 设置
CREATE TABLE app_settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
```

---

## 审计与质量加固（2026-07-03）

详细的审计报告见 `docs/questions/54-项目隐患与优化审计-20260703.md`。以下为已修复项：

### 已修复 — 高优先级
- H1: 截图分析持锁调 AI → 释放 DB 锁后再调 AI
- H2: 聚合器与 DB 不一致 → 先写 DB 再更新聚合器，失败回滚
- H3: 午夜整点时段日期错误 → `prev_hour` 时 `date_key` 回退到昨天
- H4: 时间线后台自动 AI → 仅在 tab active 时触发
- H5: 桌宠 reload 并发 → 滑块 debounce 保存
- H6: 全局 refresh 竞态 → 添加 generation 令牌

### 已修复 — 中优先级
- M1: DB 锁毒化恢复不一致 → 统一 `lock_conn()` 恢复路径
- M2: `timeline_describe` 无并发锁 → 添加 in-flight 标志
- M3: 分析队列满静默丢任务 → 添加日志
- M4: 崩溃恢复段时长归零 → 保留 checkpoint 时长
- M5: 每次同步 AI 新建 Tokio runtime → 复用
- M6: 时间过滤器仅筛当前页 → 全量过滤
- M7: 无用 metrics 轮询 → 移除
- M8: 桌宠设置乐观更新无回滚 → 添加回滚
- M9: 输入钩子失败无提示 → 添加日志

### 已修复 — 低优先级
- L1–L5 全部完成

### 桌宠稳定性修复链（2026-07-03）
- 桌宠不可见 → dev 模式 ACL → 竞态条件 → 窗口透明 → 坐标负值 → 根因回归分析
- 详见 `docs/questions/61-68`

---

## 待完成（Phase 2）

| # | 任务 | 优先级 | 说明 |
|---|------|--------|------|
| 1 | **AI 日报 Markdown 生成与导出** | 高 | 从时段总结 + 时间线描述拼装日报，支持复制/导出 |
| 2 | **activity_insights 独立展示页** | 中 | 当前洞察仅在后端存储，前端无独立浏览入口 |
| 3 | **小时活动分布图 UI** | 中 | `stats_hourly_activity` 已有后端，缺前端图表 |
| 4 | **浏览器 URL 追踪** | 中 | 从浏览器窗口标题/URL 提取具体浏览内容 |
| 5 | **桌宠 Spine 动画资源替换** | 低 | 当前使用占位 Spine 骨骼，需替换为正式角色动画 |
| 6 | **首次打开再创建 main WebView** | 低 | 当前 release 启动即创建 main 窗口（隐藏），可优化为按需创建以节省常驻内存 ~20MB |
| 7 | **开机自启** | 低 | 注册 Windows 启动项 |
| 8 | **Markdown 导出** | 低 | 日报/时间线导出为 .md 文件 |

---

## 技术备忘

### 双壳架构
- Rust 核心在进程启动即运行，与 WebView 窗口生命周期解耦
- main WebView：报表 UI（可隐藏/关闭，不影响采集）
- pet WebView：桌宠 UI（可选，独立创建/销毁）
- 托盘为进程锚点，只有「退出」才停止核心线程

### 并发模型
- `Arc<AppState>` 在 `setup()` 创建，`app.manage()` + 后台线程 clone
- `Mutex<Connection>` 单连接串行 + WAL 模式
- command 只读查询用 `async fn` + 临界区 clone 快照
- 退出流程：`ExitRequested` → `stop_flag` → 后台线程 flush → `handle.join()`

### IPC 命令（32 个）
命名遵循 Rust snake_case（`stats_today_overview`），前端 `xiaohan.ts` 暴露 camelCase。分为以下组：
- `app_*`：应用探活
- `tracking_*`：采集控制与状态
- `stats_*`：统计数据查询（overview / breakdown / hourly / timeline / metrics / heatmap）
- `settings_*`：设置读写
- `ai_*`：AI 供应商与模型管理
- `vault_*`：密码本 CRUD
- `analysis_*`：语义分析触发与查询
- `period_*`：时段总结查询
- `timeline_*`：时间线 AI 描述
- `work_types_*`：工作类型管理
- `persona_*`：人设管理
- `pet_*`：桌宠控制
- `screenshot_*`：截图采集

### 开发命令
- `npm run tauri:dev` — 开发模式（main 自动显示）
- `npm run tauri:build` — 生产构建
- `scripts/start-dev.bat` — 开发启动脚本
- `scripts/build.bat` — 构建脚本

---

## 与原 MVP 计划的偏差总结

| 维度 | 原计划（2026-07-02 初始） | 当前状态 |
|------|--------------------------|----------|
| Rust 源文件 | ~12–15 个 | 36 个 |
| 前端文件 | ~10–12 个 | 25+ 个（含 pet） |
| IPC 命令 | 10 个 | 32 个 |
| DB 表 | 2 个（segments + settings） | 10+ 个 |
| 后台线程 | 1 条（poller） | 5 条 |
| 前端页面 | 3 页 | 4 页 + 桌宠独立窗口 |
| AI 集成 | 无（Phase 2） | 多供应商 + 混合分析 + 时段总结 + 人设 |
| 桌宠 | 无 | Spine 动画 + AI 气泡 + 交互 |
| 密码本 | 无 | AES-GCM 加密存储 |
| 音频检测 | 无 | 后台音频会话检测 |
| 输入统计 | 无 | 键鼠 Hook + 文件监视 |
| 架构 | 单窗口 | 双壳（main + pet）+ 核心独立 |
