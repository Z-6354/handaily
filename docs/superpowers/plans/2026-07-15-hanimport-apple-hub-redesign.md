# hanimport Apple-style hub redesign Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rebuild hanimport web as a light Apple-inspired multi-page shell: hub at `/`, unpack at `/unpack`, roster restyled under shared chrome — without changing business API semantics except adding `GET /api/jobs`.

**Architecture:** Extract design primitives from apple.com/design for reference, hand-converge into `tokens.css`. Shared `shell.css`/`shell.js` inject nav + status dot. Move existing unpack UI to `unpack.html`; new hub `index.html`+`hub.js`. Extend `job_store.list_jobs` and serve it at `GET /api/jobs`. Restyle roster on the same tokens.

**Tech Stack:** Python 3 stdlib HTTP + vanilla HTML/CSS/JS; skillui + extract-design-system (reference only); pytest for job list + route smoke.

**Spec:** `docs/superpowers/specs/2026-07-15-hanimport-apple-hub-redesign-design.md`

## Global Constraints

- Light theme only; no SPA, no UI component library
- Reference URL: `https://www.apple.com/design/` — extract then **hand-converge**; never overwrite app styles with raw dump
- Business APIs unchanged except optional/required `GET /api/jobs` list
- Roster CRUD / ops semantics unchanged; db switch stays on roster page only
- Chinese UI copy; system font stack with PingFang / YaHei fallbacks
- Respect `prefers-reduced-motion`
- Do not commit huge crawl caches; gitignore `.extract-design-system/raw*` and skillui crawl dirs if bulky

## File map

| File | Responsibility |
|------|----------------|
| `hanimport/web/design-system/tokens.css` | CSS variables (hand-converged) |
| `hanimport/web/design-system/tokens.json` | Optional dump of converged tokens for docs |
| `hanimport/web/shell.css` | Shared top bar, layout helpers |
| `hanimport/web/shell.js` | Render nav + fetch `/api/status` for status dot |
| `hanimport/web/index.html` + `hub.js` + `hub.css` | Hub page |
| `hanimport/web/unpack.html` | Unpack workbench (migrated from old index) |
| `hanimport/web/app.js` + `style.css` | Unpack logic/styles (paths updated for shell/tokens) |
| `hanimport/web/roster.html` + `roster.css` (+ maybe `roster.js` nav only) | Shell + light restyle |
| `hanimport/scripts/job_store.py` | `list_jobs(limit=20) -> list[dict]` |
| `hanimport/scripts/serve_web.py` | Static routes + `GET /api/jobs` |
| `hanimport/scripts/test_job_store_list.py` | Unit tests for list_jobs |
| `hanimport/scripts/test_serve_routes.py` | HTTP smoke for `/`, `/unpack`, `/api/jobs` |
| `.gitignore` | Ignore bulky extract caches |
| `docs/.../apple-hub...-design.md` | Mark status approved after ship (optional final step) |

---

### Task 1: Converge design tokens

**Files:**
- Create: `hanimport/web/design-system/tokens.css`
- Create (optional): `hanimport/web/design-system/tokens.json`
- Modify: `.gitignore` (root)
- Note: extraction outputs under `hanimport/web/.extract-ref/` or repo `.extract-design-system/` — gitignore raw caches

**Interfaces:**
- Produces: CSS custom properties consumed by all pages:
  - `--bg`, `--surface`, `--text`, `--muted`, `--hairline`, `--accent`, `--accent-hover`, `--ok`, `--err`, `--warn`, `--warn-bg`, `--font-ui`, `--font-mono`, `--radius`, `--shadow-card`, `--nav-height`

- [ ] **Step 1: Attempt extraction (best-effort)**

From repo root (network required):

```bash
mkdir -p hanimport/web/.extract-ref
npx --yes extract-design-system https://www.apple.com/design/ --extract-only
# If skillui works non-interactively:
skillui --url https://www.apple.com/design/ --out hanimport/web/.extract-ref --no-skill --format design-md
```

If either fails (timeout, blocked, empty), continue with Step 2 using spec palette only — record a one-line note in `tokens.json` field `"source": "spec-fallback"` vs `"source": "apple.com/design+hand"`.

- [ ] **Step 2: Write converged `tokens.css`**

