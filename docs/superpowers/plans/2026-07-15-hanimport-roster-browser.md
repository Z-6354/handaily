# hanimport 角色库可视化管理 Implementation Plan

**进度**: 已完成（2026-07-15）

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** 在 hanimport 网页增加 `/roster`：自用/自带双库 CRUD（角色/皮肤/台词）、运维按钮、英文名默认=id。

**Architecture:** 扩展 `serve_web.py` 静态路由与 `/api/roster/*`；把 SQLite CRUD 与 ops 抽到 `roster_db.py` 可导入函数；前端 `roster.html` + `roster.js` 三栏布局。写 `bundled` 强制 `confirm_bundled`。

**Tech Stack:** Python stdlib HTTP + sqlite3, vanilla JS/CSS；复用 `roster_db.py` 路径与 import-wiki / sync / publish

**Spec:** `docs/superpowers/specs/2026-07-15-hanimport-roster-browser-design.md`  
**Depends on:** Phase 1 解包计划可并行前端顶栏，但本计划应在 Phase 1 顶栏 `/roster` 链接已存在后补齐页面。

## Global Constraints


- `db=local|bundled`；bundled 写入必须 `confirm_bundled: true`
- 英文名空白 → 存 `id`（角色与皮肤）
- 角色/皮肤主键创建后不可改
- 不引入 React

## File map

| 文件 | 职责 |
|------|------|
| `hanimport/scripts/roster_db.py` | `resolve_db_path`, CRUD, `fill_english`, ops 入口函数化 |
| `hanimport/scripts/roster_api.py` | 纯函数：处理 path+body → `(code, dict)`，便于测 |
| `hanimport/scripts/serve_web.py` | 挂 `/roster*` 静态 + 转发 API |
| `hanimport/web/roster.html` | 页面 |
| `hanimport/web/roster.js` | UI |
| `hanimport/web/roster.css` | 布局 |
| `hanimport/scripts/test_roster_api.py` | API/英文名测试 |

---

### Task 1: `name_en` 默认 id + fill_english

**Files:**
- Modify: `hanimport/scripts/roster_db.py`
- Create: `hanimport/scripts/test_roster_api.py`

**Interfaces:**
- Produces: `normalize_name_en(name_en: str, id_: str) -> str`, `fill_english_names(conn) -> dict`, upsert 路径调用 normalize

- [x] **Step 1: Failing test**

```python
from roster_db import normalize_name_en, connect, apply_schema, fill_english_names, default_local_db
import tempfile, sqlite3
from pathlib import Path

def test_normalize_name_en():
    assert normalize_name_en("", "cheshire") == "cheshire"
    assert normalize_name_en("  ", "edu") == "edu"
    assert normalize_name_en("Cheshire", "cheshire") == "Cheshire"

def test_fill_english(tmp_path: Path):
    db = tmp_path / "t.sqlite"
    conn = connect(db)
    apply_schema(conn)
    conn.execute(
        "INSERT INTO characters(id,name_zh,name_en) VALUES (?,?,?)",
        ("edu", "恶毒", ""),
    )
    conn.commit()
    n = fill_english_names(conn)
    assert n["characters"] >= 1
    en = conn.execute("SELECT name_en FROM characters WHERE id='edu'").fetchone()[0]
    assert en == "edu"
```

- [x] **Step 2: Implement**

```python
def normalize_name_en(name_en: str, id_: str) -> str:
    s = (name_en or "").strip()
    return s if s else id_

def fill_english_names(conn: sqlite3.Connection) -> dict:
    cur = conn.execute("SELECT id, name_en FROM characters")
    c_n = 0
    for cid, en in cur.fetchall():
        if not (en or "").strip():
            conn.execute("UPDATE characters SET name_en=? WHERE id=?", (cid, cid))
            c_n += 1
    cur = conn.execute("SELECT id, name_en FROM skins")
    s_n = 0
    for sid, en in cur.fetchall():
        if not (en or "").strip():
            conn.execute("UPDATE skins SET name_en=? WHERE id=?", (sid, sid))
            s_n += 1
    conn.commit()
    return {"characters": c_n, "skins": s_n}
```

在 `upsert_character` / `upsert_skin` / wiki import 写入前调用 `normalize_name_en`。

- [x] **Step 3: pytest PASS**

---

### Task 2: `roster_api.py` CRUD + 写保护

**Files:**
- Create: `hanimport/scripts/roster_api.py`
- Modify: `hanimport/scripts/test_roster_api.py`

**Interfaces:**
- Produces: `resolve_path(db: str) -> Path`, `require_write(db, body) -> str|None`（错误文案）, `handle(method, path, query, body) -> tuple[int, dict]`

规则：

```python
def require_bundled_confirm(db: str, body: dict) -> str | None:
    if db == "bundled" and not body.get("confirm_bundled"):
        return "写入自带库需要 confirm_bundled=true"
    return None
```

实现至少：`meta`, `list_characters`, `get_character`, `create/update/delete character`, skins CRUD, lines CRUD, `ops/fill-english`。  
`ops/import-wiki|sync-appdata|publish-bundled`：若 `db!='local'` → 400；否则调用 `roster_db` 已有 `cmd_*` 逻辑（抽成 `run_import_wiki()` 等无 argparse 函数）。

- [x] **Step 1: Test 403 without confirm**

```python
def test_bundled_write_requires_confirm(tmp_path, monkeypatch):
    # point bundled path to tmp sqlite with schema
    code, payload = handle("DELETE", "/api/roster/characters/x", {"db": "bundled"}, {})
    assert code == 403
```

- [x] **Step 2: Implement handlers + wire tests with temp DBs**

---

### Task 3: Wire `serve_web.py`

**Files:**
- Modify: `hanimport/scripts/serve_web.py`

- [x] Serve `roster.html`, `roster.js`, `roster.css` at `/roster`, `/roster.js`, …
- [x] `do_GET`/`do_POST`：`/api/roster...` → `roster_api.handle`
- [x] Smoke: open `/roster` returns 200

---

### Task 4: Roster UI

**Files:**
- Create: `hanimport/web/roster.html`
- Create: `hanimport/web/roster.js`
- Create: `hanimport/web/roster.css`

布局：顶栏导航 + db 切换；左列表；中角色+皮肤；右台词；操作条四按钮。

关键交互：

```javascript
let db = "local";
let confirmBundled = false;

async function rosterFetch(path, opts = {}) {
  const u = new URL(path, location.origin);
  u.searchParams.set("db", db);
  const body = opts.body ? { ...opts.body } : undefined;
  if (body && db === "bundled") {
    if (!confirm("确认写入自带预览库？\n" + metaPath)) throw new Error("cancelled");
    body.confirm_bundled = true;
  }
  // fetch...
}
```

删除前 `confirm` 显示 id + name_zh。

- [x] 手动验收 spec 验收 1–7

---

### Task 5: Ops buttons

- [x] 导入 Wiki / 同步 AppData / 发布：仅 `db=local` 启用；调用对应 POST；日志区显示返回摘要
- [x] 补齐英文名：两库可用（bundled 需确认）

---

## Spec coverage

| Spec 项 | Task |
|---------|------|
| `/` ↔ `/roster` | 3–4（依赖 Phase1 顶栏） |
| 双库切换 | 4 |
| CRUD 三角色/皮肤/台词 | 2–4 |
| 英文名 = id | 1 |
| confirm_bundled | 2–4 |
| 运维三按钮 | 2、5 |
| fill-english | 1、5 |
