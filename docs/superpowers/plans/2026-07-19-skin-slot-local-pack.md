# Skin-Slot Local Pack/Unpack Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement local `.slot.zip` pack + unpack round-trip from roster sqlite + `data/pet`/`data/skin` (phase ① only; no server upload, no hanpet).

**Architecture:** New `roster/skin_slot_pack.py` owns format constants, eligibility checks, zip build, and extract-to-root. CLI via thin wrapper or `python -m` style script under `hanimport/scripts`. Reuse `skin_probe._has_spine_assets` / `_has_cubism_assets` and `ids.is_oath_skin_id`.

**Tech Stack:** Python 3, zipfile, sqlite3, pytest, existing hanimport `scripts/` path layout.

## Global Constraints

- Format name: `handaily-skin-slot`, `format_version: 1`
- Filename: `{character_id}__{skin_id}.slot.zip`
- Pet required; skin optional; unbound / pet-missing / skin-only → skip with reason
- `lines.json` fields: wiki_key, label, lang, text, animation, sort_order (no audio)
- No network; no zip password
- Spec: `docs/superpowers/specs/2026-07-19-skin-slot-distribution-design.md`

## File Structure

| File | Responsibility |
|------|----------------|
| `hanimport/scripts/roster/skin_slot_pack.py` | validate, pack_one, unpack_one, pack_many |
| `hanimport/scripts/roster_skin_slot_pack.py` | CLI entry (`pack` / `unpack`) |
| `hanimport/tests/test_skin_slot_pack.py` | unit + round-trip fixtures |
| `package.json` (optional) | `roster:pack-slot` script later if needed |

---

### Task 1: Eligibility + manifest builders (TDD)

**Files:**
- Create: `hanimport/scripts/roster/skin_slot_pack.py`
- Test: `hanimport/tests/test_skin_slot_pack.py`

**Interfaces:**
- Produces:
  - `FORMAT = "handaily-skin-slot"`
  - `slot_zip_name(character_id: str, skin_id: str) -> str`
  - `class SkipReason`: code + message
  - `check_slot_eligible(conn, skin_row, *, pet_root, skin_root) -> SkipReason | None`
  - `build_manifest(character_row, skin_row, *, has_pet, has_kanmusu, packed_at) -> dict`
  - `lines_from_db(conn, skin_id) -> list[dict]`

- [ ] **Step 1:** Write failing tests for zip name, skip rules (no pet_model_id, missing pet dir, skin-only), manifest shape, lines export
- [ ] **Step 2:** Run pytest — expect fail
- [ ] **Step 3:** Implement helpers using `skin_probe._has_spine_assets` / `_has_cubism_assets`, `is_oath_skin_id`
- [ ] **Step 4:** Pytest pass; commit

---

### Task 2: Pack zip to disk

**Files:**
- Modify: `skin_slot_pack.py`
- Test: `test_skin_slot_pack.py`

**Interfaces:**
- Produces: `pack_slot(conn, skin_id, *, pet_root, skin_root, avatar_dir, out_dir) -> Path | SkipResult`

- [ ] **Step 1:** Failing test: tiny fake pet(+optional skin) dirs → zip contains manifest.json, lines.json, pet/…, optional skin/, optional avatar
- [ ] **Step 2:** Implement zip write (ZIP_DEFLATED ok locally)
- [ ] **Step 3:** Pytest pass; commit

---

### Task 3: Unpack + round-trip

**Files:**
- Modify: `skin_slot_pack.py`
- Test: `test_skin_slot_pack.py`

**Interfaces:**
- Produces: `unpack_slot(zip_path, *, dest_root) -> dict` writing `dest_root/pet|skin|avatars` + returning manifest

- [ ] **Step 1:** Failing round-trip test pack→unpack→spine probe true; manifest equal
- [ ] **Step 2:** Implement safe extract (reject `..`)
- [ ] **Step 3:** Pytest pass; commit

---

### Task 4: CLI for local batch

**Files:**
- Create: `hanimport/scripts/roster_skin_slot_pack.py`
- Optional: `package.json` script

**Interfaces:**
- CLI: `pack --db … --out … --ids skin1,skin2` and `unpack --zip … --dest …`

- [ ] **Step 1:** Smoke CLI on fixture
- [ ] **Step 2:** Commit

---

### Task 5: Mark phase ① done in spec status

- [ ] Update design status to「阶段①已实现」; commit docs

---

## Out of scope (later plans)

- Upload / TUS / wannian.fun
- hanpet portable import / auto-update
- Download UI
