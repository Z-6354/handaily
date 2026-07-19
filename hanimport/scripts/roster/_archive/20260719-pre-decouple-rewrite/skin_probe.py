#!/usr/bin/env python3
"""Probe local Spine (pet) / Cubism (kanmusu) assets for roster skins."""
from __future__ import annotations

import threading
import time
from pathlib import Path

STATUS_UNBOUND = "unbound"
STATUS_MISSING = "missing"
STATUS_PARTIAL = "partial"
STATUS_READY = "ready"
# Default / oath: no Cubism by design — empty kanmusu_dir → absent (不存在), not unbound
STATUS_ABSENT = "absent"

_probe_lock = threading.Lock()
_pet_cache: dict[str, tuple[float, dict]] = {}
_km_cache: dict[str, tuple[float, dict]] = {}
_PROBE_TTL_SEC = 60.0
_resolved_km_root: Path | None = None


def _repo_root() -> Path:
    here = Path(__file__).resolve()
    # hanimport/scripts/roster -> repo
    return here.parents[3]


def default_live2d_roots() -> list[Path]:
    """Desktop-pet / SD model roots (`data/pet`; game AB ``char`` maps here)."""
    import os

    from common.path_policy import default_pet

    roots: list[Path] = []
    for key in ("HANDAILY_PET_PATH", "HANDAILY_CHAR_PATH"):
        env = os.environ.get(key, "").strip()
        if env:
            p = Path(env)
            if p not in roots:
                roots.append(p)
    root = _repo_root()
    for rel in ("data/pet", "data/char", "hanpet/bundled/roster/pet-models"):
        p = root / rel
        if p.is_dir() and p not in roots:
            roots.append(p)
    if not roots:
        roots.append(default_pet())
    return roots


def default_kanmusu_root() -> Path:
    """Kanmusu / Cubism root (`data/skin`; game AB ``live2d`` maps here)."""
    import os

    from common.path_policy import default_skin

    for key in ("HANDAILY_SKIN_PATH", "HANDAILY_LIVE2D_PATH", "HANDAILY_MODEL_UNPACKED"):
        env = os.environ.get(key, "").strip()
        if env:
            return Path(env)
    root = _repo_root()
    for rel in ("data/skin", "data/live2d", "data/model/unpacked"):
        p = root / rel
        if p.is_dir():
            return p
    return default_skin()


def invalidate_probe_cache() -> None:
    global _resolved_km_root
    with _probe_lock:
        _pet_cache.clear()
        _km_cache.clear()
        _resolved_km_root = None


def _cache_get(cache: dict[str, tuple[float, dict]], key: str) -> dict | None:
    hit = cache.get(key)
    if not hit:
        return None
    ts, value = hit
    if (time.monotonic() - ts) > _PROBE_TTL_SEC:
        cache.pop(key, None)
        return None
    return value


def _cache_put(cache: dict[str, tuple[float, dict]], key: str, value: dict) -> dict:
    cache[key] = (time.monotonic(), value)
    return value


def _has_spine_assets(folder: Path) -> bool:
    if not folder.is_dir():
        return False
    has_skel = False
    has_spine_json = False
    has_atlas = False
    try:
        entries = folder.iterdir()
    except OSError:
        return False
    for p in entries:
        if not p.is_file():
            continue
        low = p.name.lower()
        if low.endswith(".skel"):
            has_skel = True
        elif low.endswith(".atlas"):
            has_atlas = True
        elif low.endswith(".json") and low not in {
            "config.json",
            ".config.json",
        } and not low.endswith(".meta.json"):
            has_spine_json = True
        if has_atlas and (has_skel or has_spine_json):
            return True
    return has_atlas and (has_skel or has_spine_json)


def _is_cubism_file(name: str) -> bool:
    low = name.lower()
    return low.endswith(".model3.json") or low.endswith(".moc3") or low.endswith(".moc")


def _has_cubism_assets(folder: Path) -> bool:
    """Fast Cubism presence check — avoid deep rglob on large unpack trees."""
    if not folder.is_dir():
        return False
    try:
        for p in folder.iterdir():
            if p.is_file() and _is_cubism_file(p.name):
                return True
        for p in folder.iterdir():
            if not p.is_dir():
                continue
            try:
                for child in p.iterdir():
                    if child.is_file() and _is_cubism_file(child.name):
                        return True
            except OSError:
                continue
    except OSError:
        return False
    return False


def _kanmusu_base_resolved(root: Path | None) -> Path:
    global _resolved_km_root
    base = root if root is not None else default_kanmusu_root()
    if root is None:
        with _probe_lock:
            if _resolved_km_root is not None:
                return _resolved_km_root
        try:
            resolved = base.resolve()
        except OSError:
            resolved = base
        with _probe_lock:
            _resolved_km_root = resolved
            return _resolved_km_root
    try:
        return base.resolve()
    except OSError:
        return base


