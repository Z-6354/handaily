# Monorepo hygiene (entry + docs) — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans (or implement directly). Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Unify startup entry docs/bats and rewrite live docs + `docs/questions` paths for the hanpet/hanimport monorepo layout.

**Architecture:** Docs-only + one thin `start-dev.bat` wrapper. Mechanical path rewrites in questions with a short Python/PowerShell helper; hand-fix structure docs (esp. Q121).

**Tech Stack:** Markdown, PowerShell bat wrappers, optional one-off rewrite script (deleted after or kept under scripts if useful).

## Global Constraints

- No business logic / API / UI / DB changes.
- Do not rewrite `docs/superpowers/specs|plans` history unless a live index link is wrong.
- Prefer one or two commits; commit after each major task per hanskill #003.

---

### Task 1: Official entry — `scripts/start-dev.bat` + `scripts/README.md`

**Files:** `scripts/start-dev.bat` (new), `scripts/README.md`, root `README.md` (one-line link)

- [ ] Create `scripts/start-dev.bat` mirroring `scripts/start.bat` → `start-dev.ps1`
- [ ] Rewrite `scripts/README.md` official entry table per spec §2
- [ ] Add to root `README.md` under 快速启动: link to `scripts/README.md`
- [ ] Commit: `docs(scripts): add start-dev.bat and align entry README`

### Task 2: Live docs spot-check

**Files:** `docs/ARCHITECTURE.md`, `docs/01-项目总览/04-代码规范/03-模块索引.md`, `docs/README.md`

- [ ] Skim and fix any stale root `src-tauri` / missing hantransfer mention
- [ ] Commit: `docs: align ARCHITECTURE and module index with monorepo`

### Task 3: questions path rewrite

**Files:** `docs/questions/*.md` (~73 files), esp. `121-*.md`

- [ ] Run mechanical replacements per spec §4 (safe order: longest prefixes first; never double-prefix `hanpet/hanpet`)
- [ ] Hand-edit Q121 directory tree to `HANDAILY/hanpet/...`
- [ ] Verify with ripgrep: count of bare `src-tauri/` (not preceded by `hanpet/`) near zero
- [ ] Spot-check 5 files
- [ ] Commit: `docs(questions): rewrite paths for hanpet monorepo layout`

### Task 4: Final verification

- [ ] Confirm `scripts/start-dev.bat` exists and `scripts/README` lists it
- [ ] `rg` sanity on questions
- [ ] Report leftover intentional exceptions if any
