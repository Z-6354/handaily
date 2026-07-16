#!/usr/bin/env python3
"""Build animations.meta.json + touch_areas.json for Cubism folders under data/model/unpacked.

Animation / Touch names are extracted from the original Unity AssetBundle
(AnimationClip + GameObject Touch*/Hit*). Cubism folders do not get config.json
(ViewerEX Spine); that stays Spine-only under data/live2d.
"""
from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path
from typing import Any

import UnityPy

CLICK = ("tap", "click", "touch", "hit")
IDLE = ("normal", "stand", "idle", "standby", "default", "login")
DRAG = ("tuozhuai", "drag", "move")
RANDOM = ("dance", "sleep", "main_1", "main_2", "wedding", "home", "mail")
TOUCH_NAME = re.compile(r"^(Touch|Hit|Paramtouch)", re.I)

# Align with Azur Lane gamecfg/assistantinfo.lua assistantTouchParts order.
AL_TOUCH_PRIORITY = {"special": 2, "head": 1, "body": 0}
LOGIC_CLICK = {
    "special": "touch_special",
    "head": "touch_head",
    "body": "touch_body",
}
DEFAULT_TOUCH_IDS = ("TouchSpecial", "TouchHead", "TouchBody")


def repo_root() -> Path:
    return Path(__file__).resolve().parents[2]


def pick(anims: list[str], keys: tuple[str, ...]) -> str | None:
    for a in anims:
        low = a.lower()
        if any(k in low for k in keys):
            return a
    return None


def pick_idle(anims: list[str]) -> str | None:
    for k in IDLE:
        for a in anims:
            if a.lower() == k:
                return a
    return pick(anims, IDLE) or (anims[0] if anims else None)


def pick_random(anims: list[str], idle: str | None, click: str | None) -> list[str]:
    out: list[str] = []
    for k in RANDOM:
        for a in anims:
            if a.lower() == k or k in a.lower():
                if a not in out and a != idle and a != click:
                    out.append(a)
    if not out:
        for a in anims:
            if a != idle and a != click and a not in out:
                out.append(a)
            if len(out) >= 3:
                break
    return out[:5]


def extract_from_bundle(bundle: Path) -> tuple[list[str], list[str]]:
    env = UnityPy.load(str(bundle))
    anims: set[str] = set()
    touches: set[str] = set()
    for obj in env.objects:
        try:
            if obj.type.name == "AnimationClip":
                data = obj.read()
                name = getattr(data, "name", None) or getattr(data, "m_Name", None)
                if name and 1 < len(str(name)) < 64:
                    anims.add(str(name))
            elif obj.type.name == "GameObject":
                data = obj.read()
                name = getattr(data, "name", None) or getattr(data, "m_Name", None)
                if name and TOUCH_NAME.search(str(name)):
                    touches.add(str(name))
        except Exception:
            continue
    return sorted(anims), sorted(touches)


def zone_for(name: str) -> str:
    low = name.lower()
    if "head" in low:
        return "head"
    if "special" in low:
        return "special"
    return "body"


def click_for(name: str, anims: list[str], default_click: str | None) -> str | None:
    low = name.lower()
    for a in anims:
        al = a.lower()
        if "head" in low and "touch_head" in al:
            return a
        if "special" in low and "touch_special" in al:
            return a
        if ("body" in low or "idle" in low) and "touch_body" in al:
            return a
    return default_click


def build_meta(anims: list[str]) -> dict[str, Any]:
    idle = pick_idle(anims)
    click = pick(anims, CLICK)
    drag = pick(anims, DRAG)
    randoms = pick_random(anims, idle, click)
    return {
        "animations": anims,
        "idle_animation": idle,
        "click_animation": click,
        "boot_animation": idle,
        "return_idle_animation": idle,
        "drag_animation": drag,
        "random_animations": randoms,
        "random_min_sec": 30,
        "random_max_sec": 120,
        "lines": [],
        "kind": "live2d_cubism",
    }


def build_touch(touches: list[str], anims: list[str], click: str | None) -> dict[str, Any]:
    names = list(touches) if touches else list(DEFAULT_TOUCH_IDS)
    # Keep AL priority order when emitting stubs / mixed lists.
    def sort_key(n: str) -> tuple[int, str]:
        z = zone_for(n)
        return (-AL_TOUCH_PRIORITY.get(z, 0), n.lower())

    areas: list[dict[str, Any]] = []
    seen: set[str] = set()
    for name in sorted(names, key=sort_key):
        key = name.lower()
        if key in seen:
            continue
        seen.add(key)
        zone = zone_for(name)
        logic = LOGIC_CLICK.get(zone, "touch_body")
        anim = click_for(name, anims, click) or logic
        areas.append(
            {
                "id": name,
                "label": name,
                "zone": zone,
                "attachments": [name],
                "priority": AL_TOUCH_PRIORITY.get(zone, 0),
                "click_animation": anim,
            }
        )
    default_click = click or "touch_body"
    return {
        "version": 1,
        "coordinate_space": "drawable",
        "default_click_animation": default_click,
        "areas": areas,
        "logic": {
            "hit_mode": "drawable",
            "mode": "priority_first",
            "on_click_busy": "ignore",
            "description": "AL generic Touch* via Cubism drawable hitTest; bounds unused",
        },
    }


