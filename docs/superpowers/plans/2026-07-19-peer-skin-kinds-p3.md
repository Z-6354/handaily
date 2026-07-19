# Peer Skin Kinds Phase 3 — Kind Preference

> **For agentic workers:** Implement per-character skin kind tab memory.

**Goal:** Remember 桌宠/舰娘 tab per character; align cold open with companion engine when no preference.

**Status:** Complete (lightweight localStorage; full DB `active_kind` deferred).

- [x] `readCharacterSkinKind` / `writeCharacterSkinKind`
- [x] Picker restore + `petGetCompanionEngine` fallback
- [x] Persist on tab change and on pick
