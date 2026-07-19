# Roster Decouple Rewrite Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Split `hanimport/scripts/roster/db.py` monolith into real modules (option C) without changing bind/import behavior; keep `db.py` as a compatibility re-export hub.

**Architecture:** Shear by existing `# --- section ---` markers into `ids` / `schema` / `crud` / `merge` / `bind_pipeline` / `import_wiki` / `sync` / `cli`. C2 then peels pure rules into `folder_rules` + `aliases`. Entry points (`roster_db.py`, `web/import_ab_bind.py`, pytest) keep importing `roster.db` / `roster_db`.

**Tech Stack:** Python 3, sqlite3, pytest, existing `hanimport/scripts` path layout.

## Global Constraints

- Behavior canaries unchanged: bare slug=default; `slug_2`→skin1; no `slug_1`; `*_h`→oath pet-only; younv/idol→variant CN; kanmusu absent status; special pet skip.
- Do not delete `data/pet` / `data/skin`; do not re-run full unpack.
- Rollback baseline: `hanimport/scripts/roster/_archive/20260719-pre-decouple-rewrite/`.
- Each task: pytest green → commit (hanskill #003) before next task.
- UTF-8 source files.

## File Structure

| File | Responsibility |
|------|----------------|
| `ids.py` | Id/alias/skin-label helpers (C1 body; C2 split) |
| `folder_rules.py` | Pure folder/suffix rules (C2) |
| `aliases.py` | LIVE2D_ALIASES + enrich (C2) |
| `schema.py` | Paths, connect, apply_schema, cmd_init |
| `crud.py` | upsert_*, purge skins, seed import |
| `merge.py` | Duplicate merge / folder-like purge |
| `bind_pipeline.py` | All bind_* / repair_* / cmd_repair_l2d_binds |
| `import_wiki.py` | Wiki import |
| `sync.py` | AppData sync / publish / export / verify |
| `cli.py` | argparse main |
| `db.py` | Re-export hub (incl. `_`-prefixed names) |
| `bind.py` etc. | Point at real modules (or keep via db) |

---

### Task 1: C1 — Shear monolith into modules (behavior-identical)

**Files:**
- Create/overwrite: `hanimport/scripts/roster/{ids,schema,crud,merge,bind_pipeline,import_wiki,sync,cli}.py` with real bodies
- Modify: `hanimport/scripts/roster/db.py` → re-export hub
- Modify: thin `bind.py` → re-export from `bind_pipeline` (optional if db hub covers tests)
- Test: `hanimport/tests` bind/import suite

**Interfaces:**
- Consumes: archived section markers in current `db.py`
- Produces: same public/private symbols on `roster.db` as before

- [ ] **Step 1:** Run extraction script: split by `# --- ids|schema|crud|merge|bind|import_wiki|sync|cli ---`
- [ ] **Step 2:** Wire cross-imports (`ids`←none; `schema`←none; `crud`←ids+schema; `merge`←ids+schema+crud; `bind_pipeline`←ids+schema+crud; …)
- [ ] **Step 3:** `db.py` re-exports all names including `_foo` via explicit copy-from-modules loop
- [ ] **Step 4:** `cd hanimport && python -m pytest tests -q --tb=line` (or at least bind/import/oath/variant/absent)
- [ ] **Step 5:** Commit `refactor(hanimport): shear roster db.py into section modules (C1)`

---

### Task 2: C2 — Peel `folder_rules` + `aliases` (no cycles)

**Files:**
- Create: `folder_rules.py`, `aliases.py`
- Modify: `ids.py` to re-export or thin-wrap
- Test: bind resolve / pet folder / oath / younv tests

- [ ] **Step 1:** Move pure strip/suffix/META/younv/idol helpers → `folder_rules.py`
- [ ] **Step 2:** Move `LIVE2D_ALIASES` + enrich/redirect → `aliases.py`
- [ ] **Step 3:** Ensure neither imports `db` / `bind_pipeline`
- [ ] **Step 4:** pytest bind suite; commit

---

### Task 3: C3 — `bind_pipeline` sole bind entry; slim AB import

**Files:**
- Modify: `hanimport/scripts/web/import_ab_bind.py`, `bind.py`
- Test: `test_import_ab_bind.py` + `npm run roster:repair-l2d` smoke

- [ ] **Step 1:** `import_ab_bind` calls `bind_pipeline` APIs only
- [ ] **Step 2:** pytest + repair-l2d smoke; commit

---

### Task 4: C4 — Confirm wiki/merge/sync/cli are sole owners

**Files:** thin wrappers already point at modules; delete any leftover bodies in wrong files

- [ ] **Step 1:** Grep that cmd_* live only in owning modules
- [ ] **Step 2:** pytest import/sync-related; commit

---

### Task 5: C5 — Finalize hub + docs

**Files:**
- Modify: `docs/guides/pet-folder-bind-rules.md` (module index)
- Modify: design spec status → implemented
- Test: full `hanimport/tests`

- [ ] **Step 1:** Docs index to `folder_rules` / `bind_pipeline`
- [ ] **Step 2:** Full pytest; commit

---

## Progress

- C0 archive + design: done (`821717c`)
- C1+: this plan
