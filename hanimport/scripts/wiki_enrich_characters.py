#!/usr/bin/env python3
"""Enrich AppData characters/manifest.json from blhx.sqlite + Cubism unpacked slugs.

- Match slug → Wiki ship (CN name)
- EN from aliases_json / data/wiki/ship-en-names.json
- Skin name from assets 换装 or 皮肤N
- Lines from ships.lines_json copied onto each skin
- kanmusu_dir = slug; pet model_id if pet-models/<slug> exists
"""
from __future__ import annotations

import argparse
import json
import re
import shutil
import sqlite3
import sys
from pathlib import Path

SKIN_SUFFIX = re.compile(r"_(?:\d+|h|g|painting|idol|younv|summer|school|winter|swimsuit|wedding|newyear|cn|jp|en|super)$", re.I)
LATIN_RE = re.compile(r"[A-Za-zÄÖÜäöüßÁÉÍÓÚáéíóúÀÈÌÒÙàèìòùÂÊÎÔÛâêîôûÃÑÕãñõÅåÆæØø]")
CJK_RE = re.compile(r"[\u3040-\u30ff\u3400-\u9fff\uf900-\ufaff]")

# Live2D folder slug → BWIKI display_name (non-standard pinyin)
LIVE2D_ALIASES = {
    "aijiang": "埃吉尔",
    "aijier": "埃吉尔",
    "abeikelongbi": "阿贝克隆比",
    "adaerbote": "阿达尔伯特亲王",
    "aerbien": "阿尔比恩",
    "aersasi": "阿尔萨斯",
    "aidang": "爱宕",
    "aierdeliqi": "埃尔德里奇",
}


def repo_root() -> Path:
    return Path(__file__).resolve().parents[2]


def appdata_data_dir() -> Path:
    import os

    override = os.environ.get("HANDAILY_DATA_DIR", "").strip()
    if override:
        return Path(override)
    appdata = os.environ.get("APPDATA") or ""
    return Path(appdata) / "xiaohan-daily" / "data"


def strip_skin(folder: str) -> tuple[str, str]:
    """Split unpack folder into (character_base, skin_suffix).

    Peels trailing tokens until the base ship id remains:
      qiye_9 → (qiye, 9)
      abeikelongbi_3_1 → (abeikelongbi, 3_1)
      abeikelongbi_3_hx → (abeikelongbi, 3_hx)
      z23_hx → (z23, hx)
    """
    name = folder
    parts: list[str] = []
    while True:
        m = SKIN_SUFFIX.search(name)
        if not m or m.start() == 0:
            break
        # Only peel from the end of the string
        if m.end() != len(name):
            break
        parts.append(m.group(0)[1:].lower())
        name = name[: m.start()]
    parts.reverse()
    return name, "_".join(parts)


def skin_label(suffix: str) -> str:
    if not suffix:
        return "默认"
    if suffix.isdigit():
        return f"皮肤{suffix}"
    return f"变体_{suffix}"


def pick_english(aliases: list, fallback: str) -> str:
    best = ""
    for a in aliases or []:
        if not isinstance(a, str):
            continue
        s = a.strip()
        if not s or not LATIN_RE.search(s):
            continue
        # Prefer mostly-latin (skip strings that are mostly CJK)
        cjk = len(CJK_RE.findall(s))
        if cjk > len(s) / 2:
            continue
        if len(s) > len(best):
            best = s
    return best or fallback


def pick_skin_title(assets: list, skin_index: int | None, fallback: str) -> str:
    skins = [
        a
        for a in (assets or [])
        if isinstance(a, dict) and a.get("kind") == "skin" and isinstance(a.get("name"), str)
    ]
    # Prefer names containing 换装
    labeled = [s for s in skins if "换装" in str(s.get("name"))]
    pool = labeled or skins
    if not pool:
        return fallback
    if skin_index is None or skin_index <= 0:
        return Path(str(pool[0]["name"])).stem.replace(".jpg", "")
    # Map skin_index N → 换装 / 换装N-1 style:
    # index 2 → often 换装 / 换装.jpg; index 3 → 换装2, etc. Heuristic.
    for s in pool:
        name = str(s["name"])
        stem = Path(name).stem
        # 爱宕换装2 → index 3? We use suffix match 换装{N-1} or 换装N
        m = re.search(r"换装\s*(\d+)", stem)
        if m and int(m.group(1)) + 1 == skin_index:
            return stem
        if m and int(m.group(1)) == skin_index:
            return stem
    if skin_index == 2 and pool:
        # first 换装 without number
        for s in pool:
            stem = Path(str(s["name"])).stem
            if re.search(r"换装\s*$", stem) or stem.endswith("换装"):
                return stem
    return fallback


