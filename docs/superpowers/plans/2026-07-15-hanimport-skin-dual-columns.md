# hanimport Skin Dual-Column Inventory Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans or implement task-by-task in-session.

**Goal:** Show pet Spine / kanmusu Live2D readiness per skin in character detail and on `/skins`.

**Architecture:** `skin_probe.py` checks local folders; roster list/detail attach statuses; new `/skins` page + nav link; no WebGL.

**Tech Stack:** Python stdlib, vanilla JS/CSS, existing serve_web/roster_api.

**Spec:** `docs/superpowers/specs/2026-07-15-hanimport-skin-dual-columns-design.md`

## Global Constraints

- No Spine/Cubism player in v1
- Status: unbound | missing | ready only
- Paths under repo `data/live2d` and `data/model/unpacked` by default
- Light shell consistent with roster

## Tasks

### Task 1: Probe + API enrich

- Create `hanimport/scripts/skin_probe.py`: `probe_pet(id)`, `probe_kanmusu(dir)` → status + path
- Enrich skins in `_get_character` and new `_list_skins`
- Route `GET /api/roster/skins`
- Tests for unbound/missing/ready with tmp dirs
- Commit

### Task 2: Detail dual-column UI

- Update `roster.js`/`roster.css` skins block to table with two status columns
- Link to `/skins?character_id=`
- Commit

### Task 3: `/skins` page + shell nav

- `skins.html` / `skins.js` / `skins.css`; serve routes; `shell.js` nav item
- Filters + pagination + jump to roster
- Commit

### Task 4: Smoke

- pytest + route smoke; mark spec 已实现
- Commit
