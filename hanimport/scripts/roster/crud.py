"""Roster character/skin CRUD helpers (sheared from db.py C1)."""

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

# --- crud ---

def _skin_row_dicts(conn: sqlite3.Connection, cid: str) -> list[dict]:
    rows = conn.execute(
        "SELECT * FROM skins WHERE character_id=? ORDER BY sort_order, id", (cid,)
    ).fetchall()
    return [{k: r[k] for k in r.keys()} for r in rows]

def _skin_is_manual_or_bound(conn: sqlite3.Connection, sid: str) -> bool:
    """True for L2D / bound / unpacked skins that Wiki sync must not purge."""
    row = conn.execute(
        "SELECT id, pet_model_id, kanmusu_dir, meta_json FROM skins WHERE id=?",
        (sid,),
    ).fetchone()
    if not row:
        return False
    sid_s = str(row[0])
    if "-l2d-" in sid_s.lower():
        return True
    if str(row[1] or "").strip() or str(row[2] or "").strip():
        return True
    try:
        meta = json.loads(row[3] or "{}")
    except json.JSONDecodeError:
        return False
    if not isinstance(meta, dict):
        return False
    if str(meta.get("source") or "").lower() == "unpacked":
        return True
    if meta.get("binds") or meta.get("bind"):
        return True
    return False

def _delete_skins_not_in(
    conn: sqlite3.Connection,
    cid: str,
    keep_ids: set[str],
    *,
    purge_orphans: bool = False,
) -> int:
    """删除该角色不在 keep 集合内的皮肤行（台词经 FK CASCADE）。

    默认保留 L2D / 已绑定 / meta.source=unpacked 的皮；仅 purge_orphans=True 时全删。
    """
    rows = conn.execute(
        "SELECT id FROM skins WHERE character_id = ?", (cid,)
    ).fetchall()
    deleted = 0
    for (sid,) in rows:
        if sid in keep_ids:
            continue
        if not purge_orphans and _skin_is_manual_or_bound(conn, sid):
            continue
        conn.execute("DELETE FROM skins WHERE id = ?", (sid,))
        deleted += 1
    return deleted

def expected_skin_ids_from_slots(cid: str, slots: list[dict]) -> set[str]:
    """Wiki skins_json → 权威 skin id 集合（不写库）。"""
    keep: set[str] = set()
    for slot in slots:
        if not isinstance(slot, dict):
            continue
        key = str(slot.get("key") or "").strip()
        label = str(slot.get("label") or key).strip()
        kind = str(slot.get("kind") or "skin")
        if not key or is_hidden_wiki_skin(kind=kind, label=label):
            continue
        if key == "default" or kind == "default":
            keep.add(skin_db_id(cid, "default"))
        elif key == "oath" or kind == "oath":
            keep.add(f"{cid}-oath")
        else:
            keep.add(f"{cid}-{key}")
    return keep

def local_skin_ids(conn: sqlite3.Connection, cid: str) -> set[str]:
    return {
        str(r[0])
        for r in conn.execute(
            "SELECT id FROM skins WHERE character_id=?", (cid,)
        )
    }

def character_skins_in_sync(
    conn: sqlite3.Connection, cid: str, slots: list[dict]
) -> bool:
    """Wiki 权威皮均已存在即可（本地可额外保留 L2D/手工皮）。"""
    keep = expected_skin_ids_from_slots(cid, slots)
    if not keep:
        return False
    return keep.issubset(local_skin_ids(conn, cid))

def purge_folder_like_skins(conn: sqlite3.Connection) -> int:
    """删除 id 不以 {character_id}- 开头的解包脏皮（如 aijier_3）。"""
    rows = conn.execute("SELECT character_id, id FROM skins").fetchall()
    deleted = 0
    for cid, sid in rows:
        cid_s, sid_s = str(cid), str(sid)
        if sid_s == cid_s or sid_s.startswith(cid_s + "-"):
            continue
        conn.execute("DELETE FROM skins WHERE id=?", (sid_s,))
        deleted += 1
    return deleted

def purge_hx_skins(conn: sqlite3.Connection) -> int:
    """Delete harmonized (*_hx) skins and their lines. Returns deleted skin count."""
    rows = conn.execute(
        "SELECT id, kanmusu_dir, name_zh FROM skins"
    ).fetchall()
    deleted = 0
    for row in rows:
        sid = str(row[0] if not isinstance(row, sqlite3.Row) else row["id"])
        kdir = str(
            (row[1] if not isinstance(row, sqlite3.Row) else row["kanmusu_dir"]) or ""
        )
        name = str(
            (row[2] if not isinstance(row, sqlite3.Row) else row["name_zh"]) or ""
        )
        if not is_hx_skin(skin_id=sid, kanmusu_dir=kdir, name_zh=name):
            continue
        conn.execute("DELETE FROM skin_lines WHERE skin_id=?", (sid,))
        conn.execute("DELETE FROM skins WHERE id=?", (sid,))
        deleted += 1
    return deleted

