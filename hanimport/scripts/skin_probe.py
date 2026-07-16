#!/usr/bin/env python3
"""Probe local Spine (pet) / Cubism (kanmusu) assets for roster skins."""
from __future__ import annotations

from pathlib import Path

STATUS_UNBOUND = "unbound"
STATUS_MISSING = "missing"
STATUS_READY = "ready"


def _repo_root() -> Path:
    here = Path(__file__).resolve().parent
    # hanimport/scripts -> repo
    return here.parent.parent


def default_live2d_roots() -> list[Path]:
    env = __import__("os").environ.get("HANDAILY_LIVE2D_PATH", "").strip()
    roots: list[Path] = []
    if env:
        roots.append(Path(env))
    root = _repo_root()
    for rel in ("data/live2d", "live2d", "hanpet/bundled/roster/pet-models"):
        p = root / rel
        if p.is_dir() and p not in roots:
            roots.append(p)
    if not roots:
        roots.append(root / "data/live2d")
    return roots


def default_kanmusu_root() -> Path:
    env = __import__("os").environ.get("HANDAILY_MODEL_UNPACKED", "").strip()
    if env:
        return Path(env)
    return _repo_root() / "data/model/unpacked"


def _has_spine_assets(folder: Path) -> bool:
    if not folder.is_dir():
        return False
    has_skel = False
    has_spine_json = False
    has_atlas = False
    for p in folder.iterdir():
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
    return has_atlas and (has_skel or has_spine_json)


def _has_cubism_assets(folder: Path) -> bool:
    if not folder.is_dir():
        return False
    for p in folder.rglob("*"):
        if not p.is_file():
            continue
        low = p.name.lower()
        if low.endswith(".model3.json") or p.suffix.lower() in {".moc3", ".moc"}:
            return True
    return False


def probe_pet(
    pet_model_id: str | None,
    *,
    roots: list[Path] | None = None,
) -> dict:
    mid = (pet_model_id or "").strip()
    if not mid:
        return {"status": STATUS_UNBOUND, "path": None}
    if ".." in mid or "/" in mid or "\\" in mid:
        return {"status": STATUS_MISSING, "path": None}
    for root in roots if roots is not None else default_live2d_roots():
        cand = root / mid
        if _has_spine_assets(cand):
            return {"status": STATUS_READY, "path": str(cand)}
        if cand.is_dir():
            return {"status": STATUS_MISSING, "path": str(cand)}
    # Prefer primary data/live2d path for display when absent
    primary = (roots or default_live2d_roots())[0] / mid
    return {"status": STATUS_MISSING, "path": str(primary)}


def probe_kanmusu(
    kanmusu_dir: str | None,
    *,
    root: Path | None = None,
) -> dict:
    rel = (kanmusu_dir or "").strip()
    if not rel:
        return {"status": STATUS_UNBOUND, "path": None}
    # Allow nested relative paths but block traversal
    parts = Path(rel.replace("\\", "/")).parts
    if any(p == ".." for p in parts) or (parts and parts[0] in ("/", "")):
        return {"status": STATUS_MISSING, "path": None}
    base = root if root is not None else default_kanmusu_root()
    cand = base.joinpath(*parts)
    try:
        cand.resolve().relative_to(base.resolve())
    except (ValueError, OSError):
        return {"status": STATUS_MISSING, "path": None}
    if _has_cubism_assets(cand):
        return {"status": STATUS_READY, "path": str(cand)}
    if cand.exists():
        return {"status": STATUS_MISSING, "path": str(cand)}
    return {"status": STATUS_MISSING, "path": str(cand)}


def enrich_skin(skin: dict, *, live2d_roots=None, kanmusu_root=None) -> dict:
    out = dict(skin)
    pet = probe_pet(skin.get("pet_model_id"), roots=live2d_roots)
    km = probe_kanmusu(skin.get("kanmusu_dir"), root=kanmusu_root)
    out["pet_status"] = pet["status"]
    out["pet_path"] = pet["path"]
    out["kanmusu_status"] = km["status"]
    out["kanmusu_path"] = km["path"]
    return out
