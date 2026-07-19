# Peer Skin Kinds Phase 4 — Character Manifest Authority

> **For agentic workers:** REQUIRED SUB-SKILL: executing-plans / implement task-by-task.

**Goal:** Sync/copy Cubism to disk + bind `kanmusu_dir` on character skins only; stop treating `kanmusu/manifest.json` as the write target for sync; character-first lookups for playback/remarks.

**Status:** Complete.

- [x] Strip kanmusu manifest R/W from sync; copy-only
- [x] `ensure_seeded` → sync if needed + attach
- [x] `list_brief` / `get_detail` / `lookup_skin_detail` character-first
- [x] `build_kanmusu_remark_from_lines` from character lines
- [x] Docs + `cargo check --lib` pass
