#!/usr/bin/env python3
"""Build hanpet-compatible Spine model JSON configs from unpacked folders.

Outputs (per model folder):
  - config.json           ViewerEX type=9 skeleton config
  - animations.meta.json    idle / click / random actions (hanpet import template)
  - touch_areas.json        click regions inferred from atlas attachments
"""
from __future__ import annotations

import argparse
import json
import re
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any

ANIM_KEYWORDS = (
    "normal",
    "stand",
    "idle",
    "touch",
    "tap",
    "click",
    "dance",
    "sleep",
    "attack",
    "move",
    "skill",
    "login",
    "wedding",
    "complete",
    "main",
    "mission",
    "dead",
    "victory",
    "motou",
    "tuozhuai",
    "drag",
    "special",
    "stand2",
    "hurt",
    "win",
)

IDLE_PREFS = ("normal", "stand", "idle", "standby", "default")
CLICK_KEYS = ("tap", "click", "touch", "hit")
DRAG_KEYS = ("tuozhuai", "drag", "move")
RANDOM_TEMPLATE = ("dance", "sleep")

HEAD_KEYS = ("head", "face", "hair", "eye", "mouth", "cheek", "brow", "ear")
BODY_KEYS = ("body", "breast", "chest", "hip", "waist", "skirt", "cloth")
HAND_KEYS = ("hand", "arm", "finger")
LEG_KEYS = ("leg", "foot", "shoe", "thigh")


@dataclass
class SpineTriple:
    skel: str
    atlas: str
    png: str


@dataclass
class AtlasRegion:
    name: str
    x: int
    y: int
    width: int
    height: int


def repo_root() -> Path:
    return Path(__file__).resolve().parents[2]


def action_template_path() -> Path:
    return repo_root() / "hanpet" / "bundled" / "pet-action-template.json"


def read_varint(data: bytes, i: int) -> tuple[int, int]:
    b = data[i]
    i += 1
    result = b & 0x7F
    shift = 7
    while (b & 0x80) and i < len(data):
        b = data[i]
        i += 1
        result |= (b & 0x7F) << shift
        shift += 7
    return result, i


def read_spine_string(data: bytes, i: int) -> tuple[str, int]:
    ln, i = read_varint(data, i)
    if ln == 0:
        return "", i
    if ln < 0:
        ln = -ln
    raw = data[i : i + max(0, ln - 1)]
    return raw.decode("utf-8", "replace"), i + ln


def extract_spine_strings(data: bytes) -> set[str]:
    out: set[str] = set()
    pos = 0
    while pos < len(data) - 1:
        try:
            s, _ = read_spine_string(data, pos)
            if 2 <= len(s) <= 48 and re.fullmatch(r"[A-Za-z0-9_]+", s):
                out.add(s)
            pos += 1
        except Exception:
            pos += 1
    return out


def parse_atlas_regions(path: Path) -> tuple[list[AtlasRegion], int, int]:
    text = path.read_text(encoding="utf-8", errors="replace")
    lines = text.splitlines()
    page_w = page_h = 0
    for line in lines:
        if line.startswith("size:"):
            parts = line.split(":", 1)[1].strip().split(",")
            if len(parts) == 2:
                page_w, page_h = int(parts[0].strip()), int(parts[1].strip())
            break

    regions: list[AtlasRegion] = []
    i = 0
    while i < len(lines):
        line = lines[i].strip()
        if (
            not line
            or line.endswith(".png")
            or ":" in line
            or line.startswith("size:")
            or line.startswith("format:")
            or line.startswith("filter:")
            or line.startswith("repeat:")
        ):
            i += 1
            continue
        name = line
        x = y = w = h = 0
        j = i + 1
        while j < len(lines) and (lines[j].startswith(" ") or lines[j].startswith("\t")):
            part = lines[j].strip()
            if part.startswith("xy:"):
                xy = part.split(":", 1)[1].strip().split(",")
                x, y = int(xy[0].strip()), int(xy[1].strip())
            elif part.startswith("size:"):
                sz = part.split(":", 1)[1].strip().split(",")
                w, h = int(sz[0].strip()), int(sz[1].strip())
            j += 1
        if w > 0 and h > 0:
            regions.append(AtlasRegion(name, x, y, w, h))
        i = j
    return regions, page_w, page_h