def ensure_model3_hit_areas(model_dir: Path, areas: list[dict[str, Any]]) -> bool:
    """Merge Touch* into model3 HitAreas so pixi-live2d model.hit() returns drawable ids."""
    model3_files = sorted(model_dir.glob("*.model3.json"))
    if not model3_files:
        return False
    path = model3_files[0]
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except Exception:
        return False
    if not isinstance(data, dict):
        return False
    hit_areas = data.get("HitAreas")
    if not isinstance(hit_areas, list):
        hit_areas = []
        data["HitAreas"] = hit_areas
    existing = {
        str(h.get("Id") or h.get("Name") or "").lower()
        for h in hit_areas
        if isinstance(h, dict)
    }
    changed = False
    for area in areas:
        aid = str(area.get("id") or "").strip()
        if not aid or aid.lower() in existing:
            continue
        for att in area.get("attachments") or [aid]:
            name = str(att).strip()
            if not name or name.lower() in existing:
                continue
            hit_areas.append({"Name": name, "Id": name})
            existing.add(name.lower())
            changed = True
    if changed:
        path.write_text(json.dumps(data, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    return changed


def find_bundle(src_dir: Path, slug: str) -> Path | None:
    direct = src_dir / slug
    if direct.is_file():
        return direct
    # extensionless / any suffix
    matches = [p for p in src_dir.iterdir() if p.is_file() and p.name.startswith(slug)]
    return matches[0] if matches else None


def process_slug(
    slug: str,
    unpacked: Path,
    src_dir: Path,
    *,
    force: bool,
    dry_run: bool,
) -> dict[str, Any]:
    model_dir = unpacked / slug
    if not model_dir.is_dir():
        return {"slug": slug, "ok": False, "error": "unpacked folder missing"}
    has_moc = any(p.suffix.lower() == ".moc3" for p in model_dir.iterdir() if p.is_file())
    if not has_moc:
        return {"slug": slug, "ok": False, "error": "not a Cubism folder"}

    meta_path = model_dir / "animations.meta.json"
    touch_path = model_dir / "touch_areas.json"
    if not force and meta_path.is_file() and touch_path.is_file():
        return {"slug": slug, "ok": True, "skipped": True}

    bundle = find_bundle(src_dir, slug)
    if not bundle:
        return {"slug": slug, "ok": False, "error": f"bundle not found in {src_dir}"}

    anims, touches = extract_from_bundle(bundle)
    meta = build_meta(anims)
    touch = build_touch(touches, anims, meta.get("click_animation"))
    result = {
        "slug": slug,
        "ok": True,
        "bundle": str(bundle),
        "animations": len(anims),
        "touch_areas": len(touches),
        "idle": meta.get("idle_animation"),
        "click": meta.get("click_animation"),
    }
    if dry_run:
        result["dry_run"] = True
        return result

    meta_path.write_text(json.dumps(meta, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    touch_path.write_text(json.dumps(touch, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    wrote = ["animations.meta.json", "touch_areas.json"]
    if ensure_model3_hit_areas(model_dir, touch.get("areas") or []):
        wrote.append("model3 HitAreas")
    result["wrote"] = wrote
    return result


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--input",
        type=Path,
        help="Cubism model folder or unpacked root (preferred)",
    )
    parser.add_argument(
        "--unpacked",
        type=Path,
        default=None,
        help="Cubism unpacked root (legacy alias of --input when directory of models)",
    )
    parser.add_argument(
        "--src",
        type=Path,
        default=repo_root() / "data/model/azurlane/custom",
        help="Original AssetBundle directory",
    )
    parser.add_argument("--slug", action="append", default=[], help="Only these slugs (repeatable)")
    parser.add_argument("--force", action="store_true", help="Overwrite existing JSON")
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()

    unpacked: Path = args.input or args.unpacked or (repo_root() / "data/model/unpacked")
    src_dir: Path = args.src
    if not unpacked.exists():
        print(json.dumps({"ok": False, "error": f"input missing: {unpacked}"}, ensure_ascii=False))
        return 1
    if not src_dir.is_dir():
        print(json.dumps({"ok": False, "error": f"src missing: {src_dir}"}, ensure_ascii=False))
        return 1

    # Single Cubism folder
    if unpacked.is_dir() and any(
        p.suffix.lower() == ".moc3" for p in unpacked.iterdir() if p.is_file()
    ):
        slugs = [unpacked.name]
        unpacked_root = unpacked.parent
        r = process_slug(
            unpacked.name,
            unpacked_root,
            src_dir,
            force=args.force,
            dry_run=args.dry_run,
        )
        # process_slug joins unpacked/slug — fix by using parent when input is the model dir
        # Overwrite paths: write into `unpacked` itself when input is the model folder
        if r.get("ok") and not r.get("skipped") and not args.dry_run:
            pass  # already written to parent/slug == input
        print(json.dumps(r, ensure_ascii=False))
        return 0 if r.get("ok") else 1

    unpacked_root = unpacked
    if not unpacked_root.is_dir():
        print(json.dumps({"ok": False, "error": f"not a directory: {unpacked_root}"}, ensure_ascii=False))
        return 1

    slugs = args.slug or sorted(
        p.name for p in unpacked_root.iterdir() if p.is_dir() and not p.name.startswith(".")
    )
    failed = 0
    for slug in slugs:
        r = process_slug(slug, unpacked_root, src_dir, force=args.force, dry_run=args.dry_run)
        print(json.dumps(r, ensure_ascii=False))
        if not r.get("ok"):
            failed += 1
    return 1 if failed else 0


if __name__ == "__main__":
    raise SystemExit(main())