```css
:root {
  color-scheme: light;
  --bg: #f5f5f7;
  --surface: #ffffff;
  --text: #1d1d1f;
  --muted: #6e6e73;
  --hairline: rgba(0, 0, 0, 0.08);
  --accent: #0071e3;
  --accent-hover: #0077ed;
  --ok: #34c759;
  --err: #ff3b30;
  --warn: #b25e09;
  --warn-bg: #fff4e5;
  --font-ui: -apple-system, "SF Pro Display", "SF Pro Text", "PingFang SC",
    "Microsoft YaHei UI", sans-serif;
  --font-mono: ui-monospace, "Cascadia Mono", Consolas, monospace;
  --radius: 12px;
  --shadow-card: 0 2px 12px rgba(0, 0, 0, 0.06);
  --nav-height: 52px;
}

@media (prefers-reduced-motion: reduce) {
  *, *::before, *::after {
    animation-duration: 0.01ms !important;
    transition-duration: 0.01ms !important;
  }
}
```

- [ ] **Step 3: Gitignore bulky extract artifacts**

Add to root `.gitignore`:

```
.extract-design-system/
hanimport/web/.extract-ref/
**/.skillui-cache/
```

- [ ] **Step 4: Commit**

```bash
git add hanimport/web/design-system/tokens.css hanimport/web/design-system/tokens.json .gitignore
git commit -m "feat(hanimport): add light design tokens for Apple-inspired shell"
```

---

### Task 2: `list_jobs` + `GET /api/jobs`

**Files:**
- Modify: `hanimport/scripts/job_store.py`
- Modify: `hanimport/scripts/serve_web.py`
- Create: `hanimport/scripts/test_job_store_list.py`
- Modify or create: `hanimport/scripts/test_serve_routes.py` (minimal HTTP for list)

**Interfaces:**
- Consumes: existing `_JOBS` dict shape from `create_job`
- Produces:
  - `list_jobs(limit: int = 20) -> list[dict[str, Any]]` — newest `updated_at` first; each item is a **shallow copy** without requiring `log_tail` truncation beyond what is stored (hub may ignore `log_tail`/`results` in UI)
  - `GET /api/jobs` → `{ "ok": true, "jobs": [ ... ] }` with optional `?limit=` (clamp 1–50, default 20)
  - Existing `GET /api/jobs/<id>` must keep working (more specific path first)

- [ ] **Step 1: Failing test**

```python
# hanimport/scripts/test_job_store_list.py
from job_store import create_job, update_job, list_jobs, get_job
import time

def test_list_jobs_order_and_limit():
    # clear by creating fresh module state — tests run in process; use unique kinds
    ids = []
    for i in range(3):
        jid = create_job("unpack")
        ids.append(jid)
        update_job(jid, status="done" if i else "running", current=i, total=3)
        time.sleep(0.01)
    listed = list_jobs(2)
    assert len(listed) == 2
    assert listed[0]["updated_at"] >= listed[1]["updated_at"]
    assert get_job(ids[-1])["id"] == listed[0]["id"]
```

- [ ] **Step 2: Run — expect fail**

```bash
cd hanimport/scripts
python -m pytest test_job_store_list.py -v
```

Expected: `ImportError` or `AttributeError: list_jobs`

- [ ] **Step 3: Implement `list_jobs`**

```python
def list_jobs(limit: int = 20) -> list[dict[str, Any]]:
    n = max(1, min(int(limit), 50))
    with _lock:
        items = sorted(_JOBS.values(), key=lambda j: j["updated_at"], reverse=True)
        return [dict(j) for j in items[:n]]
```

- [ ] **Step 4: Wire `serve_web.do_GET`**

Before `startswith("/api/jobs/")` single-id branch:

```python
if path == "/api/jobs":
    raw = _query.get("limit", "20")
    try:
        lim = int(raw)
    except ValueError:
        lim = 20
    self._send_json(200, {"ok": True, "jobs": list_jobs(lim)})
    return
```

Import `list_jobs` from `job_store`.

- [ ] **Step 5: Tests pass + commit**

```bash
cd hanimport/scripts
python -m pytest test_job_store_list.py test_serve_jobs.py -v
git add job_store.py serve_web.py test_job_store_list.py
git commit -m "feat(hanimport): list recent jobs for hub"
```

---

### Task 3: Shared shell (CSS + JS)

**Files:**
- Create: `hanimport/web/shell.css`
- Create: `hanimport/web/shell.js`

**Interfaces:**
- Consumes: `/api/status` JSON (`ok`, `unitypy`, …)
- Produces: `window.HanShell.mount({ active: "hub"|"unpack"|"roster" })`
  - Inserts `<header class="app-shell">` as first child of `body` (or into `#shell-root` if present)
  - Nav links: `/` 概览, `/unpack` 解包, `/roster` 角色库
  - Status control: `<a class="status-dot …" href="/" title="…">` with classes `ok|warn|err` — `err` if `!unitypy`, else `ok` (extend later if status grows)