def find_single_ext(folder: Path, ext: str) -> str | None:
    matches = sorted(
        p.name
        for p in folder.iterdir()
        if p.is_file() and p.suffix.lower().lstrip(".") == ext
    )
    if len(matches) == 1:
        return matches[0]
    return None


def inspect_spine_folder(folder: Path) -> SpineTriple | None:
    if not folder.is_dir():
        return None
    for cfg_name in ("config.json", ".config.json", "model.json"):
        cfg = folder / cfg_name
        if cfg.is_file():
            try:
                raw = json.loads(cfg.read_text(encoding="utf-8"))
                if raw.get("type") == 9 and raw.get("skeleton"):
                    atl = (raw.get("atlases") or [{}])[0]
                    skel = raw["skeleton"]
                    atlas = atl.get("atlas")
                    png = (atl.get("textures") or [None])[0]
                    if skel and atlas and png:
                        return SpineTriple(skel, atlas, png)
            except json.JSONDecodeError:
                pass
    skel = find_single_ext(folder, "skel")
    atlas = find_single_ext(folder, "atlas")
    png = find_single_ext(folder, "png")
    if skel and atlas and png:
        return SpineTriple(skel, atlas, png)
    return None


def list_animations(skel_path: Path, atlas_path: Path) -> list[str]:
    regions, _, _ = parse_atlas_regions(atlas_path)
    atlas_names = {r.name for r in regions}
    strings = extract_spine_strings(skel_path.read_bytes())
    anims = [
        s
        for s in strings
        if s not in atlas_names and any(k in s.lower() for k in ANIM_KEYWORDS)
    ]
    anims.sort()
    return anims


def pick_idle(anims: list[str], template: dict[str, Any]) -> str | None:
    pref = template.get("idle_animation")
    if isinstance(pref, str):
        for a in anims:
            if a.lower() == pref.lower():
                return a
    for key in IDLE_PREFS:
        for a in anims:
            if a.lower() == key:
                return a
    for key in ("idle", "stand", "normal"):
        for a in anims:
            if key in a.lower():
                return a
    return anims[0] if anims else None


def pick_by_keywords(anims: list[str], keys: tuple[str, ...]) -> str | None:
    for a in anims:
        lower = a.lower()
        if any(k in lower for k in keys):
            return a
    return None


def pick_random(anims: list[str], idle: str | None, template: dict[str, Any]) -> list[str]:
    idle_l = (idle or "").lower()
    reserved = {
        idle_l,
        *(template.get("click_animation", "") or "").lower(),
        *(template.get("boot_animation", "") or "").lower(),
        *(template.get("drag_animation", "") or "").lower(),
    }
    out: list[str] = []
    for name in template.get("random_animations") or RANDOM_TEMPLATE:
        for a in anims:
            if a.lower() == str(name).lower() and a.lower() not in reserved:
                out.append(a)
                break
    if not out:
        for a in anims:
            ll = a.lower()
            if ll in reserved:
                continue
            if any(k in ll for k in IDLE_PREFS + CLICK_KEYS + DRAG_KEYS):
                continue
            out.append(a)
    dedup: list[str] = []
    for a in out:
        if a not in dedup:
            dedup.append(a)
    return dedup


def generate_viewer_ex_config(triple: SpineTriple) -> dict[str, Any]:
    tex_stem = Path(triple.png).stem
    return {
        "conf_ver": 1,
        "type": 9,
        "options": {"tex_type": 0, "edge_padding": False},
        "skeleton": triple.skel,
        "atlases": [
            {
                "atlas": triple.atlas,
                "tex_names": [tex_stem],
                "textures": [triple.png],
            }
        ],
    }


