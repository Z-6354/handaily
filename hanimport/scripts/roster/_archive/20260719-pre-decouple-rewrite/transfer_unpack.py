"""Scan data/transfer batches → classify pet/live2d → unpack if needed."""
from __future__ import annotations

from pathlib import Path
from typing import Any, Literal

from common.path_policy import (
    default_pet,
    default_skin,
    model_folder_name,
    repo_root,
)
from common.unpack_complete import is_hx_slug, is_unpack_complete
from unpack.unpack_bundle import (
    detect_kind,
    infer_slug,
    is_unity_bundle,
    unpack_one,
)

import UnityPy

Track = Literal["pet", "live2d"]

_SKIP_BATCH_NAMES = frozenset({"history", "temp", "outbox"})


def transfer_root() -> Path:
    return repo_root() / "data" / "transfer"


def list_batch_dirs(root: Path | None = None, batch: str | None = None) -> list[Path]:
    root = root or transfer_root()
    if not root.is_dir():
        return []
    if batch:
        p = root / batch.strip()
        return [p] if p.is_dir() else []
    out: list[Path] = []
    for child in sorted(root.iterdir()):
        if not child.is_dir():
            continue
        if child.name.lower() in _SKIP_BATCH_NAMES or child.name.startswith("."):
            continue
        out.append(child)
    return out


def classify_track(path: Path) -> Track | None:
    """Rule A: path hint first (game ``char`` → pet), else bundle content."""
    norm = str(path).replace("\\", "/").lower()
    parts = norm.split("/")
    # Game AssetBundles/char (SD) and local pet → data/pet
    if "char" in parts or "pet" in parts or "spinepainting" in parts:
        return "pet"
    if "live2d" in parts:
        return "live2d"
    try:
        if not is_unity_bundle(path):
            return None
        env = UnityPy.load(str(path))
        kind = detect_kind(env)
    except Exception:  # noqa: BLE001
        return None
    if kind == "live2d":
        return "live2d"
    if kind == "spine":
        return "pet"
    return None


def discover_transfer_bundles(batch_dirs: list[Path]) -> list[dict[str, str]]:
    """Find AB files; key by (track, slug); prefer *_res on collision."""
    by_key: dict[tuple[str, str], dict[str, str]] = {}

    def _is_res(p: Path) -> bool:
        stem = (p.stem if p.suffix else p.name).lower()
        return stem.endswith("_res")

    for batch in batch_dirs:
        for path in batch.rglob("*"):
            if not path.is_file():
                continue
            name = path.name
            if name.startswith(".") or name.endswith(".part"):
                continue
            if not is_unity_bundle(path):
                continue
            slug = infer_slug(path)
            if is_hx_slug(slug):
                continue
            track = classify_track(path)
            key_track = track or "?"
            key = (key_track, slug)
            item = {
                "path": str(path.resolve()),
                "slug": slug,
                "batch": batch.name,
                "track": track or "",
            }
            prev = by_key.get(key)
            if prev is None:
                by_key[key] = item
                continue
            if _is_res(path) and not _is_res(Path(prev["path"])):
                by_key[key] = item
    return list(by_key.values())


def _normalize_track(raw: str | None) -> Track | None:
    t = (raw or "").strip().lower()
    if t in ("pet", "char", "spinepainting"):
        return "pet"
    if t == "live2d":
        return "live2d"
    return None


def output_for_track(track: Track, slug: str) -> tuple[Path, Path, str]:
    root = default_pet() if track == "pet" else default_skin()
    folder = model_folder_name(slug)
    return root, root / folder, folder


def process_transfer_unpack(
    *,
    batch: str | None = None,
    dry_run: bool = False,
    transfer_dir: Path | None = None,
) -> dict[str, Any]:
    root = transfer_dir or transfer_root()
    batches = list_batch_dirs(root, batch)
    if batch and not batches:
        return {
            "ok": False,
            "error": f"batch not found: {batch}",
            "transfer_root": str(root),
            "items": [],
        }

    bundles = discover_transfer_bundles(batches)
    items: list[dict[str, Any]] = []
    counts = {"unpack": 0, "skip": 0, "error": 0}

    for b in bundles:
        path = Path(b["path"])
        slug = b["slug"]
        entry: dict[str, Any] = {
            "path": str(path),
            "slug": slug,
            "batch": b.get("batch"),
        }
        track = _normalize_track(b.get("track")) or classify_track(path)
        if track is None:
            entry.update(action="skip", reason="unclassified", track=None)
            counts["skip"] += 1
            items.append(entry)
            continue
        out_root, out_dir, folder = output_for_track(track, slug)
        entry["track"] = track
        entry["output_dir"] = str(out_dir)
        entry["folder"] = folder

        if is_unpack_complete(out_dir, slug):
            entry.update(action="skip", reason="already_unpacked")
            counts["skip"] += 1
            items.append(entry)
            continue

        if dry_run:
            entry.update(action="unpack", reason="dry_run")
            counts["unpack"] += 1
            items.append(entry)
            continue

        try:
            result = unpack_one(path, out_root, slug)
            entry.update(
                action="unpack",
                reason="ok",
                kind=result.get("kind"),
                files=result.get("files"),
            )
            counts["unpack"] += 1
        except Exception as exc:  # noqa: BLE001
            entry.update(action="error", reason=str(exc))
            counts["error"] += 1
        items.append(entry)

    return {
        "ok": True,
        "transfer_root": str(root.resolve()),
        "batches": [p.name for p in batches],
        "dry_run": dry_run,
        "counts": counts,
        "items": items,
    }
