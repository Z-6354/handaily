# hanimport 解包 Job + 进度条 Implementation Plan

**进度**: 已完成（2026-07-15）

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** 解包页支持异步批量解包、进度条轮询，以及写入后自动生成 JSON 配置。

**Architecture:** 在 `serve_web.py` 内增加进程内 Job 注册表与后台线程；`POST /api/jobs/unpack|config` 立即返回 `job_id`，`GET /api/jobs/{id}` 供前端 300–500ms 轮询。前端 `app.js`/`index.html` 增加勾选、进度条与子集选择。

**Tech Stack:** Python 3 stdlib (`ThreadingHTTPServer`, `threading`), vanilla JS, 复用 `unpack_bundle.py` / `build_model_config.py` / `build_cubism_config.py`

**Spec:** `docs/superpowers/specs/2026-07-15-hanimport-unpack-jobs-design.md`

## Global Constraints


- 仅本机 `127.0.0.1`；不引入新依赖
- 保留原 `/api/unpack`、`/api/config` 可调用（内部改为创建 Job 并同步等待，或标记 deprecated 但仍工作）；优先让 UI 走新 Job API
- 路径与输出规则沿用现有 `resolve_output` / `discover_bundles`

## File map

| 文件 | 职责 |
|------|------|
| `hanimport/scripts/job_store.py` | Job 模型、注册表、进度更新 |
| `hanimport/scripts/serve_web.py` | HTTP 路由挂 Job |
| `hanimport/web/index.html` | 勾选、进度条 DOM |
| `hanimport/web/app.js` | 轮询、子集、流水线 |
| `hanimport/web/style.css` | 进度条样式 |
| `hanimport/scripts/test_job_store.py` | 单元测试 |

---

### Task 1: Job store

**Files:**
- Create: `hanimport/scripts/job_store.py`
- Create: `hanimport/scripts/test_job_store.py`

**Interfaces:**
- Produces: `create_job(kind) -> str`, `get_job(id) -> dict|None`, `update_job(id, **fields)`, `append_log(id, line)`, `JOBS` thread-safe

- [x] **Step 1: Write failing test**

```python
# hanimport/scripts/test_job_store.py
from job_store import create_job, get_job, update_job, append_log

def test_create_and_progress():
    jid = create_job("unpack")
    snap = get_job(jid)
    assert snap["status"] == "queued"
    assert snap["kind"] == "unpack"
    update_job(jid, status="running", current=1, total=3, current_item="a")
    append_log(jid, "unpack a")
    snap2 = get_job(jid)
    assert snap2["current"] == 1
    assert snap2["total"] == 3
    assert "unpack a" in snap2["log_tail"]
```

- [x] **Step 2: Run test — expect fail (module missing)**

Run: `cd hanimport/scripts && python -m pytest test_job_store.py -v`  
Expected: FAIL import error

- [x] **Step 3: Implement `job_store.py`**

```python
from __future__ import annotations

import threading
import time
import uuid
from typing import Any

_lock = threading.Lock()
_JOBS: dict[str, dict[str, Any]] = {}
_LOG_MAX = 200


def create_job(kind: str) -> str:
    jid = uuid.uuid4().hex[:12]
    now = time.time()
    with _lock:
        _JOBS[jid] = {
            "id": jid,
            "kind": kind,
            "status": "queued",
            "phase": "",
            "current": 0,
            "total": 0,
            "current_item": "",
            "ok_count": 0,
            "fail_count": 0,
            "log_tail": [],
            "error": None,
            "results": [],
            "created_at": now,
            "updated_at": now,
        }
    return jid


def get_job(job_id: str) -> dict[str, Any] | None:
    with _lock:
        j = _JOBS.get(job_id)
        return dict(j) if j else None


def update_job(job_id: str, **fields: Any) -> None:
    with _lock:
        j = _JOBS.get(job_id)
        if not j:
            return
        for k, v in fields.items():
            if k == "log_tail":
                continue
            j[k] = v
        j["updated_at"] = time.time()


def append_log(job_id: str, line: str) -> None:
    with _lock:
        j = _JOBS.get(job_id)
        if not j:
            return
        j["log_tail"].append(line)
        if len(j["log_tail"]) > _LOG_MAX:
            j["log_tail"] = j["log_tail"][-_LOG_MAX:]
        j["updated_at"] = time.time()
```

- [x] **Step 4: Run test — expect PASS**

Run: `cd hanimport/scripts && python -m pytest test_job_store.py -v`

- [x] **Step 5: Commit**（若用户要求提交时再执行）

---

### Task 2: Unpack / config workers + HTTP

**Files:**
- Modify: `hanimport/scripts/serve_web.py`

**Interfaces:**
- Consumes: `job_store.create_job|get_job|update_job|append_log`
- Produces: `POST /api/jobs/unpack`, `POST /api/jobs/config`, `GET /api/jobs/{id}`
- Reuse: `discover_bundles`, `run_unpack_one`, `resolve_output`, `build_folder_configs`