def list_character_ids_needing_lines(conn: sqlite3.Connection) -> list[str]:
    """本地仍有空/未匹配台词的角色（用于增量写入）。"""
    need: list[str] = []
    for (cid,) in conn.execute("SELECT id FROM characters ORDER BY id"):
        skins = conn.execute(
            "SELECT id, meta_json FROM skins WHERE character_id=?",
            (cid,),
        ).fetchall()
        if not skins:
            continue
        for sid, meta_raw in skins:
            n = conn.execute(
                "SELECT count(*) FROM skin_lines WHERE skin_id=?", (sid,)
            ).fetchone()[0]
            status = ""
            try:
                meta = json.loads(meta_raw or "{}")
                status = str((meta.get("lines_import") or {}).get("status") or "")
            except json.JSONDecodeError:
                status = ""
            if n == 0 or status in ("empty", "unmatched", "stale_flat"):
                need.append(str(cid))
                break
    return need

def lines_rows_from_wiki(raw_lines: list) -> list[dict]:
    out = []
    for i, item in enumerate(raw_lines or []):
        if not isinstance(item, dict):
            continue
        text = (item.get("text") or "").strip()
        if not text:
            continue
        key = item.get("key") if isinstance(item.get("key"), str) else ""
        label = item.get("label") if isinstance(item.get("label"), str) else ""
        lang = item.get("lang") if isinstance(item.get("lang"), str) else ""
        audio = item.get("audioUrl") if isinstance(item.get("audioUrl"), str) else ""
        anim = ""
        if key:
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
                "wiki_key": key or "",
                "label": label or "",
                "lang": lang or "",
                "text": text,
                "animation": anim,
                "audio_url": audio or "",
                "audio_relpath": "",
                "sort_order": i,
            }
        )
    return out

def fill_english_names(conn: sqlite3.Connection) -> dict:
    cur = conn.execute("SELECT id, name_en FROM characters")
    c_n = 0
    for cid, en in cur.fetchall():
        if not (en or "").strip():
            conn.execute("UPDATE characters SET name_en=? WHERE id=?", (cid, cid))
            c_n += 1
    cur = conn.execute("SELECT id, name_en FROM skins")
    s_n = 0
    for sid, en in cur.fetchall():
        if not (en or "").strip():
            conn.execute("UPDATE skins SET name_en=? WHERE id=?", (sid, sid))
            s_n += 1
    conn.commit()
    return {"characters": c_n, "skins": s_n}

def upsert_character(conn: sqlite3.Connection, row: dict) -> None:
    cid = (row.get("id") or "").strip()
    if is_folder_like_character_id(cid):
        raise ValueError(
            f"folder-like character id rejected (skin suffix): {cid!r}; "
            "use base id (e.g. abeikelongbi) and attach skins separately"
        )
    name_en = normalize_name_en(row.get("name_en") or "", cid)
    conn.execute(
        """
        INSERT INTO characters(
          id, name_zh, name_en, wiki_title, cv, faction, ship_type, rarity,
          persona_id, source, description, meta_json, updated_at
        ) VALUES (?,?,?,?,?,?,?,?,?,?,?,?,datetime('now'))
        ON CONFLICT(id) DO UPDATE SET
          name_zh=excluded.name_zh,
          name_en=CASE WHEN excluded.name_en!='' THEN excluded.name_en ELSE characters.name_en END,
          wiki_title=excluded.wiki_title,
          cv=excluded.cv,
          faction=excluded.faction,
          ship_type=excluded.ship_type,
          rarity=excluded.rarity,
          persona_id=CASE WHEN excluded.persona_id!='' THEN excluded.persona_id ELSE characters.persona_id END,
          source=excluded.source,
          description=CASE WHEN excluded.description!='' THEN excluded.description ELSE characters.description END,
          updated_at=datetime('now')
        """,
        (
            row["id"],
            row["name_zh"],
            name_en,
            row.get("wiki_title") or "",
            row.get("cv") or "",
            row.get("faction") or "",
            row.get("ship_type") or "",
            row.get("rarity") or "",
            row.get("persona_id") or row["id"],
            row.get("source") or "",
            row.get("description") or "",
            row.get("meta_json") or "{}",
        ),
    )

