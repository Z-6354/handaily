# Roster filter + sort — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans (or implement directly with TDD).

**Goal:** Server-side filter/sort on `GET /api/roster/characters` + roster UI filter bar.

**Tech:** Python roster_api + skin_probe; roster.html/js/css

---

### Task 1: Character asset aggregate helpers (TDD)

**Files:** `character_assets.py` (new) or in `skin_probe.py`; `test_character_assets.py`

- `best_status(statuses) -> unbound|missing|ready`
- `aggregate_character_assets(conn, cid) -> {skin_count, kanmusu_status, pet_status, import_mtime}`

### Task 2: List API query params + factions endpoint

**Files:** `roster_api.py`, `test_roster_api.py`

### Task 3: UI filter bar

**Files:** `roster.html`, `roster.js`, `pages/roster.css`
