# Per-Skin Wiki Lines Implementation Plan

**进度**: 已完成（2026-07-15）

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or executing-plans. Steps use checkbox (`- [x]`) syntax.

**Goal:** Scrape BWIKI lines per skin panel; import into matching `skin_id`; surface unmatched/empty via import report + UI `lines_status`.

**Architecture:** `extractShipLinesBySkin` → `lines_by_skin_json` on wiki ships; `roster_db` match helpers + stop clone-all; API enrich `lines_status`; roster/`/skins` column + filter.

**Tech Stack:** TypeScript blhx-wiki scraper/db; Python roster_db/roster_api; vanilla roster/skins UI.

**Spec:** `docs/superpowers/specs/2026-07-15-per-skin-wiki-lines-design.md`

## Constraints


- Do not write unmatched wiki panels onto wrong skins
- Keep flat `lines_json` for backward compat
- Prefer `meta_json.lines_import` over new DB columns for status
- TDD for match normalize + assign

## File map

| File | Role |
|------|------|
| `mcp/blhx-wiki/src/types.ts` | `ShipLineGroup` / optional `skin` on line |
| `mcp/blhx-wiki/src/scraper.ts` | `extractShipLinesBySkin` |
| `mcp/blhx-wiki/src/db.ts` | `lines_by_skin_json` column + upsert |
| `hanimport/scripts/line_skin_match.py` | normalize + match + status (new) |
| `hanimport/scripts/roster_db.py` | import uses groups; report counters |
| `hanimport/scripts/roster_api.py` | enrich `lines_status`; filter |
| `hanimport/web/roster.*` / `skins.*` | 台词 column + filter |
| `hanimport/scripts/test_line_skin_match.py` | unit tests |

---

### Task 1: Scraper by-skin + wiki DB column

**Files:** `types.ts`, `scraper.ts`, `db.ts`; add/extend scraper tests if present

- [x] Parse 舰船台词 panels/tabs → groups with `skin` / `skin_kind` / `lines`
- [x] `extractShipLines` can flatten groups for old callers
- [x] Persist `lines_by_skin_json`; migrate CREATE/ALTER on open
- [x] Commit

### Task 2: Match lib + import path

**Files:** `line_skin_match.py`, `roster_db.py`, `test_line_skin_match.py`

- [x] `normalize_skin_label`, `match_wiki_group_to_skin`, `apply_lines_by_skin` with report
- [x] `run_import_wiki`: use `lines_by_skin_json` when non-empty; else mark `stale_flat` / only default carefully
- [x] Remove copy-all to every unpacked folder
- [x] Op response includes counters + `lines_report`
- [x] pytest red→green
- [x] Commit

### Task 3: API + UI flags

**Files:** `roster_api.py`, `roster.html/js/css`, `skins.html/js`

- [x] Skins list/detail: `lines_status` (+ optional wiki_skin tip)
- [x] Filter `lines_issue` / filter value for 台词有问题
- [x] Import log shows summary
- [x] Commit

### Task 4: Smoke

- [x] Re-fetch one multi-skin ship → import local → spot-check distinct lines + unmatched flag
- [x] Mark spec 已实现; commit