def probe_pet(
    pet_model_id: str | None,
    *,
    roots: list[Path] | None = None,
) -> dict:
    mid = (pet_model_id or "").strip()
    if not mid:
        return {"status": STATUS_UNBOUND, "path": None}
    from common.unpack_complete import is_hx_slug

    if is_hx_slug(mid):
        # Harmonized variants are ignored — never probe disk
        return {"status": STATUS_UNBOUND, "path": None}
    if ".." in mid or "/" in mid or "\\" in mid:
        return {"status": STATUS_MISSING, "path": None}

    with _probe_lock:
        cached = _cache_get(_pet_cache, mid)
    if cached is not None:
        return dict(cached)

    root_list = roots if roots is not None else default_live2d_roots()
    for root in root_list:
        cand = root / mid
        if cand.is_dir():
            from common.unpack_complete import is_unpack_complete

            if is_unpack_complete(cand, mid):
                out = {"status": STATUS_READY, "path": str(cand)}
                with _probe_lock:
                    return dict(_cache_put(_pet_cache, mid, out))
            if _has_spine_assets(cand):
                # atlas/skel present but page PNGs incomplete → partial
                out = {"status": STATUS_PARTIAL, "path": str(cand)}
                with _probe_lock:
                    return dict(_cache_put(_pet_cache, mid, out))
            out = {"status": STATUS_MISSING, "path": str(cand)}
            with _probe_lock:
                return dict(_cache_put(_pet_cache, mid, out))
    # Prefer primary data/live2d path for display when absent
    primary = root_list[0] / mid
    out = {"status": STATUS_MISSING, "path": str(primary)}
    with _probe_lock:
        return dict(_cache_put(_pet_cache, mid, out))


def probe_kanmusu(
    kanmusu_dir: str | None,
    *,
    root: Path | None = None,
) -> dict:
    rel = (kanmusu_dir or "").strip()
    if not rel:
        return {"status": STATUS_UNBOUND, "path": None}
    from common.unpack_complete import is_hx_slug

    # folder name or last path segment ending in _hx
    leaf = Path(rel.replace("\\", "/")).name
    if is_hx_slug(leaf) or is_hx_slug(rel):
        return {"status": STATUS_UNBOUND, "path": None}
    # Allow nested relative paths but block traversal
    parts = Path(rel.replace("\\", "/")).parts
    if any(p == ".." for p in parts) or (parts and parts[0] in ("/", "")):
        return {"status": STATUS_MISSING, "path": None}

    cache_key = rel.lower()
    with _probe_lock:
        cached = _cache_get(_km_cache, cache_key)
    if cached is not None:
        return dict(cached)

    base = root if root is not None else default_kanmusu_root()
    base_resolved = _kanmusu_base_resolved(root)
    cand = base.joinpath(*parts)
    try:
        cand.resolve().relative_to(base_resolved)
    except (ValueError, OSError):
        out = {"status": STATUS_MISSING, "path": None}
        with _probe_lock:
            return dict(_cache_put(_km_cache, cache_key, out))
    if _has_cubism_assets(cand):
        out = {"status": STATUS_READY, "path": str(cand), "engine": "cubism"}
    elif _has_spine_assets(cand):
        from common.unpack_complete import is_unpack_complete

        leaf = Path(rel.replace("\\", "/")).name
        if is_unpack_complete(cand, leaf):
            out = {"status": STATUS_READY, "path": str(cand), "engine": "spine"}
        else:
            out = {"status": STATUS_PARTIAL, "path": str(cand), "engine": "spine"}
    elif cand.exists():
        out = {"status": STATUS_MISSING, "path": str(cand)}
    else:
        out = {"status": STATUS_MISSING, "path": str(cand)}
    with _probe_lock:
        return dict(_cache_put(_km_cache, cache_key, out))


def is_kanmusu_optional_slot(skin: dict) -> bool:
    """True for default / oath skins: Cubism not required (pet-only by design)."""
    sid = str(skin.get("id") or "").strip()
    if sid.endswith("-oath") or sid.endswith("-default"):
        return True
    raw = skin.get("is_default")
    if raw in (True, 1, "1", "true", "True"):
        return True
    try:
        return int(raw or 0) == 1
    except (TypeError, ValueError):
        return False


def resolve_kanmusu_display_status(skin: dict, probe_status: str) -> str:
    """Default / oath: Cubism optional — only ready/partial show; else absent (不存在).

    Unbound or missing-on-disk both mean「设计上没有舰娘」, not「未绑定/缺文件」.
    """
    if not is_kanmusu_optional_slot(skin):
        return probe_status
    if probe_status in (STATUS_READY, STATUS_PARTIAL):
        return probe_status
    return STATUS_ABSENT


def enrich_skin(skin: dict, *, live2d_roots=None, kanmusu_root=None) -> dict:
    out = dict(skin)
    # Skip disk probe entirely for harmonized skins
    try:
        from roster.db import is_hx_skin

        if is_hx_skin(
            skin_id=str(skin.get("id") or ""),
            kanmusu_dir=str(skin.get("kanmusu_dir") or ""),
            name_zh=str(skin.get("name_zh") or ""),
        ):
            out["pet_status"] = STATUS_UNBOUND
            out["pet_path"] = None
            out["kanmusu_status"] = STATUS_UNBOUND
            out["kanmusu_path"] = None
            out["kanmusu_engine"] = None
            out["hx_skipped"] = True
            return out
    except Exception:  # noqa: BLE001 — keep probe usable without roster_db
        pass
    pet = probe_pet(skin.get("pet_model_id"), roots=live2d_roots)
    km = probe_kanmusu(skin.get("kanmusu_dir"), root=kanmusu_root)
    out["pet_status"] = pet["status"]
    out["pet_path"] = pet["path"]
    out["kanmusu_status"] = resolve_kanmusu_display_status(skin, km["status"])
    out["kanmusu_path"] = km["path"]
    out["kanmusu_engine"] = km.get("engine")
    return out
