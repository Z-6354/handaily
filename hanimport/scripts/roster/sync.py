"""Roster AppData sync / publish / export / verify (sheared from db.py C1)."""

from __future__ import annotations

import argparse
import hashlib
import json
import logging
import os
import re
import shutil
import sqlite3
import sys
import zipfile
from pathlib import Path

from roster.line_skin_match import apply_lines_by_skin, merge_meta_json


def _pull(mod) -> None:
    g = globals()
    for k, v in vars(mod).items():
        # Never copy _pull itself — its globals() is bound to the defining module.
        if k.startswith("__") or k == "_pull":
            continue
        g[k] = v

import roster.ids as _ids
_pull(_ids)

import roster.schema as _schema
_pull(_schema)

import roster.crud as _crud
_pull(_crud)

import roster.merge as _merge
_pull(_merge)

import roster.bind_pipeline as _bind_pipeline
_pull(_bind_pipeline)

import roster.import_wiki as _import_wiki
_pull(_import_wiki)

# --- sync ---

def character_to_manifest(conn: sqlite3.Connection, cid: str) -> dict | None:
    c = conn.execute("SELECT * FROM characters WHERE id=?", (cid,)).fetchone()
    if not c:
        return None
    skins_out = []
    for s in conn.execute(
        "SELECT * FROM skins WHERE character_id=? ORDER BY sort_order, id", (cid,)
    ):
        lines = []
        for ln in conn.execute(
            "SELECT * FROM skin_lines WHERE skin_id=? ORDER BY sort_order, id", (s["id"],)
        ):
            item = {"text": ln["text"]}
            if ln["animation"]:
                item["animation"] = ln["animation"]
            if ln["wiki_key"]:
                item["wiki_key"] = ln["wiki_key"]
            if ln["audio_url"]:
                item["audio_url"] = ln["audio_url"]
            if ln["audio_relpath"]:
                item["audio_relpath"] = ln["audio_relpath"]
            lines.append(item)
        skin_id = skin_manifest_id(cid, s["id"])
        skins_out.append(
            {
                "id": skin_id,
                "name": s["name_zh"],
                "english_name": s["name_en"] or "",
                "model_id": s["pet_model_id"] or "",
                "default": bool(s["is_default"]),
                "skin_index": s["skin_index"],
                "kanmusu_dir": s["kanmusu_dir"] or None,
                "lines": lines,
            }
        )
    preferred = next((s["id"] for s in skins_out if s.get("default")), None)
    if preferred is None and skins_out:
        preferred = skins_out[0]["id"]
    return {
        "id": c["id"],
        "name": c["name_zh"],
        "english_name": c["name_en"] or "",
        "wiki_title": c["wiki_title"] or "",
        "cv": c["cv"] or "",
        "source": c["source"] or "roster",
        "description": c["description"] or "",
        "persona_id": c["persona_id"] or c["id"],
        "faction": c["faction"] or "",
        "ship_type": c["ship_type"] or "",
        "rarity": c["rarity"] or "",
        "skins": skins_out,
        "preferred_skin_id": preferred,
    }