- [x] **Step 1: Add worker helpers in `serve_web.py`**

实现要点（完整代码写入文件时展开）：

1. `start_unpack_job(body) -> job_id`：过滤 `slugs`（可选）；`threading.Thread(target=run_unpack_job, daemon=True).start()`
2. `run_unpack_job`：逐项 `run_unpack_one`；`continue_on_error` 时累计 `fail_count` 继续，否则 `status=error` 返回
3. 若 `generate_config` 且非 dry_run：同一 job `phase=config`，对每个成功 `output_dir`：
   - 存在 `.skel` → `build_folder_configs`
   - 存在 `.moc3` → 调用 `build_cubism_config` 对应该 slug 的入口（读脚本现有 CLI 函数；无则对 folder 写 meta 的最小封装）
4. `kind=unpack_then_config` 当 `generate_config`；否则 `unpack`
5. GET `/api/jobs/{id}` → `get_job` 或 404
6. 静态路由增加后续 roster 预留不影响本 Task

示例路由片段：

```python
if path == "/api/jobs/unpack":
    jid = create_job("unpack_then_config" if body.get("generate_config") else "unpack")
    threading.Thread(
        target=run_unpack_job,
        args=(jid, body),
        daemon=True,
    ).start()
    self._send_json(200, {"ok": True, "job_id": jid})
    return

# GET
if path.startswith("/api/jobs/"):
    jid = path[len("/api/jobs/"):].strip("/")
    snap = get_job(jid)
    if not snap:
        self._send_json(404, {"ok": False, "error": "job not found"})
        return
    self._send_json(200, {"ok": True, "job": snap})
    return
```

- [x] **Step 2: Manual smoke**

Run: `python hanimport/scripts/serve_web.py`  
`POST /api/jobs/unpack` with small dry_run input → poll until `done`

- [x] **Step 3: Commit**（若用户要求）

---

### Task 3: Frontend progress UI

**Files:**
- Modify: `hanimport/web/index.html`
- Modify: `hanimport/web/app.js`
- Modify: `hanimport/web/style.css`

- [x] **Step 1: HTML**

在解包区增加：

```html
<label class="check"><input id="opt-gen-config" type="checkbox" checked /> 写入后生成 JSON</label>
<label class="check"><input id="opt-continue" type="checkbox" /> 遇错继续</label>
<div class="progress-wrap" hidden id="progress-wrap">
  <div class="progress-bar"><div id="progress-fill"></div></div>
  <div id="progress-label" class="muted"></div>
</div>
<nav class="topnav"><a href="/">解包</a> · <a href="/roster">角色库</a></nav>
```

扫描结果改为带 checkbox 的列表（`data-slug`）。

- [x] **Step 2: JS 轮询**

```javascript
async function pollJob(jobId, onTick) {
  for (;;) {
    const data = await api(`/api/jobs/${jobId}`);
    const job = data.job;
    onTick(job);
    if (job.status === "done" || job.status === "error") return job;
    await new Promise((r) => setTimeout(r, 400));
  }
}

function renderProgress(job) {
  const wrap = $("progress-wrap");
  wrap.hidden = false;
  const pct = job.total ? Math.round((100 * job.current) / job.total) : 0;
  $("progress-fill").style.width = pct + "%";
  $("progress-label").textContent =
    `${job.phase || job.kind} ${job.current}/${job.total} ${job.current_item || ""} (${pct}%)`;
}
```

`onUnpack`：收集勾选 slug → `POST /api/jobs/unpack` → `pollJob`；把 `log_tail` 增量写入日志。  
`onConfig`：改走 `POST /api/jobs/config`。

- [x] **Step 3: CSS**

```css
.progress-wrap { margin: 0.75rem 0; }
.progress-bar { height: 8px; background: #e5e7eb; border-radius: 4px; overflow: hidden; }
#progress-fill { height: 100%; width: 0; background: #2563eb; transition: width 0.2s; }
.topnav { margin-bottom: 1rem; font-size: 0.9rem; }
```

- [x] **Step 4: Browser 验收**（对照 spec 验收 1–5）

- [x] **Step 5: Commit**（若用户要求）

---

### Task 4: Spec self-check

- [x] 核对 spec 验收 1–5 均有对应 UI/API
- [x] dry-run 不调用写盘配置
- [x] `/roster` 链接可临时 404，Phase 2 再补页（顶栏先放链接即可）

---

## Spec coverage

| Spec 项 | Task |
|---------|------|
| 异步 Job + 轮询 | 1–2 |
| 批量 + 勾选 | 3 |
| 写入后生成 JSON | 2–3 |
| 进度条 | 3 |
| 遇错继续 | 2–3 |
| 手动生成 JSON Job | 2–3 |
