"""Roster duplicate merge / folder-like purge (sheared from db.py C1)."""

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

# --- merge ---

def purge_folder_like_characters(
    conn: sqlite3.Connection, alias_map: dict[str, str] | None = None
) -> int:
    """Delete character rows whose ids are skin folders; merge into base first.

    Example: abeikelongbi_3 → merge/delete into abeikelongbi.
    Returns number of folder-like character rows removed.
    """
    alias_map = alias_map or LIVE2D_ALIASES
    donors = [
        str(r[0])
        for r in conn.execute("SELECT id FROM characters ORDER BY id")
        if is_folder_like_character_id(str(r[0]))
    ]
    removed = 0
    for donor in donors:
        base, _suffix = strip_skin(donor)
        base = alias_redirect_id(base, alias_map)
        if not base or base == donor:
            conn.execute("DELETE FROM skin_lines WHERE skin_id IN (SELECT id FROM skins WHERE character_id=?)", (donor,))
            conn.execute("DELETE FROM skins WHERE character_id=?", (donor,))
            conn.execute("DELETE FROM characters WHERE id=?", (donor,))
            removed += 1
            continue
        if not conn.execute("SELECT 1 FROM characters WHERE id=?", (base,)).fetchone():
            cn = (alias_map.get(base) or "").strip() or base
            upsert_character(
                conn,
                {
                    "id": base,
                    "name_zh": cn,
                    "name_en": "",
                    "wiki_title": cn,
                    "persona_id": base,
                    "source": "unpacked",
                },
            )
        _merge_character_into(conn, donor, base)
        _repoint_avatar_file(donor, base)
        removed += 1
    return removed

def _remap_skin_id(old_id: str, donor_cid: str, canon_cid: str) -> str:
    if old_id == donor_cid:
        return canon_cid
    prefix = donor_cid + "-"
    if old_id.startswith(prefix):
        return canon_cid + "-" + old_id[len(prefix) :]
    return f"{canon_cid}-{old_id}"

def _merge_character_into(
    conn: sqlite3.Connection, donor_cid: str, canon_cid: str
) -> None:
    """Move skins/lines from donor → canon, then delete donor character."""
    if donor_cid == canon_cid:
        return
    donor_skins = conn.execute(
        "SELECT * FROM skins WHERE character_id=?", (donor_cid,)
    ).fetchall()
    for sk in donor_skins:
        old_id = str(sk["id"])
        new_id = _remap_skin_id(old_id, donor_cid, canon_cid)
        existing = conn.execute(
            "SELECT * FROM skins WHERE id=?", (new_id,)
        ).fetchone()
        if existing:
            # fill empty bind fields on canon
            pet = (existing["pet_model_id"] or "") or (sk["pet_model_id"] or "")
            kdir = (existing["kanmusu_dir"] or "") or (sk["kanmusu_dir"] or "")
            conn.execute(
                "UPDATE skins SET pet_model_id=?, kanmusu_dir=?, updated_at=datetime('now') WHERE id=?",
                (pet, kdir, new_id),
            )
            tgt_n = conn.execute(
                "SELECT count(*) FROM skin_lines WHERE skin_id=?", (new_id,)
            ).fetchone()[0]
            if tgt_n == 0:
                conn.execute(
                    "UPDATE skin_lines SET skin_id=? WHERE skin_id=?",
                    (new_id, old_id),
                )
            else:
                conn.execute("DELETE FROM skin_lines WHERE skin_id=?", (old_id,))
            conn.execute("DELETE FROM skins WHERE id=?", (old_id,))
        else:
            # rename via insert+line move+delete (PK change)
            cols = [
                str(d[1])
                for d in conn.execute("PRAGMA table_info(skins)").fetchall()
            ]
            row = {k: sk[k] for k in sk.keys()}
            row["id"] = new_id
            row["character_id"] = canon_cid
            placeholders = ",".join("?" for _ in cols)
            conn.execute(
                f"INSERT INTO skins({','.join(cols)}) VALUES ({placeholders})",
                tuple(row.get(c) for c in cols),
            )
            conn.execute(
                "UPDATE skin_lines SET skin_id=? WHERE skin_id=?",
                (new_id, old_id),
            )
            conn.execute("DELETE FROM skins WHERE id=?", (old_id,))
    # any leftover
    conn.execute("DELETE FROM skins WHERE character_id=?", (donor_cid,))
    conn.execute("DELETE FROM characters WHERE id=?", (donor_cid,))

