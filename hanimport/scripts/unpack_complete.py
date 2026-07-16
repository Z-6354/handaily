"""Detect complete vs half-finished unpack output dirs.

Complete (skip):
  Live2D: {slug}.model3.json + {slug}.moc3 (non-empty)
  Spine:  {slug}.atlas + ({slug}.skel or {slug}.skel.bytes) (non-empty)

Incomplete: dir missing markers → delete before re-unpack.

Hx variants (*_hx): never unpack; purge leftover output dirs.
"""
from __future__ import annotations

import shutil
from pathlib import Path


def is_hx_slug(slug: str) -> bool:
    """True when slug ends with _hx (case-insensitive)."""
    s = (slug or "").strip().lower()
    return bool(s) and s.endswith("_hx")


def purge_hx_output_dirs(output_root: Path) -> list[str]:
    """Remove immediate child dirs of output_root whose names end with _hx."""
    if not output_root.is_dir():
        return []
    removed: list[str] = []
    for child in list(output_root.iterdir()):
        if child.is_dir() and is_hx_slug(child.name):
            shutil.rmtree(child)
            removed.append(child.name)
    return removed


def _non_empty(path: Path) -> bool:
    try:
        return path.is_file() and path.stat().st_size > 0
    except OSError:
        return False


def is_unpack_complete(out_dir: Path, slug: str | None = None) -> bool:
    """Return True only when core model files are present (half-finished → False)."""
    if not out_dir.is_dir():
        return False
    name = slug or out_dir.name
    moc3 = out_dir / f"{name}.moc3"
    model3 = out_dir / f"{name}.model3.json"
    if _non_empty(moc3) and _non_empty(model3):
        return True
    atlas = out_dir / f"{name}.atlas"
    skel = out_dir / f"{name}.skel"
    skel_bytes = out_dir / f"{name}.skel.bytes"
    if _non_empty(atlas) and (_non_empty(skel) or _non_empty(skel_bytes)):
        return True
    return False


def prepare_unpack_dir(out_dir: Path, slug: str | None = None) -> str:
    """Prepare output dir for unpack.

    Returns:
      'skip'   — already complete, do not unpack
      'ready'  — missing or was incomplete (deleted); safe to unpack
    """
    name = slug or out_dir.name
    if is_unpack_complete(out_dir, name):
        return "skip"
    if out_dir.exists():
        shutil.rmtree(out_dir)
    return "ready"