def run_sync_appdata(
    db: Path | None = None,
    data_dir: Path | None = None,
    ids: str = "",
    force_lines: bool = False,
    replace: bool = True,
) -> dict:
    """同步自用库 → AppData characters/manifest.json。

    replace=True（默认）：AppData 角色列表改成「本次同步的自用库角色」
    （不再把旧的八百多个 wiki 角色粘在一起）。
    replace=False：仅 upsert，保留 AppData 里其它角色。
    """
    db = Path(db) if db else default_local_db()
    data_dir = Path(data_dir) if data_dir else appdata_data_dir()
    if not db.is_file():
        return {"ok": False, "error": f"local db missing: {db}"}
    conn = connect(db)
    manifest_path = data_dir / "characters" / "manifest.json"
    manifest_path.parent.mkdir(parents=True, exist_ok=True)
    existing = load_json(manifest_path, {"version": 1, "default_id": "", "characters": []})
    if replace:
        by_id: dict = {}
    else:
        by_id = {c["id"]: c for c in existing.get("characters", []) if isinstance(c, dict)}

    char_ids = [r["id"] for r in conn.execute("SELECT id FROM characters ORDER BY id")]
    if ids:
        want = {x.strip() for x in ids.split(",") if x.strip()}
        char_ids = [i for i in char_ids if i in want]

    synced = []
    for cid in char_ids:
        char = character_to_manifest(conn, cid)
        if not char:
            continue
        prev = None if replace else by_id.get(cid)
        if prev and not force_lines:
            # keep user-edited lines if present
            prev_skins = {s.get("id"): s for s in (prev.get("skins") or []) if isinstance(s, dict)}
            for s in char["skins"]:
                old = prev_skins.get(s["id"])
                if old and old.get("lines") and not force_lines:
                    s["lines"] = old["lines"]
        by_id[cid] = char
        synced.append(cid)

        # copy cubism if present under unpacked
        for s in char["skins"]:
            kd = (s.get("kanmusu_dir") or "").strip()
            if not kd:
                continue
            src = repo_root() / "data" / "model" / "unpacked" / kd
            dst = data_dir / "kanmusu-models" / kd
            if src.is_dir():
                if dst.exists():
                    shutil.rmtree(dst)
                shutil.copytree(src, dst)

    before = len(existing.get("characters") or [])
    existing["characters"] = dedupe_characters_by_name(list(by_id.values()))
    # 全量覆盖时 default 落到自用库第一个；若仍在名单里则保留原 default
    old_default = existing.get("default_id") or ""
    if old_default and any(c.get("id") == old_default for c in existing["characters"]):
        existing["default_id"] = old_default
    elif existing["characters"]:
        existing["default_id"] = existing["characters"][0]["id"]
    else:
        existing["default_id"] = ""
    manifest_path.write_text(
        json.dumps(existing, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )

    # mirror kanmusu manifest for desktop_open
    km_chars = []
    for c in existing["characters"]:
        km_skins = []
        for s in c.get("skins") or []:
            kd = (s.get("kanmusu_dir") or "").strip()
            if not kd:
                continue
            km_skins.append(
                {
                    "id": s["id"],
                    "name": s.get("name") or kd,
                    "model_dir": kd,
                    "lines": [
                        {
                            "text": ln.get("text", ""),
                            **({"animation": ln["animation"]} if ln.get("animation") else {}),
                        }
                        for ln in (s.get("lines") or [])
                        if isinstance(ln, dict) and (ln.get("text") or "").strip()
                    ],
                }
            )
        if km_skins:
            km_chars.append(
                {
                    "id": c["id"],
                    "name": c.get("name") or c["id"],
                    "description": c.get("description") or "",
                    "skins": km_skins,
                }
            )
    km_path = data_dir / "kanmusu" / "manifest.json"
    km_path.parent.mkdir(parents=True, exist_ok=True)
    km_path.write_text(
        json.dumps({"version": 1, "characters": km_chars}, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )
    conn.close()
    return {
        "ok": True,
        "manifest": str(manifest_path),
        "synced": synced,
        "replace": replace,
        "before_count": before,
        "after_count": len(existing["characters"]),
    }

def cmd_sync_appdata(args: argparse.Namespace) -> int:
    result = run_sync_appdata(
        db=Path(args.db) if args.db else None,
        data_dir=Path(args.data_dir) if args.data_dir else None,
        ids=args.ids or "",
        force_lines=bool(args.force_lines),
        replace=not bool(getattr(args, "merge", False)),
    )
    emit(result)
    return 0 if result.get("ok") else 1

def copy_subset_db(src: Path, dst: Path, character_ids: list[str]) -> dict:
    if dst.exists():
        dst.unlink()
    dst.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(src, dst)
    conn = connect(dst)
    apply_schema(conn)
    placeholders = ",".join("?" * len(character_ids)) if character_ids else "''"
    if character_ids:
        conn.execute(f"DELETE FROM characters WHERE id NOT IN ({placeholders})", character_ids)
        # cascades may not fire for skins if we only delete characters with FK — ensure
        conn.execute(
            f"DELETE FROM skins WHERE character_id NOT IN ({placeholders})",
            character_ids,
        )
        conn.execute(
            """
            DELETE FROM skin_lines WHERE skin_id NOT IN (SELECT id FROM skins)
            """
        )
    else:
        conn.execute("DELETE FROM skin_lines")
        conn.execute("DELETE FROM skins")
        conn.execute("DELETE FROM characters")
    conn.execute(
        "INSERT INTO meta(key, value) VALUES(?, ?) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        ("bundled", "1"),
    )
    conn.commit()
    counts = {
        "characters": conn.execute("SELECT count(*) FROM characters").fetchone()[0],
        "skins": conn.execute("SELECT count(*) FROM skins").fetchone()[0],
        "lines": conn.execute("SELECT count(*) FROM skin_lines").fetchone()[0],
    }
    conn.close()
    return counts

def run_publish_bundled(db: Path | None = None, ids: str = "") -> dict:
    db = Path(db) if db else default_local_db()
    if not db.is_file():
        return {"ok": False, "error": f"local db missing: {db} — run import first"}
    allow = load_json(allowlist_path(), {"character_ids": []})
    id_list = list(allow.get("character_ids") or [])
    if ids:
        id_list = [x.strip() for x in ids.split(",") if x.strip()]
    if not id_list:
        return {"ok": False, "error": "empty allowlist — refuse publishing entire local DB"}

    out_db = bundled_roster_dir() / "handaily-roster.sqlite"
    counts = copy_subset_db(db, out_db, id_list)

    # refresh characters/manifest.json for allowlisted ids only (merge keep others if any)
    conn = connect(db)
    bundled_manifest_path = bundled_roster_dir() / "characters" / "manifest.json"
    bundled_manifest = load_json(
        bundled_manifest_path, {"version": 1, "default_id": "cheshire", "characters": []}
    )
    by_id = {
        c["id"]: c for c in bundled_manifest.get("characters", []) if isinstance(c, dict)
    }
    for cid in id_list:
        char = character_to_manifest(conn, cid)
        if char:
            by_id[cid] = char
    # Prefer allowlist order first
    ordered = [by_id[i] for i in id_list if i in by_id]
    rest = [c for i, c in by_id.items() if i not in id_list]
    bundled_manifest["characters"] = ordered + rest
    if ordered and not bundled_manifest.get("default_id"):
        bundled_manifest["default_id"] = ordered[0]["id"]
    # Keep stable default_id if still present
    if bundled_manifest.get("default_id") not in {c["id"] for c in bundled_manifest["characters"]}:
        if ordered:
            bundled_manifest["default_id"] = ordered[0]["id"]
    bundled_manifest_path.parent.mkdir(parents=True, exist_ok=True)
    bundled_manifest_path.write_text(
        json.dumps(bundled_manifest, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )
    conn.close()
    return {
        "ok": True,
        "bundled_db": str(out_db),
        "allowlist": id_list,
        "counts": counts,
        "manifest": str(bundled_manifest_path),
        "note": "models under bundled/roster/pet-models are not copied by this command",
    }

def cmd_publish_bundled(args: argparse.Namespace) -> int:
    result = run_publish_bundled(
        db=Path(args.db) if args.db else None,
        ids=args.ids or "",
    )
    emit(result)
    return 0 if result.get("ok") else 1

def cmd_export_pack(args: argparse.Namespace) -> int:
    """Export a user data pack (subset) — not the full private local DB."""
    db = Path(args.db) if args.db else default_local_db()
    if not db.is_file():
        emit({"ok": False, "error": f"local db missing: {db}"})
        return 1
    ids = [x.strip() for x in (args.ids or "").split(",") if x.strip()]
    if not ids:
        emit({"ok": False, "error": "--ids required (never export entire private DB)"})
        return 1
    out = Path(args.output)
    out.parent.mkdir(parents=True, exist_ok=True)
    tmp = out.with_suffix(".sqlite.tmp")
    counts = copy_subset_db(db, tmp, ids)
    with zipfile.ZipFile(out, "w", compression=zipfile.ZIP_DEFLATED) as zf:
        zf.write(tmp, arcname="handaily-roster.sqlite")
        zf.writestr(
            "README.txt",
            "Handaily roster pack\n"
            f"characters: {', '.join(ids)}\n"
            "Import via future pack import; do not overwrite other users' private data.\n",
        )
    tmp.unlink(missing_ok=True)
    emit({"ok": True, "pack": str(out), "ids": ids, "counts": counts})
    return 0

def cmd_verify(args: argparse.Namespace) -> int:
    db = Path(args.db) if args.db else default_local_db()
    allow = load_json(allowlist_path(), {"character_ids": []})
    allow_ids = set(allow.get("character_ids") or [])
    bundled_db = bundled_roster_dir() / "handaily-roster.sqlite"
    checks = []

    # private db must not live under hanpet/bundled except subset
    private_in_tree = list(repo_root().glob("**/data/roster/handaily-roster.sqlite"))
    checks.append(
        {
            "name": "private_db_path",
            "ok": all("bundled" not in str(p).replace("\\", "/") for p in private_in_tree),
            "paths": [str(p) for p in private_in_tree],
        }
    )

    if db.is_file():
        conn = connect(db)
        n = conn.execute("SELECT count(*) FROM characters").fetchone()[0]
        checks.append({"name": "local_characters", "ok": n > 0, "count": n})
        conn.close()
    else:
        checks.append({"name": "local_db_exists", "ok": False, "path": str(db)})

    if bundled_db.is_file():
        conn = connect(bundled_db)
        rows = [r[0] for r in conn.execute("SELECT id FROM characters").fetchall()]
        extra = [i for i in rows if i not in allow_ids]
        checks.append(
            {
                "name": "bundled_subset_of_allowlist",
                "ok": len(extra) == 0 and len(rows) > 0,
                "bundled_ids": rows,
                "extra": extra,
                "allowlist": sorted(allow_ids),
            }
        )
        conn.close()
    else:
        checks.append({"name": "bundled_db_exists", "ok": False, "path": str(bundled_db)})

    # gitignore presence
    gi = (repo_root() / "data" / ".gitignore").read_text(encoding="utf-8")
    checks.append(
        {
            "name": "gitignore_sqlite",
            "ok": "roster/*.sqlite" in gi or "handaily-roster" in gi,
        }
    )

    ok = all(c.get("ok") for c in checks)
    emit({"ok": ok, "checks": checks})
    return 0 if ok else 2