def repair_uppercased_hash_character_ids(conn: sqlite3.Connection) -> int:
    """Undo mistaken uppercase of BWIKI hash ids (P00e01b29 → p00e01b29)."""
    changed = 0
    ids = [str(r[0]) for r in conn.execute("SELECT id FROM characters").fetchall()]
    for cid in ids:
        if not conn.execute("SELECT 1 FROM characters WHERE id=?", (cid,)).fetchone():
            continue
        if not HASH_PERSONA_ID.match(cid):
            continue
        canon = cid.lower()
        if canon == cid:
            continue
        if conn.execute("SELECT 1 FROM characters WHERE id=?", (canon,)).fetchone():
            _merge_character_into(conn, cid, canon)
            _repoint_avatar_file(cid, canon)
        else:
            row = conn.execute("SELECT * FROM characters WHERE id=?", (cid,)).fetchone()
            if not row:
                continue
            payload = {k: row[k] for k in row.keys()}
            payload["id"] = canon
            if str(payload.get("persona_id") or "").casefold() == cid.casefold():
                payload["persona_id"] = canon
            upsert_character(conn, payload)
            _merge_character_into(conn, cid, canon)
            _repoint_avatar_file(cid, canon)
        changed += 1
    return changed

def canonicalize_ship_code_character_ids(conn: sqlite3.Connection) -> int:
    """Rename z46 → Z46 (etc.) and merge into existing uppercase row if present."""
    changed = 0
    ids = [str(r[0]) for r in conn.execute("SELECT id FROM characters").fetchall()]
    for cid in ids:
        if not conn.execute("SELECT 1 FROM characters WHERE id=?", (cid,)).fetchone():
            continue
        if not is_ship_code_id(cid):
            continue
        canon = normalize_character_id(cid)
        if canon == cid:
            row = conn.execute(
                "SELECT name_zh, wiki_title FROM characters WHERE id=?", (cid,)
            ).fetchone()
            if not row:
                continue
            nz = (row["name_zh"] or "").strip()
            wt = (row["wiki_title"] or "").strip()
            new_nz = normalize_character_id(nz) if is_ship_code_id(nz) else nz
            new_wt = normalize_character_id(wt) if is_ship_code_id(wt) else wt
            if new_nz != nz or new_wt != wt:
                conn.execute(
                    "UPDATE characters SET name_zh=?, wiki_title=?, updated_at=datetime('now') WHERE id=?",
                    (new_nz, new_wt, cid),
                )
                changed += 1
            continue
        if conn.execute("SELECT 1 FROM characters WHERE id=?", (canon,)).fetchone():
            _merge_character_into(conn, cid, canon)
            _repoint_avatar_file(cid, canon)
        else:
            row = conn.execute("SELECT * FROM characters WHERE id=?", (cid,)).fetchone()
            if not row:
                continue
            payload = {k: row[k] for k in row.keys()}
            payload["id"] = canon
            payload["persona_id"] = canon
            nz = str(payload.get("name_zh") or "").strip()
            wt = str(payload.get("wiki_title") or "").strip()
            if is_ship_code_id(nz) or nz.casefold() == canon.casefold():
                payload["name_zh"] = (
                    canon
                    if nz.casefold() == canon.casefold()
                    else normalize_character_id(nz)
                )
            if is_ship_code_id(wt) or wt.casefold() == canon.casefold():
                payload["wiki_title"] = (
                    canon
                    if wt.casefold() == canon.casefold()
                    else normalize_character_id(wt)
                )
            upsert_character(conn, payload)
            _merge_character_into(conn, cid, canon)
            _repoint_avatar_file(cid, canon)
        changed += 1
    return changed

