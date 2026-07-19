"""Thin re-export surface for roster.bind (impl in roster.db)."""
from __future__ import annotations

from roster import db as _db

resolve_bind_skin_id = getattr(_db, 'resolve_bind_skin_id')
bind_unpacked_models = getattr(_db, 'bind_unpacked_models')
ensure_skin_for_unpacked = getattr(_db, 'ensure_skin_for_unpacked')
reclaim_l2d_orphan_into_skin = getattr(_db, 'reclaim_l2d_orphan_into_skin')
repair_l2d_folder_orphans = getattr(_db, 'repair_l2d_folder_orphans')
repair_misindexed_wiki_folder_binds = getattr(_db, 'repair_misindexed_wiki_folder_binds')
repair_blhx_skin_folder_binds = getattr(_db, 'repair_blhx_skin_folder_binds')
cmd_repair_l2d_binds = getattr(_db, 'cmd_repair_l2d_binds')
_build_cn_to_slug = getattr(_db, '_build_cn_to_slug')
_resolve_character_id = getattr(_db, '_resolve_character_id')
_bind_paths_for_folder = getattr(_db, '_bind_paths_for_folder')