def rewrite_atlas_texture_page(atlas_path: Path, png_name: str) -> bool:
    text = atlas_path.read_text(encoding="utf-8", errors="replace")
    lines = text.splitlines()
    changed = False
    for idx, line in enumerate(lines):
        t = line.strip()
        if t and not t.endswith(".png") and ":" not in t:
            continue
        if t.endswith(".png"):
            if t != png_name:
                lines[idx] = png_name
                changed = True
            break
    if changed:
        atlas_path.write_text("\n".join(lines) + ("\n" if text.endswith("\n") else ""), encoding="utf-8")
    return changed


def classify_region(name: str) -> str:
    lower = name.lower()
    if any(k in lower for k in HEAD_KEYS):
        return "head"
    if any(k in lower for k in HAND_KEYS):
        return "hand"
    if any(k in lower for k in LEG_KEYS):
        return "leg"
    if any(k in lower for k in BODY_KEYS):
        return "body"
    return "other"


def merge_bounds(regions: list[AtlasRegion]) -> dict[str, float] | None:
    if not regions:
        return None
    x0 = min(r.x for r in regions)
    y0 = min(r.y for r in regions)
    x1 = max(r.x + r.width for r in regions)
    y1 = max(r.y + r.height for r in regions)
    return {
        "x": float(x0),
        "y": float(y0),
        "width": float(x1 - x0),
        "height": float(y1 - y0),
    }


def normalize_bounds(bounds: dict[str, float], page_w: int, page_h: int) -> dict[str, float]:
    if page_w <= 0 or page_h <= 0:
        return bounds
    return {
        "x": round(bounds["x"] / page_w, 4),
        "y": round(bounds["y"] / page_h, 4),
        "width": round(bounds["width"] / page_w, 4),
        "height": round(bounds["height"] / page_h, 4),
    }


def pick_touch_animation(zone: str, anims: list[str], default_click: str | None) -> str | None:
    lower_map = {a.lower(): a for a in anims}
    if zone == "head":
        for cand in ("touch_special", "touch_head", "touch_face", "touch2"):
            if cand in lower_map:
                return lower_map[cand]
    if zone == "body":
        for cand in ("touch_body", "touch2", "touch"):
            if cand in lower_map:
                return lower_map[cand]
    if zone == "hand":
        for cand in ("touch_hand", "touch"):
            if cand in lower_map:
                return lower_map[cand]
    if default_click and default_click in anims:
        return default_click
    return pick_by_keywords(anims, CLICK_KEYS)


def build_touch_areas(
    regions: list[AtlasRegion],
    page_w: int,
    page_h: int,
    anims: list[str],
    click_animation: str | None,
) -> dict[str, Any]:
    groups: dict[str, list[AtlasRegion]] = {}
    for r in regions:
        zone = classify_region(r.name)
        if zone == "other":
            continue
        groups.setdefault(zone, []).append(r)

    areas: list[dict[str, Any]] = []
    labels = {"head": "头部", "body": "身体", "hand": "手部", "leg": "腿部"}
    for zone, regs in sorted(groups.items()):
        bounds = merge_bounds(regs)
        if not bounds:
            continue
        nb = normalize_bounds(bounds, page_w, page_h)
        if nb["width"] <= 0 or nb["height"] <= 0:
            continue
        areas.append(
            {
                "id": zone,
                "label": labels.get(zone, zone),
                "attachments": sorted({r.name for r in regs})[:12],
                "bounds": nb,
                "click_animation": pick_touch_animation(zone, anims, click_animation),
            }
        )

    if not areas and page_w > 0 and page_h > 0:
        areas.append(
            {
                "id": "full",
                "label": "全身",
                "attachments": [],
                "bounds": {"x": 0.0, "y": 0.0, "width": 1.0, "height": 1.0},
                "click_animation": click_animation or pick_by_keywords(anims, CLICK_KEYS),
            }
        )

    return {
        "version": 1,
        "coordinate_space": "atlas_normalized",
        "default_click_animation": click_animation,
        "areas": areas,
        "logic": {
            "mode": "first_match",
            "description": "按 areas 顺序检测点击落点；未命中时使用 default_click_animation",
            "on_click_busy": "ignore",
        },
    }


