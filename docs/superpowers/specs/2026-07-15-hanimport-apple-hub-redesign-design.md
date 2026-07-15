# hanimport 苹果风枢纽重做 — 设计

**日期**: 2026-07-15  
**状态**: 待用户审阅（brainstorm 已逐节批准）  
**来源**: 用户选择 — 大幅重做（C）+ 中心枢纽（1）+ 浅色（A）+ 参考 [apple.com/design](https://www.apple.com/design/)（1）+ 实现方案 A（多页壳层 + 抽 token）

**相关**: 角色库功能语义仍以 `2026-07-15-hanimport-roster-browser-design.md` 为准；解包异步任务以 `2026-07-15-hanimport-unpack-jobs-design.md` 为准。**本文件覆盖二者之上的信息架构与视觉壳**；其中路由表以本文件为准（`/` 改为枢纽）。

## 目标

用 **skillui** 与 **extract-design-system** 从苹果设计站抽取设计原语，收敛为项目 token，重做 hanimport 本地网页：

1. **`/` 中心枢纽**：品牌 Hero、双入口（解包 / 角色库）、环境摘要、最近解包任务  
2. **浅色、大留白、系统无衬线** 的苹果官网气质（非像素复刻）  
3. **多页共享壳层**：概览 · 解包 · 角色库；业务 API 语义基本不动  

## 非目标

- SPA / 客户端路由框架  
- 鉴权、多用户、发行版打包进 hanpet UI  
- 第三方组件库、营销视频背景、重玻璃拟态  
- 改角色库 CRUD / Wiki 导入 / 同步语义；改解包 job 执行语义  
- 像素级复刻 Apple 站点或 SF 字体商用授权之外的资源绑定（使用系统字体栈）

## 实现路线（已选 A）

1. 对 `https://www.apple.com/design/` 运行抽取（`skillui --url …`、`npx extract-design-system …`）  
2. 人工收敛 → `hanimport/web/design-system/tokens.css`（及可选 `tokens.json`）  
3. 共享 `shell.css` + 顶栏；枢纽 / 解包 / 角色库分页  
4. `serve_web.py` 更新静态路由；可选 `GET /api/jobs` 供枢纽「最近任务」  

## 路由与信息架构

| 路径 | 职责 |
|------|------|
| `/` | 枢纽首页（新建） |
| `/unpack` | 解包工作台（现有解包 UI 迁入） |
| `/roster` | 角色库（功能保留，换壳 + 浅色） |

**共享顶栏（三页一致）**

- 左：产品名「小寒导入器」  
- 中：导航 — 概览 · 解包 · 角色库（当前页高亮）  
- 右：环境状态点（OK / 警告 / 错误），点进枢纽看详情  

**库切换（自用 / 自带）** 仍只在 `/roster` 页头副行，不进全局导航。

### 枢纽首页块（自上而下）

1. Hero：产品名 + 一句话用途  
2. 双入口大卡片：「解包模型」「管理角色库」→ `/unpack`、`/roster`  
3. 环境摘要：复用 `/api/status`  
4. 最近解包任务：列表（进行中 / 完成 / 失败），链到 `/unpack?job=<id>`  

窄屏：双入口上下堆叠。

### `/unpack`

保留扫描、选项、进度、扫描结果、日志；布局为主操作 surface + 结果区。  
`?job=<id>`：自动展示该 job 进度/日志；任务不存在则横幅提示，表单仍可用。

### `/roster`

保留三栏（列表 · 角色/皮肤 · 台词）。  
操作条：主 CTA 1～2 个，其余次要样式或「更多」。  
窄屏：列表全宽 → 详情 → 皮肤/台词折叠。  
写自带库二次确认与警告标签保留。

## 视觉与 token

**参考站**: https://www.apple.com/design/  

**抽取**: 实现阶段执行 skillui + extract-design-system；结果仅作启动参考，须人工收敛，**禁止**未经确认整站覆盖现有样式。

### 目标色板

| Token | 用途 | 方向 |
|------|------|------|
| `--bg` | 页底 | `#f5f5f7` |
| `--surface` | 卡片/面板 | `#ffffff` |
| `--text` | 主文 | `#1d1d1f` |
| `--muted` | 次要 | `#6e6e73` |
| `--hairline` | 分割 | 半透明黑 |
| `--accent` | 主 CTA | `#0071e3` |
| `--ok` / `--err` | 状态 | 绿 / 红，克制 |
| 警告标签 | 自带库确认 | 琥珀系 |

### 字体

- 展示 / UI：`-apple-system, "SF Pro Display", "SF Pro Text", "PingFang SC", "Microsoft YaHei UI", sans-serif`  
- 日志 / 路径：`ui-monospace, "Cascadia Mono", Consolas, monospace`  

### 布局签名

枢纽：**大留白 + 双入口并排大卡片**（苹果站产品板块感）。  
工具页：同一 token，留白收紧、信息密度提高。

### 动效

入口卡片 hover 轻抬升；进度条宽度过渡；尊重 `prefers-reduced-motion`。

## 数据流

| 能力 | 变化 |
|------|------|
| `GET /api/status` | 枢纽摘要 + 顶栏状态点复用 |
| 解包 scan / jobs / config | 语义不变；UI 在 `/unpack` |
| 角色库 roster API | 不变 |
| `GET /api/jobs` | **新增**（若 job_store 尚无列表）：最近 N 条 id / kind / status / progress / updated_at |

枢纽不写库。

## 错误与空态

- 环境异常：顶栏色点 + 枢纽可读写明缺项；不阻断进入子页  
- 无最近任务：「尚无解包任务」+ 链到解包  
- job 过期/缺失：unpack 横幅，表单仍可用  
- 文案：说明结果与下一步，避免空泛道歉  

## 文件落点（预期）

| 路径 | 作用 |
|------|------|
| `hanimport/web/design-system/tokens.css` | CSS 变量 |
| `hanimport/web/shell.css` + 共享顶栏片段/脚本 | 三页壳 |
| `hanimport/web/index.html` | 枢纽（替换原解包首页） |
| `hanimport/web/hub.js`（或等价） | 枢纽逻辑 |
| `hanimport/web/unpack.html` + 迁入的解包 JS/CSS | 解包页 |
| `hanimport/web/roster.html` 等 | 换壳浅色 |
| `hanimport/scripts/serve_web.py` | 路由与可选 jobs 列表 |
| `hanimport/scripts/job_store.py` | 如需 list_recent |

抽取产物可放 `.extract-design-system/` 或 skillui 输出目录；**勿提交**体积过大的原始抓取缓存（若过大则 gitignore）。

## 验收

1. `/` 为浅色枢纽：双入口 + 环境摘要 + 最近任务区  
2. `/unpack` 覆盖现有解包全流程（扫描、异步进度、日志、生成配置）  
3. `/roster` 功能与既有角色库设计一致，仅视觉与壳层变化  
4. 三页顶栏导航高亮正确；窄屏堆叠可用  
5. Token 经苹果设计站抽取并人工收敛；无新 UI 组件库；业务 API 语义除可选 `GET /api/jobs` 外不变  

## 决策记录

| 项 | 选择 |
|----|------|
| 范围 | C 大幅重做结构与流程观感 |
| IA | 中心枢纽 |
| 色调 | 浅色 |
| 参考 | apple.com/design |
| 工程 | 多页 + 抽 token（A），非 SPA |
