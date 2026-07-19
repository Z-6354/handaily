# Peer Skin Kinds (Phase 1) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Flat 桌宠/舰娘 tabs in skin picker; Spine-only picks; remove buried kanmusu skin controls.

**Architecture:** One skin slot, two UI projections via shared `skinKindFilter`; force `companion: "spine"`; retire CharacterPetSettings kanmusu tab and settings sync entry.

**Tech Stack:** React/TS (hanpet), existing `charactersSkinsPage` IPC.

## Global Constraints

- Phase 1: no Cubism on select; no schema change.
- Do not show internal model/character ids.
- Reuse `pet-tab` / `pet-model-card` styles.

---

### Task 1: Shared kind filter + unit tests

**Files:**
- Create: `hanpet/src/lib/skinKindFilter.ts`
- Create: `hanpet/src/lib/skinKindFilter.test.ts` (if vitest present) or skip

- [x] Export `SkinKind = "spine" | "kanmusu"`, `skinMatchesKind`, `filterSkinsByKind`

### Task 2: CharacterSkinPicker kind tabs

**Files:**
- Modify: `hanpet/src/components/CharacterSkinPicker.tsx`

- [x] Load full skin list for character; filter by kind; client paginate
- [x] Tabs 桌宠/舰娘; remember kind in sessionStorage
- [x] Pick always `"spine"`; kanmusu tab hides redundant 舰娘 badge

### Task 3: Remove kanmusu settings UI

**Files:**
- Modify: `hanpet/src/components/CharacterPetSettings.tsx`
- Modify: `hanpet/src/pages/SettingsPanel.tsx`
- Modify: `hanpet/src/pages/PersonaPanel.tsx`
- Modify: `hanpet/src/lib/helpContent.ts`

- [x] Drop 舰娘皮肤 tab + sync/preview/desktop
- [x] Hide settings 舰娘皮肤 section
- [x] `switchSkin` default `"spine"`
- [x] Help copy update

### Task 4: Verify + commit

- [x] Lint touched files
- [ ] Manual acceptance per spec §7 (user)
- [x] Commit