- [ ] **Step 1: Write `shell.css`**

Include: fixed/sticky top bar height `var(--nav-height)`, hairline bottom, brand left, centered or left nav links, status dot right (8px circle). Active link: `color: var(--text); font-weight: 600`. Body padding-top for sticky bar.

- [ ] **Step 2: Write `shell.js`**

```javascript
export async function mount({ active }) {
  // Without bundler: attach as IIFE on window.HanShell instead of export
}
```

Use IIFE (no bundler):

```javascript
window.HanShell = {
  async mount({ active }) {
    const root = document.getElementById("shell-root") || document.body;
    const header = document.createElement("header");
    header.className = "app-shell";
    header.innerHTML = `...nav markup...`;
    // mark [data-nav=active] 
    if (!document.querySelector(".app-shell")) {
      root.insertBefore(header, root.firstChild);
    }
    try {
      const res = await fetch("/api/status");
      const data = await res.json();
      const dot = header.querySelector(".status-dot");
      const bad = !data.unitypy;
      dot.classList.add(bad ? "warn" : "ok");
      dot.title = bad ? "环境需注意（见概览）" : "环境正常";
    } catch {
      header.querySelector(".status-dot")?.classList.add("err");
    }
  },
};
```

- [ ] **Step 3: Serve static assets**

In `serve_web._serve_static`, allow:

`/shell.css`, `/shell.js`, `/design-system/tokens.css`, `/hub.js`, `/hub.css`, `/unpack.html`, `/unpack`

Map `/unpack` → `unpack.html`. Map `/design-system/tokens.css` → `WEB_DIR / "design-system/tokens.css"`.

- [ ] **Step 4: Commit**

```bash
git add hanimport/web/shell.css hanimport/web/shell.js hanimport/scripts/serve_web.py
git commit -m "feat(hanimport): shared light app shell chrome"
```

---

### Task 4: Hub page + move unpack to `/unpack`

**Files:**
- Create: `hanimport/web/hub.js`, `hanimport/web/hub.css`
- Rewrite: `hanimport/web/index.html` (hub)
- Create: `hanimport/web/unpack.html` (content migrated from old index)
- Modify: `hanimport/web/app.js` (support `?job=`; call `HanShell.mount({active:"unpack"})`)
- Modify: `hanimport/web/style.css` (import tokens; light surfaces; remove dark `:root` overrides — move page-specific rules only)

**Interfaces:**
- Hub fetches `GET /api/status` and `GET /api/jobs?limit=10`
- Job row link: `/unpack?job=<id>`
- `app.js` on load: if `URLSearchParams` has `job`, poll `GET /api/jobs/<id>`; if 404 show `#job-banner` text 「任务不存在或已清理」

- [ ] **Step 1: Create `unpack.html`**

Copy structure from current `index.html` unpack sections; change links; add:

```html
<link rel="stylesheet" href="/design-system/tokens.css" />
<link rel="stylesheet" href="/shell.css" />
<link rel="stylesheet" href="/style.css" />
<div id="shell-root"></div>
<div id="job-banner" class="banner" hidden></div>
...existing unpack cards...
<script src="/shell.js"></script>
<script src="/app.js"></script>
<script>HanShell.mount({ active: "unpack" });</script>
```

- [ ] **Step 2: Rewrite `index.html` as hub**

Hero title 小寒导入器; subtitle 本地解包与角色库工作台; two `.entry-card` links; `#env-summary`; `#recent-jobs`.

- [ ] **Step 3: `hub.js`**

```javascript
async function load() {
  await HanShell.mount({ active: "hub" });
  const st = await fetch("/api/status").then((r) => r.json());
  // render env summary lines
  const jobs = await fetch("/api/jobs?limit=10").then((r) => r.json());
  // empty state vs list with status + link
}
load();
```

- [ ] **Step 4: `app.js` job deep-link**

At end of existing init:

```javascript
const jobId = new URLSearchParams(location.search).get("job");
if (jobId) {
  // reuse existing poll helpers; on 404 show banner
}
```

- [ ] **Step 5: Light-adapt `style.css`**

Replace dark `:root` with `@import` or rely on tokens.css linked first; buttons/cards use `--surface`, `--hairline`, `--accent`.

- [ ] **Step 6: Manual + route smoke**

```bash
cd hanimport/scripts
python -c "from pathlib import Path; import serve_web; assert (serve_web.WEB_DIR/'unpack.html').is_file()"
python -m pytest test_job_store_list.py -v
```

Start server, open `/` and `/unpack`.

- [ ] **Step 7: Commit**