def merge_roster_duplicates_by_name(
    conn: sqlite3.Connection, alias_map: dict[str, str] | None = None
) -> int:
    """合并同中文名多角色行（如 aijiang+aijier）。返回合并掉的 donor 数。

    名称按 casefold 归组，避免 wiki stub `z46` 与正式 `Z46` 双开。
    随后将舰船代号 id 规范为大写（z46 → Z46）。
    """
    from collections import defaultdict

    alias_map = alias_map or LIVE2D_ALIASES
    by_name: dict[str, list[str]] = defaultdict(list)
    name_display: dict[str, str] = {}
    source_by_id: dict[str, str] = {}

    def _merge_name_key(name: str, cid: str) -> tuple[str, str]:
        """Group key + display: ASCII stubs via alias (i404 → 伊404)."""
        raw = (name or "").strip()
        if not raw:
            return "", ""
        # Already has a real CJK display name — trust it
        if CJK_RE.search(raw):
            return raw.casefold(), raw
        # Stub still using folder slug / hull-looking ASCII as name_zh
        for cand in (raw, cid):
            cn = _alias_cn_for_slug(cand, alias_map)
            if cn and CJK_RE.search(cn):
                return cn.casefold(), cn
        return raw.casefold(), raw

    for row in conn.execute("SELECT id, name_zh, source FROM characters"):
        name = (row["name_zh"] or "").strip()
        cid = str(row["id"])
        source_by_id[cid] = (row["source"] or "").strip()
        if not name:
            continue
        key, display = _merge_name_key(name, cid)
        if not key:
            continue
        by_name[key].append(cid)
        prev = name_display.get(key)
        if prev is None or (prev.isascii() and not display.isascii()) or (
            prev.islower() and display != display.lower()
        ):
            name_display[key] = display
    merged = 0
    for key, ids in by_name.items():
        if len(ids) < 2:
            continue
        name = name_display.get(key) or key

        def _skin_count(cid: str) -> int:
            return int(
                conn.execute(
                    "SELECT count(*) FROM skins WHERE character_id=?", (cid,)
                ).fetchone()[0]
            )

        # Wiki rows win over unpacked folder stubs (i404 stub vs 伊404 wiki hash)
        wiki_ids = [i for i in ids if source_by_id.get(i) == "wiki"]
        if wiki_ids:
            canon = max(wiki_ids, key=_skin_count)
        else:
            pref = preferred_slug_for_cn(name, alias_map)
            if pref and pref not in ids:
                for i in ids:
                    if i.casefold() == pref.casefold():
                        pref = i
                        break
                else:
                    pref = None
            if not pref:
                for i in ids:
                    if i.casefold() == key and not HASH_PERSONA_ID.match(i):
                        pref = i
                        break
            non_hash = [i for i in ids if not HASH_PERSONA_ID.match(i)]
            if pref and pref in ids:
                canon = pref
            elif non_hash:
                # Prefer uppercase ship-code id when present
                ship_ids = [i for i in non_hash if is_ship_code_id(i)]
                pool = ship_ids or non_hash
                canon = max(
                    pool,
                    key=lambda i: (i == normalize_character_id(i), _skin_count(i)),
                )
            else:
                canon = max(ids, key=_skin_count)
        for donor in ids:
            if donor == canon:
                continue
            _merge_character_into(conn, donor, canon)
            _repoint_avatar_file(donor, canon)
            merged += 1
        disp = name_display.get(key) or name
        if is_ship_code_id(disp):
            disp = normalize_character_id(disp)
        if disp and disp != key:
            conn.execute(
                "UPDATE characters SET name_zh=?, wiki_title=COALESCE(NULLIF(wiki_title,''), ?) "
                "WHERE id=?",
                (disp, disp, canon),
            )
    merged += repair_uppercased_hash_character_ids(conn)
    merged += canonicalize_ship_code_character_ids(conn)
    return merged

