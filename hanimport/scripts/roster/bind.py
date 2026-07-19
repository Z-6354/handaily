"""Re-export surface for roster.bind (impl in roster.bind_pipeline)."""
from __future__ import annotations

from roster import bind_pipeline as _bp

resolve_bind_skin_id = getattr(_bp, "resolve_bind_skin_id")
bind_unpacked_models = getattr(_bp, "bind_unpacked_models")
ensure_skin_for_unpacked = getattr(_bp, "ensure_skin_for_unpacked")
reclaim_l2d_orphan_into_skin = getattr(_bp, "reclaim_l2d_orphan_into_skin")
repair_l2d_folder_orphans = getattr(_bp, "repair_l2d_folder_orphans")
repair_misindexed_wiki_folder_binds = getattr(_bp, "repair_misindexed_wiki_folder_binds")
repair_blhx_skin_folder_binds = getattr(_bp, "repair_blhx_skin_folder_binds")
cmd_repair_l2d_binds = getattr(_bp, "cmd_repair_l2d_binds")
_build_cn_to_slug = getattr(_bp, "_build_cn_to_slug")
_resolve_character_id = getattr(_bp, "_resolve_character_id")
_bind_paths_for_folder = getattr(_bp, "_bind_paths_for_folder")
