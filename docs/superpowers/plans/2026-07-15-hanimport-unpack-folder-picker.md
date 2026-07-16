# hanimport 解包文件夹选择 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 解包页用本机系统对话框选文件夹/多文件，并支持多路径合并扫描与 `paths[]` 精确解包。

**Architecture:** `dialog_picker.py` 封装 tkinter；`serve_web` 增加 dialog/scan/unpack 多路径与 `paths`；前端路径行 + 附加文件列表。复用现有 Job 轮询。

**Tech Stack:** Python 3 stdlib (tkinter), vanilla JS, pytest

**Spec:** `docs/superpowers/specs/2026-07-15-hanimport-unpack-folder-picker-design.md`

## Global Constraints

- 仅本机 loopback；不上传 bundle
- 兼容旧 `input` + `slugs` API
- 不改 UnityPy / `unpack_bundle.py` 内核
- 对话框取消返回 `cancelled: true`，不报错

## File map

| 文件 | 职责 |
|------|------|
| `hanimport/scripts/dialog_picker.py` | tk 选文件夹/多文件 |
| `hanimport/scripts/test_dialog_picker.py` | mock 单测 |
| `hanimport/scripts/serve_web.py` | API + discover_many + paths unpack |
| `hanimport/scripts/test_serve_jobs.py` / 新测 | 多路径 scan、paths job |
| `hanimport/web/unpack.html` | UI |
| `hanimport/web/app.js` | 对话框与多路径逻辑 |
| `hanimport/web/style.css` 或 unpack 相关 | 路径行样式 |

---

### Task 1: dialog_picker

- [x] 写 `test_dialog_picker.py`（mock askdirectory/askopenfilenames）
- [x] 实现 `dialog_picker.py`：`pick_folder(title)`, `pick_files(title)` → path/None or list
- [x] pytest 通过

### Task 2: serve_web discover + scan + job

- [x] 测 `discover_bundles_many`：目录+文件合并去重、slug 冲突 warning
- [x] 测 unpack job 接受 `paths` 跳过 discover
- [x] 实现路由 `/api/dialog/folder|files`、扩展 scan/unpack
- [x] 相关 pytest 通过

### Task 3: unpack UI

- [x] unpack.html：浏览文件夹/文件/输出、附加列表
- [x] app.js：调用 dialog、多 inputs scan、paths unpack
- [x] 手测清单就绪

### Task 4: 验收

- [x] pytest 全绿；更新 spec 状态为已实现
