"""Aggregate roster character skin counts + Cubism/Spine probe status."""
from __future__ import annotations

import sqlite3
from pathlib import Path
from typing import Any

from skin_probe import (
    STATUS_MISSING,
    STATUS_READY,
    STATUS_UNBOUND,
    default_kanmusu_root,
    default_live2d_roots,
    probe_kanmusu,
    probe_pet,
)

_STATUS_RANK = {
    STATUS_UNBOUND: 0,
    STATUS_MISSING: 1,
    STATUS_READY: 2,
}


def best_asset_status(statuses: list[str]) -> str:
    """Pick best status among skins: ready > missing > unbound."""
    if not statuses:
        return STATUS_UNBOUND
    best = STATUS_UNBOUND
    for s in statuses:
        key = s if s in _STATUS_RANK else STATUS_UNBOUND
        if _STATUS_RANK[key] > _STATUS_RANK[best]:
            best = key
    return best


def dir_mtime(path: Path | None) -> float | None:
    if path is None:
        return None
    try:
        p = Path(path)
        if p.is_dir():
            return float(p.stat().st_mtime)
        if p.is_file():
            return float(p.stat().st_mtime)
    except OSError:
        return None
    return None


def aggregate_character_assets(
    conn: sqlite3.Connection,
    character_id: str,
    *,
    live2d_roots: list[Path] | None = None,
    kanmusu_root: Path | None = None,
) -> dict[str, Any]:
    """Return skin_count, pet_status, kanmusu_status, import_mtime for one character."""
    roots = live2d_roots if live2d_roots is not None else default_live2d_roots()
    km_root = kanmusu_root if kanmusu_root is not None else default_kanmusu_root()
    skins = conn.execute(
        "SELECT pet_model_id, kanmusu_dir FROM skins WHERE character_id=?",
        (character_id,),
    ).fetchall()
    pet_statuses: list[str] = []
    km_statuses: list[str] = []
    mtimes: list[float] = []
    for sk in skins:
        if isinstance(sk, sqlite3.Row):
            pet_id = sk["pet_model_id"]
            km_dir = sk["kanmusu_dir"]
        else:
            pet_id, km_dir = sk[0], sk[1]
        pet = probe_pet(pet_id, roots=roots)
        km = probe_kanmusu(km_dir, root=km_root)
        pet_statuses.append(pet["status"])
        km_statuses.append(km["status"])
        for st, path in ((pet["status"], pet["path"]), (km["status"], km["path"])):
            if st == STATUS_READY and path:
                mt = dir_mtime(Path(path))
                if mt is not None:
                    mtimes.append(mt)
    return {
        "skin_count": len(skins),
        "pet_status": best_asset_status(pet_statuses),
        "kanmusu_status": best_asset_status(km_statuses),
        "import_mtime": max(mtimes) if mtimes else None,
    }
