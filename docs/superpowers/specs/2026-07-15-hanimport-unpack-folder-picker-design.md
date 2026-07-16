# hanimport 解包页 · 文件夹选择 + 多路径批量 — 设计

**日期**: 2026-07-15  
**状态**: 已实现  
**前置**: `2026-07-15-hanimport-unpack-jobs-design.md`（Job / 进度条已实现）

## 目标

优化 `/unpack` 解包页交互与批量逻辑：

1. **本机系统对话框**选择输入文件夹、输出文件夹、以及多个输入文件（不上传、不手抄长路径）
2. **多路径合并扫描**：一个主文件夹 + 可选追加的多个文件（可多次）→ 一份勾选列表 → 一次 Job 解包
3. 保留手填路径能力与现有 dry-run / 遇错继续 / 写入后生成 JSON

## 决策摘要

| 项 | 选择 |
|----|------|
| 文件夹/文件选择 | 服务端 tkinter 系统对话框（本机 `127.0.0.1`） |
| 输入形态 | 主输入 = 文件夹（或单文件）；「添加文件…」追加到附加列表 |
| 输出 | 系统对话框选目录；空则沿用 `resolve_output` |
| 解包内核 | 不改 UnityPy / `unpack_bundle.py` 算法 |
| 进度 | 继续复用 `/api/jobs/unpack` 轮询 |

## UI（`/unpack`）

```
输入路径  [________________] [浏览文件夹…]
附加文件  (chip/列表，可移除单条)  [添加文件…] [清空附加]
输出目录  [________________] [浏览…]     ← 空 = 自动
□ 仅预览  □ 写入后生成 JSON  □ 遇错继续
[扫描] [开始解包] [生成 JSON]
扫描结果（勾选） / 进度 / 日志
```

- 「浏览文件夹…」→ `POST /api/dialog/folder` → 写入主输入框  
- 「添加文件…」→ `POST /api/dialog/files`（多选）→ 追加到附加列表（去重，已存在则忽略）  
- 「浏览…」（输出）→ 同 folder dialog → 写入输出框  
- 主输入与附加路径仍可键盘编辑；清空附加不影响主输入  
- Apple 风格：路径行用现有 `components` / 解包页视觉，不另起一套

## API

### 对话框

| 方法 | 路径 | Body | 响应 |
|------|------|------|------|
| POST | `/api/dialog/folder` | `{ "title"? }` | `{ ok, path?, cancelled? }` |
| POST | `/api/dialog/files` | `{ "title"? }` | `{ ok, paths?: string[], cancelled? }` |

行为：

- 在服务端线程用 **tkinter** `askdirectory` / `askopenfilenames`（阻塞该请求；建议短时超时文案：用户未操作）
- 用户取消：`{ ok: true, cancelled: true }`（HTTP 200，前端不报错）
- 无 DISPLAY / tk 失败：`{ ok: false, error: "无法打开系统对话框：…" }`
- 仅本机；不加鉴权以外的信任假设（已是 loopback）

### 扫描（扩展）

`POST /api/scan`

```json
{
  "input": "D:\\...\\folder",          // 兼容旧字段；与 inputs[0] 二选一或并存
  "inputs": ["D:\\folder", "D:\\a", "D:\\b"]  // 可选；目录递归 + 文件
}
```

响应：

```json
{
  "ok": true,
  "bundles": [
    { "path": "...", "slug": "aidang", "source": "D:\\folder" }
  ],
  "warnings": ["slug 冲突：aidang 出现 2 次，已保留先扫到的路径"]
}
```

规则：

- 对每个存在的路径调用现有 `discover_bundles`，**按绝对路径去重**合并
- **slug 冲突**（同 slug 不同 path）：默认保留先出现的；`warnings` 说明；列表仍可只显示保留项（MVP）；后续若勾选要区分可用 `paths` 解包绕过 slug
- 全部无效：`ok: false, error: …`

### 解包 Job（扩展）

`POST /api/jobs/unpack` 在现有字段上增加：

| 字段 | 说明 |
|------|------|
| `inputs` | `string[]`，多根扫描；与 `input` 并存时合并 |
| `paths` | `string[]`，显式 bundle 绝对路径；**若提供则优先于**「discover + slugs」 |

推荐前端流程：扫描后勾选 → 提交时传勾选项的 `paths`（更准）+ 可选 `output` / 开关；仍可兼容只传 `input`+`slugs`。

`resolve_output`：有显式 `output` 用显式；否则用**主 `input`（或 inputs[0]）**推默认根，与今日单路径行为一致。

## 逻辑改动落点

| 文件 | 职责 |
|------|------|
| `hanimport/scripts/dialog_picker.py`（新） | tkinter 封装；可单测 mock |
| `hanimport/scripts/serve_web.py` | 路由、`discover_bundles_many`、`run_unpack_job` 支持 `inputs`/`paths` |
| `hanimport/web/unpack.html` | 浏览按钮、附加文件区 |
| `hanimport/web/app.js` | 对话框调用、多路径 scan/unpack |
| 测试 | `test_dialog_picker.py`（mock）、`test_serve_jobs` / scan 多路径与 `paths` 过滤 |

## 非目标

- 浏览器上传 AssetBundle / `showDirectoryPicker` 冒充绝对路径  
- 多进程并发解包（仍顺序 Job）  
- 改 `unpack_bundle.py` 提取算法  
- 解包后自动写 roster / 同步 AppData  

## 验收

1. 点「浏览文件夹」弹出系统选目录，确认后主输入框有绝对路径  
2. 「添加文件」可多选追加；扫描合并目录内 bundle + 附加文件；附加可移除  
3. 输出「浏览」可选目录；空输出时默认路径规则与现网一致  
4. 勾选子集后解包：Job 进度正常；`paths` 模式下只解勾选项  
5. 取消对话框不报错；无 tk 环境时错误可读  
6. dry-run / 遇错继续 / 生成 JSON 行为不变  

## Spec 自检

- [x] 无 TBD/占位实现描述  
- [x] 与 unpack-jobs 的 Job API 兼容（扩展字段，不破坏旧 `input`）  
- [x] 范围边界明确（非目标）  
- [x] slug 冲突有默认策略  