def lines_from_wiki(raw_lines: list) -> list[dict]:
    out = []
    for item in raw_lines or []:
        if not isinstance(item, dict):
            continue
        text = (item.get("text") or "").strip()
        if not text:
            continue
        key = item.get("key")
        label = item.get("label")
        anim = None
        if isinstance(key, str) and key:
            # map common keys to touch_* / idle
            k = key.lower()
            if "touch2" in k or "touch_special" in k:
                anim = "touch_special"
            elif "head" in k:
                anim = "touch_head"
            elif "touch" in k:
                anim = "touch_body"
            elif "idle" in k or "main" in k:
                anim = "idle"
        out.append(
            {
                "text": text,
                **({"animation": anim} if anim else {}),
                **({"wiki_key": key} if isinstance(key, str) and key else {}),
            }
        )
    return out


def load_json(path: Path, default):
    if not path.is_file():
        return default
    return json.loads(path.read_text(encoding="utf-8"))


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument(
        "--wiki-db",
        type=Path,
        default=repo_root() / "mcp/blhx-wiki/data/blhx.sqlite",
    )
    ap.add_argument(
        "--unpacked",
        type=Path,
        default=repo_root() / "data/model/unpacked",
    )
    ap.add_argument(
        "--en-map",
        type=Path,
        default=repo_root() / "data/wiki/ship-en-names.json",
    )
    ap.add_argument(
        "--data-dir",
        type=Path,
        default=None,
        help="AppData xiaohan-daily/data (default: %%APPDATA%%/...)",
    )
    ap.add_argument("--force-lines", action="store_true")
    args = ap.parse_args()

    data_dir = args.data_dir or appdata_data_dir()
    chars_dir = data_dir / "characters"
    manifest_path = chars_dir / "manifest.json"
    pet_models = data_dir / "pet-models"
    kanmusu_models = data_dir / "kanmusu-models"

    if not args.wiki_db.is_file():
        print(json.dumps({"ok": False, "error": f"wiki db missing: {args.wiki_db}"}))
        return 1
    if not args.unpacked.is_dir():
        print(json.dumps({"ok": False, "error": f"unpacked missing: {args.unpacked}"}))
        return 1

    en_map = load_json(args.en_map, {})
    aliases_path = repo_root() / "mcp/blhx-wiki/data/live2d-aliases.json"
    alias_map = {**LIVE2D_ALIASES, **load_json(aliases_path, {})}

    conn = sqlite3.connect(str(args.wiki_db))
    conn.row_factory = sqlite3.Row

    # Ensure live2d_mappings rows for folders (for future MCP tools)
    folders = sorted(
        p.name for p in args.unpacked.iterdir() if p.is_dir() and not p.name.startswith(".")
    )

    chars_dir.mkdir(parents=True, exist_ok=True)
    kanmusu_models.mkdir(parents=True, exist_ok=True)

    if manifest_path.is_file():
        manifest = load_json(manifest_path, {"version": 1, "default_id": "", "characters": []})
    else:
        manifest = {"version": 1, "default_id": "", "characters": []}

    by_id = {c["id"]: c for c in manifest.get("characters", []) if isinstance(c, dict)}
    upserted = []

    for folder in folders:
        base, suffix = strip_skin(folder)
        skin_index = int(suffix) if suffix.isdigit() else (0 if not suffix else None)
        cn = alias_map.get(base)
        row = None
        if cn:
            row = conn.execute(
                "SELECT wiki_title, display_name, aliases_json, lines_json, assets_json FROM ships WHERE display_name=? OR wiki_title=?",
                (cn, cn),
            ).fetchone()
        if row is None:
            # try catalog fuzzy via display containing - last resort: skip EN and leave name=base
            print(json.dumps({"folder": folder, "warn": "no wiki ship", "base": base}, ensure_ascii=False))
            display = cn or base
            wiki_title = display
            aliases = []
            lines_raw = []
            assets = []
        else:
            display = row["display_name"] or row["wiki_title"]
            wiki_title = row["wiki_title"]
            aliases = json.loads(row["aliases_json"] or "[]")
            lines_raw = json.loads(row["lines_json"] or "[]")
            assets = json.loads(row["assets_json"] or "[]")
            try:
                conn.execute(
                    """
                    INSERT INTO live2d_mappings(folder, wiki_title, display_name, skin_label, score, updated_at)
                    VALUES (?,?,?,?,?,datetime('now'))
                    ON CONFLICT(folder) DO UPDATE SET
                      wiki_title=excluded.wiki_title,
                      display_name=excluded.display_name,
                      skin_label=excluded.skin_label,
                      score=excluded.score,
                      updated_at=excluded.updated_at
                    """,
                    (folder, wiki_title, display, skin_label(suffix), 99),
                )
            except sqlite3.Error:
                pass

        english = pick_english(aliases, en_map.get(base, ""))
        fallback_skin = skin_label(suffix)
        skin_name = pick_skin_title(assets, skin_index, fallback_skin)
        lines = lines_from_wiki(lines_raw)

        # Copy Cubism into AppData kanmusu-models/<folder>
        src = args.unpacked / folder
        dst = kanmusu_models / folder
        if src.is_dir():
            if dst.exists():
                shutil.rmtree(dst)
            shutil.copytree(src, dst)

        pet_model_id = ""
        if (pet_models / folder).is_dir():
            pet_model_id = folder
        elif (pet_models / f"skin-{folder}").is_dir():
            pet_model_id = f"skin-{folder}"

        char = by_id.get(base)
        if char is None:
            char = {
                "id": base,
                "name": display,
                "source": "wiki",
                "description": "",
                "persona_id": base,
                "skins": [],
                "preferred_skin_id": None,
                "faction": "",
                "ship_type": "",
                "rarity": "",
                "english_name": english,
                "wiki_title": wiki_title,
            }
            by_id[base] = char
            manifest.setdefault("characters", []).append(char)
        else:
            char["name"] = display
            char["english_name"] = english or char.get("english_name") or ""
            char["wiki_title"] = wiki_title
            if not char.get("source"):
                char["source"] = "wiki"
            if not char.get("persona_id"):
                char["persona_id"] = base

        skins = char.setdefault("skins", [])
        existing = next((s for s in skins if s.get("id") == folder), None)
        skin_obj = {
            "id": folder,
            "name": skin_name,
            "model_id": pet_model_id,
            "default": False,
            "skin_index": skin_index,
            "kanmusu_dir": folder,
            "lines": lines if (args.force_lines or not (existing or {}).get("lines")) else existing.get("lines", lines),
        }
        if existing:
            # merge keep user lines unless force
            if not args.force_lines and existing.get("lines"):
                skin_obj["lines"] = existing["lines"]
            if existing.get("model_id") and not pet_model_id:
                skin_obj["model_id"] = existing["model_id"]
            idx = skins.index(existing)
            skins[idx] = skin_obj
        else:
            skins.append(skin_obj)

        if not char.get("preferred_skin_id"):
            char["preferred_skin_id"] = folder

        upserted.append(
            {
                "folder": folder,
                "character_id": base,
                "name": display,
                "english_name": english,
                "skin_name": skin_name,
                "lines": len(skin_obj["lines"]),
                "pet_model_id": pet_model_id,
                "kanmusu_dir": folder,
            }
        )

    conn.commit()
    conn.close()

    if not manifest.get("default_id") and manifest.get("characters"):
        manifest["default_id"] = manifest["characters"][0]["id"]

    manifest_path.write_text(
        json.dumps(manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )
    # Also mirror a lightweight kanmusu/manifest for desktop_open compatibility
    kanmusu_chars: dict[str, dict] = {}
    for c in manifest.get("characters", []):
        if not isinstance(c, dict):
            continue
        km_skins = []
        for s in c.get("skins") or []:
            kd = (s.get("kanmusu_dir") or "").strip()
            if not kd:
                continue
            km_skins.append(
                {
                    "id": s.get("id") or kd,
                    "name": s.get("name") or kd,
                    "model_dir": kd,
                    "lines": [
                        {"text": ln.get("text", ""), **({"animation": ln["animation"]} if ln.get("animation") else {})}
                        for ln in (s.get("lines") or [])
                        if isinstance(ln, dict) and (ln.get("text") or "").strip()
                    ],
                }
            )
        if km_skins:
            kanmusu_chars[c["id"]] = {
                "id": c["id"],
                "name": c.get("name") or c["id"],
                "description": c.get("description") or "",
                "skins": km_skins,
            }
    kanmusu_manifest = {
        "version": 1,
        "characters": list(kanmusu_chars.values()),
    }
    km_path = data_dir / "kanmusu" / "manifest.json"
    km_path.parent.mkdir(parents=True, exist_ok=True)
    km_path.write_text(json.dumps(kanmusu_manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")

    out = json.dumps(
        {
            "ok": True,
            "manifest": str(manifest_path),
            "kanmusu_manifest": str(km_path),
            "upserted": upserted,
        },
        ensure_ascii=False,
        indent=2,
    )
    sys.stdout.buffer.write((out + "\n").encode("utf-8"))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
