#!/usr/bin/env python3
"""Merge BWIKI-hash duplicate characters into pinyin/kanmusu canonical ids (AppData).

Usage:
  python hanimport/scripts/merge_duplicate_characters.py
  python hanimport/scripts/merge_duplicate_characters.py --dry-run
"""

from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import sqlite3
from collections import defaultdict
from pathlib import Path
from typing import Any

HASH_PERSONA = re.compile(r"^p[0-9a-f]{8}$", re.I)
HASH_MODEL = re.compile(r"^m[0-9a-f]{8}(?:-(\d+))?$", re.I)
# Wiki 导入的数字皮肤包目录（如 2-14、4-2、5）不是角色皮肤序号
NUM_PACK = re.compile(r"^\d+(?:-\d+)?$")
SUFFIX_NUM = re.compile(r"(?:_|-)(\d+)$")
SLUG_WITH_INDEX = re.compile(r"^[a-z][a-z0-9]*_(\d+)$", re.I)


def appdata_root() -> Path:
    override = os.environ.get("HANDAILY_DATA_DIR")
    if override:
        return Path(override)
    return Path(os.environ["APPDATA"]) / "xiaohan-daily" / "data"


def load_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def save_json(path: Path, data: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(data, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def is_hash_id(cid: str) -> bool:
    return bool(HASH_PERSONA.match(cid))


def pick_canonical(chars: list[dict[str, Any]]) -> dict[str, Any]:
    """Prefer non-hash (pinyin) id; else one with kanmusu_dir skins."""
    non_hash = [c for c in chars if not is_hash_id(c["id"])]
    if non_hash:
        # prefer the one that already has kanmusu skins
        with_km = [
            c
            for c in non_hash
            if any((s.get("kanmusu_dir") or "").strip() for s in c.get("skins") or [])
        ]
        return with_km[0] if with_km else non_hash[0]
    with_km = [
        c
        for c in chars
        if any((s.get("kanmusu_dir") or "").strip() for s in c.get("skins") or [])
    ]
    return with_km[0] if with_km else chars[0]


NAME_INDEX = re.compile(r"(?:换装|皮肤)\s*(\d+)")


def infer_skin_index(skin: dict[str, Any]) -> int | None:
    raw = skin.get("skin_index")
    if raw is not None and str(raw).strip() != "":
        try:
            return int(raw)
        except (TypeError, ValueError):
            pass

    # 显示名「皮肤2」「换装2」优先（数字包 model_id 如 2-9 本身无序号语义）
    name = (skin.get("name") or "").strip()
    if name:
        m = NAME_INDEX.search(name)
        if m:
            return int(m.group(1))
        if re.search(r"换装\s*$", name) or name.endswith("换装"):
            return 2

    kd = (skin.get("kanmusu_dir") or "").strip()
    if kd:
        m = SLUG_WITH_INDEX.match(kd) or SUFFIX_NUM.search(kd)
        if m:
            return int(m.group(1))
        # 无序号后缀的基础目录视为默认皮（与 Rust infer 一致）
        if not NUM_PACK.match(kd) and not HASH_PERSONA.match(kd):
            return 0

    sid = (skin.get("id") or "").strip()
    if sid in ("default",):
        return 0
    if sid.startswith("skin-"):
        sid_body = sid[5:]
        if NUM_PACK.match(sid_body):
            return None
        hm = HASH_MODEL.match(sid_body)
        if hm:
            return int(hm.group(1)) if hm.group(1) else 0
        m = SLUG_WITH_INDEX.match(sid_body)
        if m:
            return int(m.group(1))
    else:
        m = SLUG_WITH_INDEX.match(sid)
        if m:
            return int(m.group(1))

    mid = (skin.get("model_id") or "").strip()
    if mid:
        if NUM_PACK.match(mid):
            return None
        hm = HASH_MODEL.match(mid)
        if hm:
            return int(hm.group(1)) if hm.group(1) else 0
        if HASH_PERSONA.match(mid):
            return 0
        m = SLUG_WITH_INDEX.match(mid)
        if m:
            return int(m.group(1))
    return None


def skin_sort_key(skin: dict[str, Any]) -> tuple[int, str]:
    idx = infer_skin_index(skin)
    return (idx if idx is not None else 10_000, skin.get("id") or "")


def merge_skin(dst: dict[str, Any], src: dict[str, Any]) -> dict[str, Any]:
    out = dict(dst)
    # Spine 小人优先填空
    if not (out.get("model_id") or "").strip() and (src.get("model_id") or "").strip():
        out["model_id"] = src["model_id"]
    if not (out.get("kanmusu_dir") or "").strip() and (src.get("kanmusu_dir") or "").strip():
        out["kanmusu_dir"] = src["kanmusu_dir"]
    if out.get("skin_index") is None and src.get("skin_index") is not None:
        out["skin_index"] = src["skin_index"]
    # keep richer lines
    dst_lines = out.get("lines") or []
    src_lines = src.get("lines") or []
    if len(src_lines) > len(dst_lines):
        out["lines"] = src_lines
    if not (out.get("english_name") or "").strip() and (src.get("english_name") or "").strip():
        out["english_name"] = src["english_name"]
    if not (out.get("name") or "").strip() and (src.get("name") or "").strip():
        out["name"] = src["name"]
    return out


def coalesce_skins(skins: list[dict[str, Any]], canon_id: str) -> list[dict[str, Any]]:
    by_idx: dict[int, dict[str, Any]] = {}
    leftovers: list[dict[str, Any]] = []
    for s in skins:
        idx = infer_skin_index(s)
        if idx is None:
            leftovers.append(dict(s))
            continue
        if idx in by_idx:
            by_idx[idx] = merge_skin(by_idx[idx], s)
        else:
            row = dict(s)
            row["skin_index"] = idx
            # normalize id for indexed skins under canonical character
            if idx == 0:
                row["id"] = "default"
            else:
                kd = (row.get("kanmusu_dir") or "").strip()
                if kd:
                    row["id"] = kd
                elif SLUG_WITH_INDEX.match((row.get("id") or "").strip()):
                    pass
                else:
                    row["id"] = f"{canon_id}_{idx}"
            by_idx[idx] = row
    merged = sorted(by_idx.values(), key=skin_sort_key)
    # leftover spine skins (numeric folder names etc.) appended, de-dupe by model_id
    seen_models = {
        (s.get("model_id") or "").strip() for s in merged if (s.get("model_id") or "").strip()
    }
    for s in leftovers:
        mid = (s.get("model_id") or "").strip()
        if mid and mid in seen_models:
            continue
        if mid:
            seen_models.add(mid)
        leftovers_row = dict(s)
        if mid and not (leftovers_row.get("id") or "").startswith("skin-"):
            leftovers_row["id"] = f"skin-{mid}"
        elif not leftovers_row.get("id"):
            leftovers_row["id"] = f"skin-{mid or 'extra'}"
        merged.append(leftovers_row)
    return merged


def relocate_persona_files(data_dir: Path, old_id: str, new_id: str, dry: bool) -> None:
    personas = data_dir / "personas"
    for ext in (".md", ".json"):
        src = personas / f"{old_id}{ext}"
        dst = personas / f"{new_id}{ext}"
        if not src.is_file():
            continue
        if dry:
            print(f"  would move persona file {src.name} -> {dst.name}")
            continue
        if dst.is_file():
            # keep larger / richer file
            if src.stat().st_size > dst.stat().st_size:
                shutil.copy2(src, dst)
            src.unlink(missing_ok=True)
        else:
            src.rename(dst)


def rewrite_settings_ids(data_dir: Path, id_map: dict[str, str], dry: bool) -> None:
    candidates = [
        data_dir / "xiaohan.sqlite",
        data_dir / "xiaohan.db",
        *sorted(data_dir.glob("*.sqlite")),
        *sorted(data_dir.glob("*.db")),
    ]
    db_path = next((p for p in candidates if p.is_file()), None)
    if db_path is None:
        print("  settings db not found; skip favorites rewrite")
        return
    if dry:
        print(f"  would rewrite settings in {db_path.name} for {len(id_map)} ids")
        return
    con = sqlite3.connect(str(db_path))
    try:
        tables = {
            r[0]
            for r in con.execute(
                "SELECT name FROM sqlite_master WHERE type='table'"
            ).fetchall()
        }
        table = "app_settings" if "app_settings" in tables else (
            "settings" if "settings" in tables else None
        )
        if table is None:
            print("  no app_settings/settings table; skip")
            return
        rows = list(con.execute(f"SELECT key, value FROM {table}"))
        for key, value in rows:
            if value is None:
                continue
            new_val = value
            changed = False
            if key == "character_favorites":
                try:
                    arr = json.loads(value)
                    if isinstance(arr, list):
                        mapped = [id_map.get(str(x), str(x)) for x in arr]
                        seen: set[str] = set()
                        uniq = []
                        for x in mapped:
                            if x in seen:
                                continue
                            seen.add(x)
                            uniq.append(x)
                        new_val = json.dumps(uniq, ensure_ascii=False)
                        changed = new_val != value
                except json.JSONDecodeError:
                    pass
            elif key in (
                "active_character_id",
                "active_persona_id",
                "kanmusu_active_character_id",
            ):
                mapped = id_map.get(str(value).strip())
                if mapped and mapped != value:
                    new_val = mapped
                    changed = True
            if changed:
                con.execute(
                    f"UPDATE {table} SET value=? WHERE key=?",
                    (new_val, key),
                )
                print(f"  {table}[{key}] remapped")
        con.commit()
    finally:
        con.close()


def merge_group(
    chars: list[dict[str, Any]],
    personas_by_id: dict[str, dict[str, Any]],
    data_dir: Path,
    dry: bool,
) -> tuple[dict[str, Any], list[str], dict[str, str]]:
    canon = pick_canonical(chars)
    donors = [c for c in chars if c["id"] != canon["id"]]
    id_map: dict[str, str] = {d["id"]: canon["id"] for d in donors}
    print(f"* {canon.get('name')} → keep {canon['id']}, merge { [d['id'] for d in donors] }")

    all_skins: list[dict[str, Any]] = list(canon.get("skins") or [])
    for d in donors:
        all_skins.extend(d.get("skins") or [])
        # enrich character meta
        if not (canon.get("english_name") or "").strip() and (d.get("english_name") or "").strip():
            canon["english_name"] = d["english_name"]
        if not (canon.get("wiki_title") or "").strip() and (d.get("wiki_title") or "").strip():
            canon["wiki_title"] = d["wiki_title"]
        if not (canon.get("description") or "").strip() and (d.get("description") or "").strip():
            canon["description"] = d["description"]
        if (d.get("source") or "") and not (canon.get("source") or "").strip():
            canon["source"] = d["source"]

    canon["skins"] = coalesce_skins(all_skins, canon["id"])
    canon["persona_id"] = canon["id"]

    # persona manifest: prefer non-hash / richer
    for d in donors:
        relocate_persona_files(data_dir, d["id"], canon["id"], dry)
        if d["id"] in personas_by_id and canon["id"] not in personas_by_id:
            personas_by_id[canon["id"]] = {
                **personas_by_id[d["id"]],
                "id": canon["id"],
                "name": canon.get("name") or personas_by_id[d["id"]].get("name", ""),
            }
        elif d["id"] in personas_by_id and canon["id"] in personas_by_id:
            # keep canon entry name
            personas_by_id[canon["id"]]["name"] = (
                canon.get("name") or personas_by_id[canon["id"]].get("name") or ""
            )

    remove_ids = [d["id"] for d in donors]
    return canon, remove_ids, id_map


def run(dry: bool) -> int:
    data_dir = appdata_root()
    char_path = data_dir / "characters" / "manifest.json"
    persona_path = data_dir / "personas" / "manifest.json"
    if not char_path.is_file():
        print(f"missing {char_path}")
        return 1

    char_manifest = load_json(char_path)
    persona_manifest = load_json(persona_path) if persona_path.is_file() else {"personas": []}
    characters: list[dict[str, Any]] = list(char_manifest.get("characters") or [])
    personas: list[dict[str, Any]] = list(persona_manifest.get("personas") or [])
    personas_by_id = {p["id"]: p for p in personas}

    by_name: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for c in characters:
        name = (c.get("name") or "").strip()
        if name:
            by_name[name].append(c)

    groups = {k: v for k, v in by_name.items() if len(v) > 1}
    if not groups:
        print("no duplicate name groups")
        return 0

    print(f"found {len(groups)} duplicate name group(s)")
    keep_ids: set[str] = set()
    remove_ids: set[str] = set()
    id_map: dict[str, str] = {}
    merged_chars: dict[str, dict[str, Any]] = {}

    for _name, group in sorted(groups.items()):
        canon, rem, mapping = merge_group(group, personas_by_id, data_dir, dry)
        merged_chars[canon["id"]] = canon
        keep_ids.add(canon["id"])
        remove_ids.update(rem)
        id_map.update(mapping)

    # rebuild characters list
    out_chars: list[dict[str, Any]] = []
    seen: set[str] = set()
    for c in characters:
        cid = c["id"]
        if cid in remove_ids:
            continue
        if cid in merged_chars:
            if cid in seen:
                continue
            out_chars.append(merged_chars[cid])
            seen.add(cid)
            continue
        out_chars.append(c)
        seen.add(cid)

    out_personas = []
    seen_p: set[str] = set()
    for pid, p in personas_by_id.items():
        if pid in remove_ids:
            continue
        if pid in seen_p:
            continue
        # ensure merged canon persona exists
        out_personas.append(p)
        seen_p.add(pid)
    for cid in keep_ids:
        if cid not in seen_p:
            c = merged_chars[cid]
            out_personas.append(
                {
                    "id": cid,
                    "name": c.get("name") or cid,
                    "source": c.get("source") or "",
                    "description": c.get("description") or "",
                }
            )
            seen_p.add(cid)

    print(f"characters {len(characters)} → {len(out_chars)}; remove {sorted(remove_ids)}")
    if dry:
        for cid, c in merged_chars.items():
            skins = [
                {
                    "id": s.get("id"),
                    "idx": s.get("skin_index"),
                    "model_id": s.get("model_id"),
                    "kanmusu_dir": s.get("kanmusu_dir"),
                }
                for s in c.get("skins") or []
            ]
            print(f"  {cid} skins => {skins}")
        return 0

    char_manifest["characters"] = out_chars
    persona_manifest["personas"] = out_personas
    save_json(char_path, char_manifest)
    save_json(persona_path, persona_manifest)
    rewrite_settings_ids(data_dir, id_map, dry=False)
    print("done")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--dry-run", action="store_true")
    args = ap.parse_args()
    return run(dry=args.dry_run)


if __name__ == "__main__":
    raise SystemExit(main())