def enrich_unpacked_character_names(
    conn: sqlite3.Connection,
    wiki_db: Path | None = None,
    alias_map: dict[str, str] | None = None,
) -> int:
    """Fill Chinese name/wiki_title for stubs where name_zh == id (pinyin folder).

    Uses catalog pinyin slug index + live2d aliases (kubo→可怖 when pinyin is kebu).
    Returns number of characters updated.
    """
    from roster.avatar_fetch import default_wiki_db, resolve_catalog_by_slug

    wiki_db = Path(wiki_db) if wiki_db else default_wiki_db()
    if not wiki_db.is_file():
        return 0
    alias_map = alias_map or {
        **LIVE2D_ALIASES,
        **load_json(repo_root() / "data/wiki/live2d-aliases.json", {}),
    }
    updated = 0
    rows = conn.execute(
        "SELECT id, name_zh, wiki_title, name_en FROM characters ORDER BY id"
    ).fetchall()
    for row in rows:
        cid = str(row["id"])
        name_zh = (row["name_zh"] or "").strip()
        wiki_title = (row["wiki_title"] or "").strip()
        name_en = (row["name_en"] or "").strip()
        # Only touch obvious stubs (name still equals slug / empty CN)
        if name_zh and name_zh != cid and not name_zh.isascii():
            continue
        if wiki_title and wiki_title != cid and not wiki_title.isascii():
            continue
        base, _ = strip_skin(cid)
        # Prefer explicit live2d aliases (i404→伊404; pinyin of 伊 is yi404)
        aliased = (
            _alias_cn_for_slug(cid, alias_map)
            or _alias_cn_for_slug(base, alias_map)
            or _alias_cn_for_slug(name_zh, alias_map)
        )
        hit = resolve_catalog_by_slug(wiki_db, cid) or resolve_catalog_by_slug(
            wiki_db, base
        )
        if aliased and CJK_RE.search(aliased):
            display = aliased
            title = aliased
        elif hit:
            display = (hit.get("display_name") or hit.get("wiki_title") or "").strip()
            title = (hit.get("wiki_title") or display).strip()
        else:
            continue
        if not display:
            continue
        # Keep English code as name_en when stub was folder code (kubo)
        new_en = name_en
        if (not new_en or new_en == cid or HASH_PERSONA_ID.match(new_en)) and cid.isascii():
            if not HASH_PERSONA_ID.match(cid):
                new_en = cid
        conn.execute(
            """
            UPDATE characters SET
              name_zh=?,
              wiki_title=?,
              name_en=?,
              source=CASE WHEN source='unpacked' THEN 'wiki' ELSE source END,
              updated_at=datetime('now')
            WHERE id=?
            """,
            (display, title, new_en, cid),
        )
        updated += 1
    return updated

def _repoint_avatar_file(donor_cid: str, canon_cid: str) -> None:
    """If donor has an avatar file and canon does not, rename/copy to canon id."""
    if donor_cid == canon_cid:
        return
    try:
        from roster.avatar_fetch import (
            avatars_dir,
            invalidate_avatar_index,
            resolve_avatar_file,
        )
    except ImportError:
        return
    if resolve_avatar_file(canon_cid):
        return
    src = resolve_avatar_file(donor_cid)
    if not src:
        return
    dest = avatars_dir() / f"{canon_cid}{src.suffix}"
    try:
        if not dest.exists():
            src.replace(dest)
        else:
            src.unlink(missing_ok=True)
        invalidate_avatar_index()
    except OSError:
        try:
            import shutil as _shutil

            _shutil.copy2(src, dest)
            invalidate_avatar_index()
        except OSError as exc:
            logging.warning(
                "avatar repoint failed donor=%s canon=%s: %s",
                donor_cid,
                canon_cid,
                exc,
            )

