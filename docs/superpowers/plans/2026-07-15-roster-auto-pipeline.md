# Roster Wiki Auto Pipeline Implementation Plan

**进度**: 已完成（2026-07-15）

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** Open local `/roster` auto-runs Wiki sync as characters → avatars+skins → lines; remove「导入 Wiki」button.

**Architecture:** Split `run_import_wiki` into phase helpers; `wiki_pipeline_jobs.py` orchestrates one `roster-wiki-pipeline` job; UI single toast + auto-start.

**Tech Stack:** Python (hanimport scripts), job_store, vanilla roster.js/html/css, pytest.

## Global Constraints


- Phase order hard: characters → avatars_skins → lines
- No `upsert_skin(id=folder)` for new skins; bind models only
- bundled never auto-starts pipeline
- CLI `import-wiki` may remain for debug; UI button gone
- UTF-8; commit after each task

## File map

| File | Role |
|------|------|
| `hanimport/scripts/roster_db.py` | `import_wiki_characters`, `import_wiki_skins`, `bind_unpacked_models`, `import_wiki_lines`; keep thin `run_import_wiki` wrapping all |
| `hanimport/scripts/wiki_pipeline_jobs.py` | Orchestrator job |
| `hanimport/scripts/roster_api.py` | op `wiki-pipeline`; stop chaining avatar+lines after import-wiki |
| `hanimport/web/roster.{html,js,css}` | Remove import button; pipeline toast |
| `hanimport/scripts/test_wiki_pipeline_phases.py` | Phase order + no folder-id skins |
| `hanimport/scripts/test_bind_models.py` | Bind without creating dirty skins |

---

### Task 1: Phase helpers + model bind (no dirty skins)

**Files:**
- Modify: `hanimport/scripts/roster_db.py`
- Create: `hanimport/scripts/test_bind_models.py`
- Create: `hanimport/scripts/test_wiki_pipeline_phases.py` (characters/skins without lines first)

**Produces:**
- `import_wiki_characters(conn, wiki, …) -> list[str]` character ids
- `import_wiki_skins_for_characters(conn, wiki, cids, ship_cols)` skins replace only
- `bind_unpacked_models(conn, unpacked, pet_models, alias_map, cn_to_slug)` updates existing skins only
- `import_wiki_lines_for_characters(conn, wiki, cids, ship_cols, lines_stats)`
- `run_import_wiki(..., phases: set[str] | None = None)` default all phases for CLI

- [x] **Step 1:** Test `bind_unpacked_models` does not create `id=folder` skin when only `{cid}-default` exists; sets `kanmusu_dir` on matching skin by ordinal `_N`.

- [x] **Step 2:** Implement helpers; change folder loop to bind-only; `run_import_wiki` calls phases in order.

- [x] **Step 3:** `pytest hanimport/scripts/test_bind_models.py hanimport/scripts/test_skins_replace.py -q` PASS

- [x] **Step 4:** Commit `refactor(hanimport): split wiki import phases; bind models without dirty skins`

---

### Task 2: `wiki_pipeline_jobs` + API

**Files:**
- Create: `hanimport/scripts/wiki_pipeline_jobs.py`
- Modify: `hanimport/scripts/roster_api.py`
- Test: `hanimport/scripts/test_wiki_pipeline_job.py` (mock/light: create job kind, phase field updates with fixture wiki+roster)

**Produces:**
- `start_wiki_pipeline_job(body) -> str`
- `run_wiki_pipeline_job`: phase `characters` → `avatars_skins` (skins+bind + run avatar fetch inline) → `lines` (fetch missing then import lines)
- `POST .../ops/wiki-pipeline`
- Remove post-`import-wiki` auto enqueue of avatar+lines jobs (pipeline owns that)
- `find_active_job("roster-wiki-pipeline")` attach semantics in start

- [x] **Step 1:** Implement job runner respecting pause between characters/ships.

- [x] **Step 2:** Wire API `wiki-pipeline`.

- [x] **Step 3:** Smoke pytest or minimal unit for start returns job_id and kind.

- [x] **Step 4:** Commit `feat(hanimport): roster-wiki-pipeline job and API`

---

### Task 3: UI — remove button, single pipeline toast

**Files:**
- Modify: `hanimport/web/roster.html`, `roster.js`, `roster.css`
- Update spec status in `docs/superpowers/specs/2026-07-15-roster-auto-pipeline-design.md`

**Produces:**
- No `#btn-import-wiki`
- `#pipeline-toast` (reuse avatar-toast styles); title by phase
- `maybeStartWikiPipeline()` on local list load; attach active job
- Remove auto parallel avatar+lines starts

- [x] **Step 1:** HTML/JS/CSS changes.

- [x] **Step 2:** Manual smoke note in commit.

- [x] **Step 3:** Commit `feat(hanimport): auto wiki pipeline toast; drop import wiki button`

- [x] **Step 4:** Mark spec 已实现; commit docs if needed
