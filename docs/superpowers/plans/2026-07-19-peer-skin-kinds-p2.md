# Peer Skin Kinds Phase 2 — Cubism in Pet Window

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 桌宠 Tab 点选播 Spine；舰娘 Tab 点选在同一桌宠窗播 Cubism（`companion: "kanmusu"`）。

**Architecture:** Keep one skin slot + kind tabs from P1. Branch only the select path: `spine` vs `kanmusu` IPC. Reuse existing `characters_set_skin` / `pet_menu_switch_skin` / `kanmusu::desktop_open`. No schema change.

**Tech Stack:** hanpet React/TS + existing Tauri companion engine.

## Global Constraints

- Same pet window host; no restoring CharacterPetSettings「舰娘上桌」旁路.
- Kanmusu list selectable when `kanmusu_ready` (even if Spine incomplete).
- Spine list still requires `model_ready`.
- Spec: `docs/superpowers/specs/2026-07-19-peer-skin-kinds-design.md` §0.4 P2 / §9.

---

### Task 1: CharacterSkinPicker kind → companion

**Files:**
- Modify: `hanpet/src/components/CharacterSkinPicker.tsx`

- [x] `SkinCard` for `kind=kanmusu`: incomplete when `!kanmusu_ready`; clickable when ready
- [x] `pick()`: spine → `"spine"`; kanmusu → `"kanmusu"`
- [x] Peer badge copy

### Task 2: Pet menu dual preferEngine

**Files:**
- Modify: `hanpet/src/pet/menu.ts`

- [x] Kanmusu list: `preferEngine: "kanmusu"`, disabled on `!kanmusu_ready`
- [x] Spine list: `preferEngine: "spine"`

### Task 3: Docs + verify

- [x] Help copy
- [x] Design status Phase 1–2
- [x] `test:skin-kind` + `build:fe` pass
- [x] Commit

**Status:** P2 complete. P3+ separate.