```bash
git add hanimport/web/ hanimport/scripts/serve_web.py
git commit -m "feat(hanimport): hub home and unpack route"
```

---

### Task 5: Restyle roster + shared shell

**Files:**
- Modify: `hanimport/web/roster.html`, `roster.css`, `roster.js` (mount shell; drop old dual-link topnav or replace)
- Modify: `serve_web.py` if any new assets

**Interfaces:**
- Consumes: `HanShell.mount({ active: "roster" })`
- Keeps all existing roster API calls and confirm_bundled flows
- Ops bar: keep all buttons; style `#btn-import-wiki` and `#btn-sync-appdata` as `.primary`, others secondary (CSS only — no remove)

- [ ] **Step 1: Update `roster.html` head/body chrome**

Link tokens + shell; `<div id="shell-root">`; remove obsolete `topnav` 解包·角色库 or leave empty; page title area stays with db-switch.

- [ ] **Step 2: Light `roster.css`**

Surfaces white, hairlines, selected list item `background: rgba(0,113,227,0.08)`; badge.warn uses `--warn` / `--warn-bg`; three-column layout retained; `@media (max-width: 900px)` stack columns.

- [ ] **Step 3: Call shell from roster.js init**

```javascript
if (window.HanShell) HanShell.mount({ active: "roster" });
```

- [ ] **Step 4: Manual check**

Open `/roster`, switch local/bundled, confirm warn badge visible; ensure Wiki/sync buttons still work against running server if available.

- [ ] **Step 5: Commit**

```bash
git add hanimport/web/roster.html hanimport/web/roster.css hanimport/web/roster.js
git commit -m "feat(hanimport): light shell restyle for roster"
```

---

### Task 6: Route smoke tests + spec acceptance pass

**Files:**
- Create: `hanimport/scripts/test_serve_routes.py`
- Modify: `docs/superpowers/specs/2026-07-15-hanimport-apple-hub-redesign-design.md` status → 已批准/已实现

**Interfaces:**
- Produces: pytest that boots `ThreadingHTTPServer` with `serve_web.Handler` (mirror `test_serve_jobs.py` pattern if any), GET `/`, `/unpack`, `/roster`, `/design-system/tokens.css`, `/api/jobs` → 200

- [ ] **Step 1: Write `test_serve_routes.py`**

```python
from http.client import HTTPConnection
from http.server import ThreadingHTTPServer
import threading
import serve_web
from job_store import create_job

def _server():
    httpd = ThreadingHTTPServer(("127.0.0.1", 0), serve_web.Handler)
    t = threading.Thread(target=httpd.serve_forever, daemon=True)
    t.start()
    port = httpd.server_address[1]
    return httpd, port

def test_pages_and_jobs_list():
    create_job("unpack")
    httpd, port = _server()
    try:
        c = HTTPConnection("127.0.0.1", port, timeout=3)
        for path in ("/", "/unpack", "/roster", "/design-system/tokens.css", "/api/jobs"):
            c.request("GET", path)
            r = c.getresponse()
            body = r.read()
            assert r.status == 200, path
            assert body, path
    finally:
        httpd.shutdown()
```

Adjust `Handler` class name to whatever `serve_web` exports (read file; use actual name).

- [ ] **Step 2: Run suite**

```bash
cd hanimport/scripts
python -m pytest test_job_store_list.py test_serve_routes.py test_serve_jobs.py -v
```

Expected: all PASS

- [ ] **Step 3: Acceptance checklist (manual)**

- [ ] `/` light hub: dual entry + env + recent jobs  
- [ ] `/unpack` full unpack flow  
- [ ] `/roster` ops/CRUD visually light, behavior same  
- [ ] Nav active states; narrow width stack  
- [ ] No new component library  

- [ ] **Step 4: Commit**

```bash
git add hanimport/scripts/test_serve_routes.py docs/superpowers/specs/2026-07-15-hanimport-apple-hub-redesign-design.md
git commit -m "test(hanimport): smoke routes for hub redesign"
```

---

## Spec coverage self-check

| Spec requirement | Task |
|------------------|------|
| Hub at `/` | T4 |
| `/unpack`, `/roster` | T4, T5 |
| Shared top bar + status | T3 |
| Light tokens from apple.com/design | T1 |
| `GET /api/jobs` | T2 |
| `?job=` deep link + missing banner | T4 |
| Roster semantics unchanged | T5 (CSS/chrome only) |
| Acceptance | T6 |
| No SPA / no component lib | Global |

Placeholder scan: none intentional. Type names: `list_jobs`, `HanShell.mount`, paths consistent across tasks.