def build_animation_meta(anims: list[str], template: dict[str, Any]) -> dict[str, Any]:
    idle = pick_idle(anims, template)
    click = pick_by_keywords(anims, CLICK_KEYS)
    if not click and isinstance(template.get("click_animation"), str):
        pref = template["click_animation"]
        click = next((a for a in anims if a.lower() == pref.lower()), None)
    drag = pick_by_keywords(anims, DRAG_KEYS)
    boot = idle
    return_idle = idle
    random_anims = pick_random(anims, idle, template)
    return {
        "animations": anims,
        "idle_animation": idle,
        "click_animation": click,
        "boot_animation": boot,
        "return_idle_animation": return_idle,
        "drag_animation": drag,
        "random_animations": random_anims,
        "random_min_sec": template.get("random_min_sec", 30),
        "random_max_sec": template.get("random_max_sec", 120),
        "lines": [],
    }


def load_template() -> dict[str, Any]:
    path = action_template_path()
    if path.is_file():
        return json.loads(path.read_text(encoding="utf-8"))
    return {
        "idle_animation": "normal",
        "click_animation": "touch",
        "random_animations": list(RANDOM_TEMPLATE),
        "random_min_sec": 30,
        "random_max_sec": 120,
    }


def build_folder_configs(folder: Path, dry_run: bool = False, force: bool = False) -> dict[str, Any]:
    triple = inspect_spine_folder(folder)
    if not triple:
        raise ValueError(f"not a Spine folder: {folder}")

    skel_path = folder / triple.skel
    atlas_path = folder / triple.atlas
    anims = list_animations(skel_path, atlas_path)
    if not anims:
        raise ValueError(f"no animations detected in {skel_path.name}")

    template = load_template()
    meta = build_animation_meta(anims, template)
    viewer = generate_viewer_ex_config(triple)
    regions, page_w, page_h = parse_atlas_regions(atlas_path)
    touch = build_touch_areas(regions, page_w, page_h, anims, meta.get("click_animation"))

    outputs = {
        "config.json": viewer,
        "animations.meta.json": meta,
        "touch_areas.json": touch,
    }

    written: list[str] = []
    if not dry_run:
        rewrite_atlas_texture_page(atlas_path, triple.png)
        for name, payload in outputs.items():
            dest = folder / name
            if dest.exists() and not force and name == "config.json":
                # keep existing viewer config if triple matches
                try:
                    existing = json.loads(dest.read_text(encoding="utf-8"))
                    if existing.get("type") == 9:
                        written.append(f"{name} (kept)")
                        continue
                except json.JSONDecodeError:
                    pass
            dest.write_text(json.dumps(payload, ensure_ascii=False, indent=2), encoding="utf-8")
            written.append(name)

    return {
        "ok": True,
        "folder": str(folder),
        "slug": folder.name,
        "animations": anims,
        "outputs": list(outputs.keys()),
        "written": written,
        "idle": meta.get("idle_animation"),
        "click": meta.get("click_animation"),
        "touch_areas": len(touch.get("areas") or []),
        "dry_run": dry_run,
    }


def discover_spine_folders(root: Path) -> list[Path]:
    if inspect_spine_folder(root):
        return [root]
    out: list[Path] = []
    if not root.is_dir():
        return out
    for child in sorted(root.iterdir()):
        if child.is_dir() and inspect_spine_folder(child):
            out.append(child)
    return out


def main() -> int:
    parser = argparse.ArgumentParser(description="Build hanpet Spine model JSON configs")
    parser.add_argument("--input", required=True, type=Path)
    parser.add_argument("--dry-run", action="store_true")
    parser.add_argument("--force", action="store_true")
    args = parser.parse_args()

    folders = discover_spine_folders(args.input)
    if not folders:
        print(json.dumps({"ok": False, "error": f"no Spine folders under {args.input}"}, ensure_ascii=False), file=sys.stderr)
        return 1

    results = []
    for folder in folders:
        try:
            results.append(build_folder_configs(folder, dry_run=args.dry_run, force=args.force))
        except Exception as exc:  # noqa: BLE001
            print(json.dumps({"ok": False, "error": str(exc), "folder": str(folder)}, ensure_ascii=False), file=sys.stderr)
            return 1

    print(json.dumps({"ok": True, "count": len(results), "results": results}, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    sys.exit(main())