def upsert_skin(conn: sqlite3.Connection, row: dict, replace_lines: bool) -> None:
    name_en = normalize_name_en(row.get("name_en") or "", row["id"])
    conn.execute(
        """
        INSERT INTO skins(
          id, character_id, name_zh, name_en, skin_index, pet_model_id, kanmusu_dir,
          sort_order, is_default, meta_json, updated_at
        ) VALUES (?,?,?,?,?,?,?,?,?,?,datetime('now'))
        ON CONFLICT(id) DO UPDATE SET
          character_id=excluded.character_id,
          name_zh=excluded.name_zh,
          name_en=CASE WHEN excluded.name_en!='' THEN excluded.name_en ELSE skins.name_en END,
          skin_index=excluded.skin_index,
          pet_model_id=CASE WHEN excluded.pet_model_id!='' THEN excluded.pet_model_id ELSE skins.pet_model_id END,
          kanmusu_dir=CASE WHEN excluded.kanmusu_dir!='' THEN excluded.kanmusu_dir ELSE skins.kanmusu_dir END,
          sort_order=excluded.sort_order,
          is_default=excluded.is_default,
          meta_json=excluded.meta_json,
          updated_at=datetime('now')
        """,
        (
            row["id"],
            row["character_id"],
            row["name_zh"],
            name_en,
            row.get("skin_index"),
            row.get("pet_model_id") or "",
            row.get("kanmusu_dir") or "",
            row.get("sort_order") or 0,
            1 if row.get("is_default") else 0,
            row.get("meta_json") or "{}",
        ),
    )
    if replace_lines:
        conn.execute("DELETE FROM skin_lines WHERE skin_id=?", (row["id"],))
        for ln in row.get("lines") or []:
            conn.execute(
                """
                INSERT INTO skin_lines(
                  skin_id, wiki_key, label, lang, text, animation, audio_url, audio_relpath, sort_order
                ) VALUES (?,?,?,?,?,?,?,?,?)
                """,
                (
                    row["id"],
                    ln.get("wiki_key") or "",
                    ln.get("label") or "",
                    ln.get("lang") or "",
                    ln["text"],
                    ln.get("animation") or "",
                    ln.get("audio_url") or "",
                    ln.get("audio_relpath") or "",
                    int(ln.get("sort_order") or 0),
                ),
            )

def cmd_import_bundled_seed(args: argparse.Namespace) -> int:
    """Seed local DB from existing hanpet/bundled characters manifest (builtins)."""
    db = Path(args.db) if args.db else default_local_db()
    conn = connect(db)
    apply_schema(conn)
    manifest = load_json(
        bundled_roster_dir() / "characters" / "manifest.json",
        {"characters": []},
    )
    n_char = n_skin = 0
    for c in manifest.get("characters") or []:
        if not isinstance(c, dict) or not c.get("id"):
            continue
        upsert_character(
            conn,
            {
                "id": c["id"],
                "name_zh": c.get("name") or c["id"],
                "name_en": c.get("english_name") or "",
                "wiki_title": c.get("wiki_title") or "",
                "cv": c.get("cv") or "",
                "faction": c.get("faction") or "",
                "ship_type": c.get("ship_type") or "",
                "rarity": c.get("rarity") or "",
                "persona_id": c.get("persona_id") or c["id"],
                "source": c.get("source") or "bundled",
                "description": c.get("description") or "",
            },
        )
        n_char += 1
        conn.execute(
            "DELETE FROM skin_lines WHERE skin_id IN (SELECT id FROM skins WHERE character_id=?)",
            (c["id"],),
        )
        conn.execute("DELETE FROM skins WHERE character_id=?", (c["id"],))
        for i, s in enumerate(c.get("skins") or []):
            if not isinstance(s, dict) or not s.get("id"):
                continue
            raw_id = str(s["id"])
            db_id = skin_db_id(c["id"], raw_id)
            upsert_skin(
                conn,
                {
                    "id": db_id,
                    "character_id": c["id"],
                    "name_zh": s.get("name") or "默认",
                    "name_en": s.get("english_name") or s.get("name_en") or "",
                    "skin_index": s.get("skin_index"),
                    "pet_model_id": s.get("model_id") or "",
                    "kanmusu_dir": (s.get("kanmusu_dir") or ""),
                    "sort_order": i,
                    "is_default": bool(s.get("default")) or raw_id == "default",
                    "lines": [],
                },
                replace_lines=False,
            )
            n_skin += 1
    conn.commit()
    conn.close()
    emit({"ok": True, "db": str(db), "characters": n_char, "skins": n_skin})
    return 0

