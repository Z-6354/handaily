# hanimport Roster Avatar Grid Implementation Plan

**进度**: 已完成（2026-07-15）

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** Local Wiki avatars under `data/roster/avatars/`, roster left pane as avatar grid + detail, background fetch job with pauseable toast UI.

**Architecture:** Files on disk; `GET /avatars/{id}`; list API adds `avatar_url`; `avatar_fetch.py` resolves `catalog.avatar_url` and downloads via urllib; job_store gains pause/resume + `skipped`; roster ops `fetch-avatars`; frontend grid + bottom-right toast.

**Tech Stack:** Python 3 stdlib (urllib, sqlite3, threading), vanilla JS/CSS; existing job_store + serve_web.

**Spec:** `docs/superpowers/specs/2026-07-15-hanimport-roster-avatar-grid-design.md`

## Global Constraints


- Avatars: `data/roster/avatars/{id}.{ext}` only
- No modal progress dialog — toast only with pause/resume
- Auto-fetch only for `db=local`; bundled never auto-downloads
- Path traversal forbidden on `/avatars/`
- Concurrency ≤ 2 downloads
- Light Apple shell tokens for toast UI

## File map

| File | Role |
|------|------|
| `data/roster/avatars/.gitkeep` | Keep empty dir |
| `.gitignore` | Ignore `data/roster/avatars/*` except gitkeep |
| `hanimport/scripts/avatar_fetch.py` | Resolve URL, download, list missing |
| `hanimport/scripts/job_store.py` | pause/resume, skipped field |
| `hanimport/scripts/roster_db.py` | avatar path helpers; import-wiki enqueue hook point |
| `hanimport/scripts/roster_api.py` | fetch-avatars op; characters include avatar_url |
| `hanimport/scripts/serve_web.py` | `/avatars/`, pause/resume routes, start fetch worker |
| `hanimport/web/roster.html/css/js` | Grid + toast |
| `hanimport/scripts/test_avatar_fetch.py` | Unit tests |

---

### Task 1: Paths + static serve + list `avatar_url`

**Files:** create `data/roster/avatars/.gitkeep`; modify `.gitignore`, `roster_db.py`, `roster_api.py`, `serve_web.py`; test `test_avatar_fetch.py`

- [x] Avatars dir helper `avatars_dir() -> Path`, `resolve_avatar_file(id) -> Path|None`, `avatar_public_url(id) -> str|None`
- [x] `GET /avatars/{id}` safe
- [x] Characters list items include `avatar_url`
- [x] pytest: resolve missing/present; path traversal 404
- [x] Commit

### Task 2: Download worker + pause/resume job

**Files:** `avatar_fetch.py`, `job_store.py`, `roster_api.py`, `serve_web.py`, tests

- [x] `lookup_avatar_url(wiki_db, wiki_title, name_zh) -> str|None` from `catalog`
- [x] `download_avatar(url, dest_stem) -> Path`
- [x] `run_fetch_avatars_job(job_id, char_ids, ...)` loop with pause checks; status `paused` via phase or status field per spec (`phase` or use `status=paused`)
- [x] Spec uses status/phase: implement `status` in `{queued,running,paused,done,error}` for fetch jobs
- [x] `POST .../fetch-avatars`, `POST /api/jobs/{id}/pause|resume`
- [x] Commit

### Task 3: Grid UI + toast

**Files:** `roster.html`, `roster.css`, `roster.js`

- [x] Replace left list with card grid; keep selection → detail
- [x] Toast component; auto-start fetch when local + missing; poll job; pause/resume/hide
- [x] Default page size 48
- [x] Commit

### Task 4: Wire import-wiki enqueue + smoke

- [x] After import-wiki success, start/merge fetch for upserted ids (local)
- [x] pytest + manual checklist note
- [x] Commit; mark spec 已实现
