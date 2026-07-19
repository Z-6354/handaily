"""Roster pet/skin bind + repair pipeline (sheared from db.py C1)."""

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

import roster.folder_rules as _folder_rules
_pull(_folder_rules)

import roster.aliases as _aliases
_pull(_aliases)

import roster.ids as _ids
_pull(_ids)

import roster.schema as _schema
_pull(_schema)

import roster.crud as _crud
_pull(_crud)

# --- bind ---

def _build_cn_to_slug(
    conn: sqlite3.Connection,
    wiki: sqlite3.Connection,
    alias_map: dict[str, str],
) -> dict[str, str]:
    """中文名 → 权威拼音 / 已有 id。"""
    rev: dict[str, str] = {}
    # 先按中文聚合别名，写入权威 slug（避免 aijiang/aijier 谁先谁后）
    seen_cn: set[str] = set()
    for _slug, cn in alias_map.items():
        cn = (cn or "").strip()
        if not cn or cn in seen_cn:
            continue
        seen_cn.add(cn)
        pref = preferred_slug_for_cn(cn, alias_map)
        if pref:
            rev[cn] = pref
    try:
        for row in wiki.execute(
            "SELECT folder, wiki_title, display_name FROM live2d_mappings"
        ):
            base, _ = strip_skin(row["folder"] or "")
            base = alias_redirect_id(base, alias_map)
            for key in ((row["display_name"] or "").strip(), (row["wiki_title"] or "").strip()):
                if key and base and key not in rev:
                    rev[key] = base
    except sqlite3.Error:
        pass
    for row in conn.execute("SELECT id, name_zh, wiki_title, source FROM characters"):
        cid = alias_redirect_id(str(row["id"] or ""), alias_map)
        src = (row["source"] or "").strip()
        for key in ((row["name_zh"] or "").strip(), (row["wiki_title"] or "").strip()):
            if not key or not cid:
                continue
            existing = rev.get(key)
            if not existing:
                rev[key] = cid
                continue
            # Wiki / hash persona ids beat pinyin alias stubs (伊404 → p85… not i404)
            if src == "wiki" and HASH_PERSONA_ID.match(cid) and not HASH_PERSONA_ID.match(
                existing
            ):
                rev[key] = cid
    return rev

def _resolve_character_id(cn: str, cn_to_slug: dict[str, str]) -> str:
    cn = (cn or "").strip()
    if not cn:
        return _stable_wiki_id("unknown")
    # Hull codes always use uppercase id — never hash (z46 / Z46 → Z46)
    if is_ship_code_id(cn):
        return normalize_character_id(cn)
    if cn in cn_to_slug:
        return alias_redirect_id(cn_to_slug[cn])
    for k, v in cn_to_slug.items():
        if k.casefold() == cn.casefold():
            return alias_redirect_id(v)
    return _stable_wiki_id(cn)

def resolve_bind_skin_id(
    conn: sqlite3.Connection, cid: str, suffix: str
) -> str | None:
    """Map unpacked folder suffix to an existing roster skin id (no create).

    Azur Lane folder convention (common):
      slug       → default
      slug_2     → first alternate (Wiki skin1)
      slug_3     → second alternate (Wiki skin2)
    So digit N>=2 prefers Wiki skin{N-1} before skin{N}.
    """
    if not suffix or suffix == "0":
        row = conn.execute(
            "SELECT id FROM skins WHERE character_id=? AND is_default=1 LIMIT 1",
            (cid,),
        ).fetchone()
        if row:
            return str(row[0])
        sid = skin_db_id(cid, "default")
        exists = conn.execute("SELECT 1 FROM skins WHERE id=?", (sid,)).fetchone()
        return sid if exists else None

    # DOA collab: slug_doa → default; slug_2_doa → same digit mapping as slug_2
    if suffix.lower() == "doa":
        return resolve_bind_skin_id(conn, cid, "")
    m_doa = re.fullmatch(r"(\d+)_doa", suffix, re.I)
    if m_doa:
        return resolve_bind_skin_id(conn, cid, m_doa.group(1))

    # μ兵装 / 小舰娘: slug_idol / slug_younv → that variant's default skin
    if suffix.lower() in ("idol", "younv"):
        return resolve_bind_skin_id(conn, cid, "")
    m_var = re.fullmatch(r"(\d+)_(idol|younv)", suffix, re.I)
    if m_var:
        return resolve_bind_skin_id(conn, cid, m_var.group(1))

    # 誓约：桌宠文件夹 ``{slug}_h`` → ``{cid}-oath``（无舰娘）
    if suffix.lower() == "h":
        oath_sid = f"{cid}-oath"
        if conn.execute("SELECT 1 FROM skins WHERE id=?", (oath_sid,)).fetchone():
            return oath_sid
        row = conn.execute(
            "SELECT id FROM skins WHERE character_id=? AND id LIKE '%-oath' LIMIT 1",
            (cid,),
        ).fetchone()
        return str(row[0]) if row else None

    # Pure digit → Wiki skin slots. Compound (3_1 / 3_hx) intentionally
    # does not fall back to the parent skin — those get dedicated L2D rows.
    #
    # BLHX folders: bare ``slug`` = default (原皮也算皮肤); first costume is
    # ``slug_2`` → Wiki skin1. There is no ``slug_1``.
    if suffix.isdigit():
        n = int(suffix)
        if n == 1:
            return None
        # Prefer BLHX _N → skin{N-1}; fall back to skin{N} / skin_index.
        wiki_nums: list[int] = []
        if n >= 2:
            wiki_nums.append(n - 1)
        wiki_nums.append(n)
        seen: set[int] = set()
        ordered: list[int] = []
        for w in wiki_nums:
            if w not in seen and w >= 1:
                seen.add(w)
                ordered.append(w)
        for w in ordered:
            sid = f"{cid}-skin{w}"
            if conn.execute("SELECT 1 FROM skins WHERE id=?", (sid,)).fetchone():
                return sid
        for w in ordered:
            row = conn.execute(
                "SELECT id FROM skins WHERE character_id=? AND skin_index=? LIMIT 1",
                (cid, w),
            ).fetchone()
            if row:
                return str(row[0])
        return None

    # non-digit / compound: match existing bind by folder-shaped kanmusu_dir or id
    row = conn.execute(
        "SELECT id FROM skins WHERE character_id=? AND "
        "(kanmusu_dir=? OR id=? OR name_zh LIKE ?) LIMIT 1",
        (cid, f"{cid}_{suffix}", f"{cid}-{suffix}", f"%{suffix}%"),
    ).fetchone()
    return str(row[0]) if row else None

def reclaim_l2d_orphan_into_skin(
    conn: sqlite3.Connection,
    cid: str,
    target_sid: str,
    *,
    folder: str,
    suffix: str,
) -> bool:
    """Merge a leftover {cid}-L2D-{suffix} row into the Wiki/target skin.

    Typical bug residue: lines on skin1, assets on L2D-2. After reclaim, assets
    (and any orphan-only lines) live on target; orphan row is deleted.
    Returns True if an orphan was removed.
    """
    if not suffix:
        orphan_id = f"{cid}-L2D-default"
    else:
        orphan_id = f"{cid}-L2D-{suffix}"
    orphan_id = re.sub(r"[^a-zA-Z0-9._-]+", "-", orphan_id)
    if orphan_id == target_sid:
        return False
    row = conn.execute(
        "SELECT id, kanmusu_dir, pet_model_id, meta_json FROM skins WHERE id=?",
        (orphan_id,),
    ).fetchone()
    if not row:
        # Same folder bound on a differently named L2D leftover
        row = conn.execute(
            "SELECT id, kanmusu_dir, pet_model_id, meta_json FROM skins "
            "WHERE character_id=? AND id LIKE ? AND id != ? AND kanmusu_dir=? LIMIT 1",
            (cid, f"{cid}-L2D-%", target_sid, folder),
        ).fetchone()
    if not row:
        return False
    orphan_id = str(row[0])
    if orphan_id == target_sid:
        return False

    o_kanmusu = (row[1] or "").strip()
    o_pet = (row[2] or "").strip()
    tgt = conn.execute(
        "SELECT kanmusu_dir, pet_model_id, meta_json FROM skins WHERE id=?",
        (target_sid,),
    ).fetchone()
    if not tgt:
        return False
    t_kanmusu = (tgt[0] or "").strip()
    t_pet = (tgt[1] or "").strip()
    # 誓约：只合桌宠，不写舰娘目录
    if is_oath_skin_id(target_sid):
        new_kanmusu = ""
        new_pet = t_pet or o_pet
    else:
        new_kanmusu = t_kanmusu or o_kanmusu or folder
        new_pet = t_pet or o_pet

    # Move lines if target empty and orphan has some
    t_n = conn.execute(
        "SELECT COUNT(*) FROM skin_lines WHERE skin_id=?", (target_sid,)
    ).fetchone()[0]
    o_n = conn.execute(
        "SELECT COUNT(*) FROM skin_lines WHERE skin_id=?", (orphan_id,)
    ).fetchone()[0]
    if t_n == 0 and o_n > 0:
        conn.execute(
            "UPDATE skin_lines SET skin_id=? WHERE skin_id=?",
            (target_sid, orphan_id),
        )

    # Merge meta: keep target lines_import; copy slot_key if missing
    try:
        t_meta = json.loads(tgt[2] or "{}")
        if not isinstance(t_meta, dict):
            t_meta = {}
    except json.JSONDecodeError:
        t_meta = {}
    try:
        o_meta = json.loads(row[3] or "{}")
        if not isinstance(o_meta, dict):
            o_meta = {}
    except json.JSONDecodeError:
        o_meta = {}
    if "slot_key" not in t_meta and o_meta.get("slot_key"):
        t_meta["slot_key"] = o_meta["slot_key"]
    t_meta["reclaimed_from"] = orphan_id

    conn.execute(
        """
        UPDATE skins SET
          kanmusu_dir=?,
          pet_model_id=?,
          meta_json=?,
          updated_at=datetime('now')
        WHERE id=?
        """,
        (
            new_kanmusu,
            new_pet,
            json.dumps(t_meta, ensure_ascii=False),
            target_sid,
        ),
    )
    conn.execute("DELETE FROM skins WHERE id=?", (orphan_id,))
    return True

def _character_has_wiki_skins(conn: sqlite3.Connection, cid: str) -> bool:
    """True if character already has canonical Wiki slots (not only L2D orphans)."""
    row = conn.execute(
        """
        SELECT 1 FROM skins
        WHERE character_id=? AND id NOT LIKE '%-L2D-%'
        LIMIT 1
        """,
        (cid,),
    ).fetchone()
    return row is not None


def prune_unmapped_l2d_orphan_skins(conn: sqlite3.Connection) -> int:
    """Delete L2D-* rows that cannot map onto any existing Wiki skin slot.

    Extra game folders (e.g. i404_3 when Wiki only has default+skin1) must not
    remain as a third empty/orphan skin on a Wiki character.
    """
    deleted = 0
    rows = conn.execute(
        "SELECT id, character_id FROM skins WHERE id LIKE '%-L2D-%'"
    ).fetchall()
    pat = re.compile(r"^(.+)-L2D-(.+)$")
    for sid, cid in rows:
        sid_s = str(sid)
        cid_s = str(cid)
        if not _character_has_wiki_skins(conn, cid_s):
            continue
        m = pat.match(sid_s)
        if not m:
            continue
        suffix = m.group(2)
        bind_suffix = "" if suffix == "default" else suffix
        target = resolve_bind_skin_id(conn, cid_s, bind_suffix)
        if target and target != sid_s and "-L2D-" not in target:
            # Reclaimable onto a Wiki slot — leave for reclaim pass
            continue
        conn.execute("DELETE FROM skin_lines WHERE skin_id=?", (sid_s,))
        conn.execute("DELETE FROM skins WHERE id=?", (sid_s,))
        deleted += 1
    return deleted


def repair_l2d_folder_orphans(conn: sqlite3.Connection) -> dict[str, int]:
    """Merge leftover {cid}-L2D-{N} rows into Wiki skin{N-1} (BLHX folder map).

    Folder slug_N (N>=2) ↔ Wiki skin{N-1}. Historical binds created L2D-N while
    lines stayed on empty skin{N-1}; this heals the whole roster in one pass.
    """
    stats = {"scanned": 0, "reclaimed": 0, "skipped": 0, "pruned": 0}
    rows = conn.execute(
        "SELECT id, character_id, kanmusu_dir, pet_model_id FROM skins "
        "WHERE id LIKE '%-L2D-%'"
    ).fetchall()
    pat = re.compile(r"^(.+)-L2D-(\d+)$")
    for sid, cid, kanmusu, pet in rows:
        stats["scanned"] += 1
        m = pat.match(str(sid))
        if not m:
            stats["skipped"] += 1
            continue
        cid_s = str(m.group(1))
        n = int(m.group(2))
        if n < 2:
            stats["skipped"] += 1
            continue
        wiki_sid = f"{cid_s}-skin{n - 1}"
        wiki = conn.execute(
            "SELECT id, kanmusu_dir, pet_model_id FROM skins WHERE id=?",
            (wiki_sid,),
        ).fetchone()
        if not wiki:
            stats["skipped"] += 1
            continue
        o_kanmusu = (kanmusu or "").strip()
        o_pet = (pet or "").strip()
        w_kanmusu = (wiki[1] or "").strip()
        w_pet = (wiki[2] or "").strip()
        orphan_has = bool(o_kanmusu or o_pet)
        wiki_empty = not w_kanmusu and not w_pet
        same_folder = bool(o_kanmusu) and o_kanmusu == w_kanmusu
        if not orphan_has:
            stats["skipped"] += 1
            continue
        if not (wiki_empty or same_folder or not w_kanmusu):
            # Wiki already bound to a different folder — leave alone
            stats["skipped"] += 1
            continue
        folder = o_kanmusu or f"{cid_s}_{n}"
        if reclaim_l2d_orphan_into_skin(
            conn, cid_s, wiki_sid, folder=folder, suffix=str(n)
        ):
            # Ensure folder lands on wiki skin even if reclaim kept prior empty
            if o_kanmusu:
                conn.execute(
                    "UPDATE skins SET kanmusu_dir=?, "
                    "pet_model_id=CASE WHEN ?!='' THEN ? ELSE pet_model_id END, "
                    "updated_at=datetime('now') WHERE id=?",
                    (o_kanmusu, o_pet, o_pet, wiki_sid),
                )
            stats["reclaimed"] += 1
        else:
            stats["skipped"] += 1
    stats["pruned"] = prune_unmapped_l2d_orphan_skins(conn)
    return stats

def repair_misindexed_wiki_folder_binds(conn: sqlite3.Connection) -> dict[str, int]:
    """Move assets when Wiki skinK was wrongly bound to folder _{K} instead of _{K+1}.

    Old resolve used skin{N} for folder _N, so with both skin1+skin2 present,
    slug_2 landed on skin2 while skin1 (lines) stayed empty. Move assets to
    skin{N-1}; do not move lines (they already sit on the correct Wiki slot).
    """
    stats = {"scanned": 0, "moved": 0, "skipped": 0}
    skin_pat = re.compile(r"^(.+)-skin(\d+)$")
    folder_pat = re.compile(r"_(\d+)$")
    rows = conn.execute(
        "SELECT id, character_id, kanmusu_dir, pet_model_id FROM skins "
        "WHERE id LIKE '%-skin%' AND IFNULL(kanmusu_dir,'') != ''"
    ).fetchall()
    moves: list[tuple[int, str, str, str, str]] = []
    for sid, cid, kanmusu, pet in rows:
        stats["scanned"] += 1
        sm = skin_pat.match(str(sid))
        if not sm:
            stats["skipped"] += 1
            continue
        k = int(sm.group(2))
        kd = (kanmusu or "").strip()
        fm = folder_pat.search(kd)
        if not fm:
            stats["skipped"] += 1
            continue
        n = int(fm.group(1))
        # Correct BLHX map: folder _N → skin{N-1}. Wrong classic bug: skinK has _K.
        if n != k:
            stats["skipped"] += 1
            continue
        if n < 2:
            stats["skipped"] += 1
            continue
        target = f"{sm.group(1)}-skin{n - 1}"
        if target == sid:
            stats["skipped"] += 1
            continue
        tgt = conn.execute(
            "SELECT kanmusu_dir, pet_model_id FROM skins WHERE id=?", (target,)
        ).fetchone()
        if not tgt:
            stats["skipped"] += 1
            continue
        if (tgt[0] or "").strip() or (tgt[1] or "").strip():
            stats["skipped"] += 1
            continue
        moves.append((n, str(sid), target, kd, (pet or "").strip()))

    for _n, src, target, kd, pet in sorted(moves, key=lambda x: x[0]):
        tgt = conn.execute(
            "SELECT kanmusu_dir, pet_model_id FROM skins WHERE id=?", (target,)
        ).fetchone()
        if not tgt or (tgt[0] or "").strip() or (tgt[1] or "").strip():
            stats["skipped"] += 1
            continue
        conn.execute(
            """
            UPDATE skins SET
              kanmusu_dir=?,
              pet_model_id=CASE WHEN ?!='' THEN ? ELSE pet_model_id END,
              updated_at=datetime('now')
            WHERE id=?
            """,
            (kd, pet, pet, target),
        )
        conn.execute(
            """
            UPDATE skins SET
              kanmusu_dir='',
              pet_model_id='',
              updated_at=datetime('now')
            WHERE id=?
            """,
            (src,),
        )
        stats["moved"] += 1
    return stats

def repair_blhx_skin_folder_binds(conn: sqlite3.Connection) -> dict[str, int]:
    """Full heal: fix misindexed Wiki binds, then reclaim L2D orphans.

    Runs until a pass makes no further changes (chains like skin3←_3 then
    L2D-4→skin3 need multiple passes).
    """
    total = {
        "passes": 0,
        "misindexed_scanned": 0,
        "misindexed_moved": 0,
        "misindexed_skipped": 0,
        "l2d_scanned": 0,
        "l2d_reclaimed": 0,
        "l2d_skipped": 0,
        "l2d_pruned": 0,
    }
    for _ in range(12):
        a = repair_misindexed_wiki_folder_binds(conn)
        b = repair_l2d_folder_orphans(conn)
        total["passes"] += 1
        total["misindexed_scanned"] = a["scanned"]
        total["misindexed_moved"] += a["moved"]
        total["misindexed_skipped"] = a["skipped"]
        total["l2d_scanned"] = b["scanned"]
        total["l2d_reclaimed"] += b["reclaimed"]
        total["l2d_skipped"] = b["skipped"]
        total["l2d_pruned"] += int(b.get("pruned") or 0)
        if not a["moved"] and not b["reclaimed"] and not b.get("pruned"):
            break
    return total

def ensure_skin_for_unpacked(
    conn: sqlite3.Connection,
    cid: str,
    folder: str,
    suffix: str,
) -> str:
    """Create a roster skin row for an unpacked folder when Wiki has no match."""
    from common.unpack_complete import is_hx_slug

    if is_hx_slug(folder) or (suffix or "").lower() == "hx" or (suffix or "").lower().endswith(
        "_hx"
    ):
        raise ValueError(f"refusing hx skin folder: {folder!r}")
    sid = f"{cid}-L2D-{suffix}" if suffix else f"{cid}-L2D-default"
    # Keep id filesystem-safe / stable
    sid = re.sub(r"[^a-zA-Z0-9._-]+", "-", sid)
    exists = conn.execute("SELECT 1 FROM skins WHERE id=?", (sid,)).fetchone()
    skin_index = None
    primary = suffix.split("_", 1)[0] if suffix else ""
    if primary.isdigit():
        skin_index = int(primary)
    name_zh = skin_label(suffix) if suffix else "默认"
    if suffix and not suffix.isdigit() and "_" in suffix:
        name_zh = f"皮肤{suffix}"
    if exists:
        conn.execute(
            """
            UPDATE skins SET
              kanmusu_dir=?,
              name_zh=COALESCE(NULLIF(name_zh,''), ?),
              updated_at=datetime('now')
            WHERE id=?
            """,
            (folder, name_zh, sid),
        )
        return sid
    upsert_skin(
        conn,
        {
            "id": sid,
            "character_id": cid,
            "name_zh": name_zh,
            "name_en": "",
            "is_default": 1 if not suffix else 0,
            "skin_index": skin_index,
            "kanmusu_dir": folder,
            "source": "unpacked",
            "lines": [],
        },
        replace_lines=False,
    )
    return sid

def _bind_paths_for_folder(
    unpacked_dir: Path, pet_models: Path, folder: str
) -> tuple[str, str]:
    """Return (kanmusu_dir, pet_model_id) for a non-oath unpacked folder.

    Cubism folders remain 舰娘; optional Spine mirror under live2d is 桌宠.
    Spine-only unpacked (no Cubism): still 舰娘; if the same slug exists under
    live2d with Spine assets, also bind 桌宠 so the UI shows local skin files.
    """
    from roster.skin_probe import _has_cubism_assets, _has_spine_assets

    kanmusu_dir = folder
    pet_model_id = ""
    has_cubism = _has_cubism_assets(unpacked_dir)
    has_spine = _has_spine_assets(unpacked_dir)

    def _pet_mirror() -> str:
        if (pet_models / folder).is_dir() and _has_spine_assets(pet_models / folder):
            return folder
        if (pet_models / f"skin-{folder}").is_dir() and _has_spine_assets(
            pet_models / f"skin-{folder}"
        ):
            return f"skin-{folder}"
        return ""

    if has_cubism:
        pet_model_id = _pet_mirror()
    elif has_spine:
        # Spine-only painting: kanmusu = unpacked; pet = live2d mirror when present
        pet_model_id = _pet_mirror()
    else:
        # Unknown / incomplete folder: keep prior dual-bind when live2d mirror exists
        if (pet_models / folder).is_dir():
            pet_model_id = folder
        elif (pet_models / f"skin-{folder}").is_dir():
            pet_model_id = f"skin-{folder}"
    return kanmusu_dir, pet_model_id

def _lookup_character_id_by_cn(
    conn: sqlite3.Connection,
    cn: str,
    *,
    cn_to_slug: dict[str, str],
) -> str | None:
    row = conn.execute(
        """
        SELECT id FROM characters
        WHERE name_zh=? OR wiki_title=?
        ORDER BY CASE
          WHEN source='wiki' THEN 0
          WHEN id LIKE 'p%' THEN 1
          ELSE 2
        END
        LIMIT 1
        """,
        (cn, cn),
    ).fetchone()
    if row:
        return str(row[0])
    return cn_to_slug.get(cn) or _resolve_character_id(cn, cn_to_slug)


def _resolve_character_id_for_folder(
    conn: sqlite3.Connection,
    base: str,
    *,
    alias_map: dict[str, str],
    cn_to_slug: dict[str, str],
    suffix: str = "",
) -> str | None:
    """Map unpack folder base(+suffix) → roster character id.

    ``*_younv`` → 小XX；``*_idol`` → XX(μ兵装)；勿绑回成人设。
    """
    cn = (alias_map.get(base) or alias_map.get(base.lower()) or "").strip() or None
    cid: str | None = None
    if cn:
        variant_cn = variant_character_name_for_suffix(cn, suffix)
        if variant_cn:
            cid = _lookup_character_id_by_cn(conn, variant_cn, cn_to_slug=cn_to_slug)
            if cid:
                redirected = alias_redirect_id(cid, alias_map)
                if conn.execute(
                    "SELECT 1 FROM characters WHERE id=?", (redirected,)
                ).fetchone():
                    return redirected
                return cid
            # Variant folder but no 小XX / μ row — do not fall back to adult
            if _suffix_variant_kind(suffix):
                return None
        cid = _lookup_character_id_by_cn(conn, cn, cn_to_slug=cn_to_slug)
    if not cid and not _suffix_variant_kind(suffix):
        # Default skin already bound to this bare folder (danfo vs pinyin danfu)
        row = conn.execute(
            """
            SELECT character_id FROM skins
            WHERE is_default=1
              AND (pet_model_id=? OR kanmusu_dir=?)
            LIMIT 1
            """,
            (base, base),
        ).fetchone()
        if row:
            cid = str(row[0])
    if not cid:
        redirected = alias_redirect_id(base, alias_map)
        if conn.execute(
            "SELECT 1 FROM characters WHERE id=?", (redirected,)
        ).fetchone():
            return redirected
        if conn.execute("SELECT 1 FROM characters WHERE id=?", (base,)).fetchone():
            return base
        return None
    redirected = alias_redirect_id(cid, alias_map)
    if conn.execute(
        "SELECT 1 FROM characters WHERE id=?", (redirected,)
    ).fetchone():
        return redirected
    return cid


def repair_misbound_variant_pets(conn: sqlite3.Connection) -> int:
    """Clear ``*_younv`` / ``*_idol`` pets wrongly set on adult (non-variant) characters."""
    cleared = 0
    rows = conn.execute(
        """
        SELECT s.id, s.pet_model_id, c.name_zh
        FROM skins s
        JOIN characters c ON c.id = s.character_id
        WHERE IFNULL(s.pet_model_id,'') != ''
          AND (s.pet_model_id LIKE '%younv' OR s.pet_model_id LIKE '%idol')
        """
    ).fetchall()
    for sid, pet_id, name_zh in rows:
        pet = (pet_id or "").strip()
        zh = (name_zh or "").strip()
        _base, suf = strip_skin(pet)
        kind = _suffix_variant_kind(suf)
        if not kind:
            continue
        if kind == "younv":
            # Adult 安克雷奇 must not keep ankeleiqi_younv; 小安克雷奇 may.
            if zh.startswith("小"):
                continue
        elif kind == "idol":
            if "(μ兵装)" in zh or "μ兵装" in zh:
                continue
        conn.execute(
            """
            UPDATE skins SET pet_model_id='', updated_at=datetime('now')
            WHERE id=?
            """,
            (sid,),
        )
        cleared += 1
    return cleared


def bind_pet_folder_models(
    conn: sqlite3.Connection,
    *,
    pet_models: Path,
    alias_map: dict[str, str] | None = None,
    cn_to_slug: dict[str, str] | None = None,
    only_set: set[str] | None = None,
) -> int:
    """Scan ``data/pet`` and set ``pet_model_id`` (BLHX: ``slug``, ``slug_2``, …).

    Does not write ``kanmusu_dir`` (舰娘仍由 skin 树扫描). Skips ``*_h`` (oath)
    and non-existent ``*_1`` folders.
    """
    if not pet_models.is_dir():
        return 0
    alias_map = alias_map or {}
    cn_to_slug = cn_to_slug or {}
    from common.unpack_complete import is_hx_slug
    from common.path_policy import is_special_pet_folder
    from roster.skin_probe import _has_spine_assets

    bound = 0
    folders = sorted(
        p.name for p in pet_models.iterdir() if p.is_dir() and not p.name.startswith(".")
    )
    for folder in folders:
        if is_hx_slug(folder) or is_special_pet_folder(folder):
            continue
        base, suffix = strip_skin(folder)
        if (suffix or "").lower() == "h":
            continue
        if suffix.isdigit() and int(suffix) == 1:
            continue
        if only_set and base not in only_set and folder not in only_set:
            continue
        if not _has_spine_assets(pet_models / folder):
            continue
        cid = _resolve_character_id_for_folder(
            conn,
            base,
            alias_map=alias_map,
            cn_to_slug=cn_to_slug,
            suffix=suffix,
        )
        if not cid:
            continue
        # younv/idol folders bind to the variant character's default skin
        bind_suffix = "" if _suffix_variant_kind(suffix) else suffix
        sid = resolve_bind_skin_id(conn, cid, bind_suffix)
        if not sid or is_oath_skin_id(sid):
            continue
        conn.execute(
            """
            UPDATE skins SET
              pet_model_id=?,
              updated_at=datetime('now')
            WHERE id=?
            """,
            (folder, sid),
        )
        bound += 1
    return bound


def bind_oath_h_pets(
    conn: sqlite3.Connection,
    *,
    pet_models: Path,
    alias_map: dict[str, str] | None = None,
    cn_to_slug: dict[str, str] | None = None,
    only_set: set[str] | None = None,
) -> int:
    """Scan ``pet`` for ``{slug}_h`` folders and bind to ``{cid}-oath`` (pet only).

    誓约皮肤没有舰娘资源；只读桌宠目录，写入 ``pet_model_id``，清空 ``kanmusu_dir``。
    """
    if not pet_models.is_dir():
        return 0
    alias_map = alias_map or {}
    cn_to_slug = cn_to_slug or {}
    from common.unpack_complete import is_hx_slug
    from common.path_policy import is_special_pet_folder
    from roster.skin_probe import _has_spine_assets

    bound = 0
    folders = sorted(
        p.name for p in pet_models.iterdir() if p.is_dir() and not p.name.startswith(".")
    )
    for folder in folders:
        if is_hx_slug(folder) or is_special_pet_folder(folder):
            continue
        base, suffix = strip_skin(folder)
        if (suffix or "").lower() != "h":
            continue
        if only_set and base not in only_set and folder not in only_set:
            continue
        pet_dir = pet_models / folder
        if not _has_spine_assets(pet_dir):
            continue

        cid = _resolve_character_id_for_folder(
            conn,
            base,
            alias_map=alias_map,
            cn_to_slug=cn_to_slug,
            suffix=suffix,
        )
        if not cid:
            continue

        sid = resolve_bind_skin_id(conn, cid, "h")
        if not sid or not is_oath_skin_id(sid):
            continue
        conn.execute(
            """
            UPDATE skins SET
              kanmusu_dir='',
              pet_model_id=?,
              updated_at=datetime('now')
            WHERE id=?
            """,
            (folder, sid),
        )
        bound += 1
    return bound


def bind_unpacked_models(
    conn: sqlite3.Connection,
    unpacked: Path,
    *,
    pet_models: Path | None = None,
    alias_map: dict[str, str] | None = None,
    cn_to_slug: dict[str, str] | None = None,
    only_set: set[str] | None = None,
    create_missing: bool = True,
) -> int:
    """Attach pet/kanmusu paths onto Wiki skins; optionally create skins for orphans."""
    alias_map = alias_map or {}
    cn_to_slug = cn_to_slug or {}
    pet_models = pet_models or (appdata_data_dir() / "pet-models")
    if not unpacked.is_dir():
        # Still bind pet folders when skin root is missing
        n = bind_pet_folder_models(
            conn,
            pet_models=pet_models,
            alias_map=alias_map,
            cn_to_slug=cn_to_slug,
            only_set=only_set,
        )
        n += bind_oath_h_pets(
            conn,
            pet_models=pet_models,
            alias_map=alias_map,
            cn_to_slug=cn_to_slug,
            only_set=only_set,
        )
        return n
    from common.unpack_complete import is_hx_slug
    from common.path_policy import is_special_pet_folder

    bound = 0
    folders = sorted(
        p.name for p in unpacked.iterdir() if p.is_dir() and not p.name.startswith(".")
    )
    for folder in folders:
        if is_hx_slug(folder):
            continue
        if is_special_pet_folder(folder):
            continue
        base, suffix = strip_skin(folder)
        # 誓约 ``*_h`` 只走 pet 扫描，不在 skin/unpacked 树里绑舰娘
        if (suffix or "").lower() == "h":
            continue
        # BLHX: 无 ``slug_1``；第一套换装是 ``slug_2``
        if suffix.isdigit() and int(suffix) == 1:
            continue
        if only_set and base not in only_set and folder not in only_set:
            continue
        cn = (alias_map.get(base) or alias_map.get(base.lower()) or "").strip() or None
        cid: str | None = None
        if cn:
            # guanghui_idol → 光辉(μ兵装); beierfasite_younv → 小贝法
            variant_cn = variant_character_name_for_suffix(cn, suffix)
            if variant_cn:
                row = conn.execute(
                    """
                    SELECT id FROM characters
                    WHERE name_zh=? OR wiki_title=?
                    ORDER BY CASE
                      WHEN source='wiki' THEN 0
                      WHEN id LIKE 'p%' THEN 1
                      ELSE 2
                    END
                    LIMIT 1
                    """,
                    (variant_cn, variant_cn),
                ).fetchone()
                if row:
                    cid = str(row[0])
                    cn = variant_cn
                elif _suffix_variant_kind(suffix):
                    # Do not bind idol/younv assets onto the adult ship
                    continue
            if not cid:
                # Prefer existing Wiki roster row by Chinese name over alias pinyin stub
                row = conn.execute(
                    """
                    SELECT id FROM characters
                    WHERE name_zh=? OR wiki_title=?
                    ORDER BY CASE
                      WHEN source='wiki' THEN 0
                      WHEN id LIKE 'p%' THEN 1
                      ELSE 2
                    END
                    LIMIT 1
                    """,
                    (cn, cn),
                ).fetchone()
                if row:
                    cid = str(row[0])
                else:
                    cid = cn_to_slug.get(cn) or _resolve_character_id(cn, cn_to_slug)
                    # Prefer an existing roster row by Chinese name over a fresh hash id
                    if cid and not conn.execute(
                        "SELECT 1 FROM characters WHERE id=?", (cid,)
                    ).fetchone():
                        row = conn.execute(
                            "SELECT id FROM characters WHERE name_zh=? OR wiki_title=? LIMIT 1",
                            (cn, cn),
                        ).fetchone()
                        if row:
                            cid = str(row[0])
        if not cid:
            redirected = alias_redirect_id(base, alias_map)
            if conn.execute(
                "SELECT 1 FROM characters WHERE id=?", (redirected,)
            ).fetchone():
                cid = redirected
            elif conn.execute(
                "SELECT 1 FROM characters WHERE id=?", (base,)
            ).fetchone():
                cid = base
            else:
                # Painting / pet folder slug equals Wiki display name (e.g. 「22」)
                row = conn.execute(
                    """
                    SELECT id FROM characters
                    WHERE name_zh=? OR wiki_title=?
                    ORDER BY CASE
                      WHEN source='wiki' THEN 0
                      WHEN id LIKE 'p%' THEN 1
                      ELSE 2
                    END
                    LIMIT 1
                    """,
                    (base, base),
                ).fetchone()
                if row:
                    cid = str(row[0])
                elif create_missing:
                    # Orphan painting folder (collab / unmapped) → stub character in 自用库
                    cid = redirected or base
                    upsert_character(
                        conn,
                        {
                            "id": cid,
                            "name_zh": cn or base,
                            "name_en": "",
                            "wiki_title": cn or base,
                            "persona_id": cid,
                            "source": "unpacked",
                        },
                    )
                else:
                    continue
        else:
            original_cid = cid
            redirected = alias_redirect_id(cid, alias_map)
            if conn.execute(
                "SELECT 1 FROM characters WHERE id=?", (redirected,)
            ).fetchone():
                cid = redirected
            elif conn.execute(
                "SELECT 1 FROM characters WHERE id=?", (original_cid,)
            ).fetchone():
                cid = original_cid
            elif cn:
                # Case-insensitive display-name match (wiki name_zh may be z13)
                row = conn.execute(
                    """
                    SELECT id FROM characters
                    WHERE lower(name_zh)=lower(?) OR lower(wiki_title)=lower(?)
                    ORDER BY CASE
                      WHEN source='wiki' THEN 0
                      WHEN id LIKE 'p%' THEN 1
                      ELSE 2
                    END
                    LIMIT 1
                    """,
                    (cn, cn),
                ).fetchone()
                if row:
                    cid = str(row[0])
                elif not create_missing:
                    continue
                else:
                    cid = redirected
            elif not create_missing:
                continue
            else:
                cid = redirected
        if only_set and cid not in only_set and base not in only_set:
            continue
        sid = resolve_bind_skin_id(conn, cid, suffix)
        if not sid:
            # Also match a prior bind that already points at this exact folder
            row = conn.execute(
                "SELECT id FROM skins WHERE character_id=? AND kanmusu_dir=? LIMIT 1",
                (cid, folder),
            ).fetchone()
            sid = str(row[0]) if row else None
        if not sid:
            if not create_missing:
                continue
            # Wiki characters already have fixed slots — do not invent L2D-N for
            # extra game folders (i404_3 when Wiki only lists default+skin1).
            if _character_has_wiki_skins(conn, cid):
                continue
            # Ensure character row exists for orphan folders
            if not conn.execute(
                "SELECT 1 FROM characters WHERE id=?", (cid,)
            ).fetchone():
                upsert_character(
                    conn,
                    {
                        "id": cid,
                        "name_zh": cn or base,
                        "name_en": "",
                        "wiki_title": cn or base,
                        "persona_id": cid,
                        "source": "unpacked",
                    },
                )
            sid = ensure_skin_for_unpacked(conn, cid, folder, suffix)
        else:
            # Fold leftover L2D-{suffix} into the Wiki/target skin when present
            reclaim_l2d_orphan_into_skin(
                conn, cid, sid, folder=folder, suffix=suffix
            )
        kanmusu_dir, pet_model_id = _bind_paths_for_folder(
            unpacked / folder, pet_models, folder
        )
        # 誓约：只绑桌宠，不绑舰娘
        if is_oath_skin_id(str(sid)):
            oath_pet = pet_model_id
            if not oath_pet and (pet_models / folder).is_dir():
                oath_pet = folder
            elif not oath_pet and (pet_models / f"skin-{folder}").is_dir():
                oath_pet = f"skin-{folder}"
            conn.execute(
                """
                UPDATE skins SET
                  kanmusu_dir='',
                  pet_model_id=CASE WHEN ?!='' THEN ? ELSE pet_model_id END,
                  updated_at=datetime('now')
                WHERE id=?
                """,
                (oath_pet, oath_pet, sid),
            )
        else:
            conn.execute(
                """
                UPDATE skins SET
                  kanmusu_dir=?,
                  pet_model_id=?,
                  updated_at=datetime('now')
                WHERE id=?
                """,
                (kanmusu_dir, pet_model_id, sid),
            )
        bound += 1
    # 桌宠：扫 pet（``slug`` / ``slug_2`` / …）；誓约 ``*_h`` 另处理
    bound += bind_pet_folder_models(
        conn,
        pet_models=pet_models,
        alias_map=alias_map,
        cn_to_slug=cn_to_slug,
        only_set=only_set,
    )
    bound += bind_oath_h_pets(
        conn,
        pet_models=pet_models,
        alias_map=alias_map,
        cn_to_slug=cn_to_slug,
        only_set=only_set,
    )
    # Drop younv/idol pets left on adult characters, then heal Wiki slot mismatches
    bound += repair_misbound_variant_pets(conn)
    repair_stats = repair_blhx_skin_folder_binds(conn)
    bound += int(repair_stats.get("misindexed_moved") or 0)
    bound += int(repair_stats.get("l2d_reclaimed") or 0)
    bound += int(repair_stats.get("l2d_pruned") or 0)
    return bound

def cmd_repair_l2d_binds(args: argparse.Namespace) -> int:
    """Heal BLHX folder↔Wiki skin mismatches (misindexed binds + L2D orphans)."""
    db = Path(args.db) if args.db else default_local_db()
    if not db.is_file():
        emit({"ok": False, "error": f"db not found: {db}"})
        return 1
    conn = connect(db)
    stats = repair_blhx_skin_folder_binds(conn)
    from common.path_policy import default_pet

    alias_map = enrich_alias_map_from_roster(
        conn,
        {
            **LIVE2D_ALIASES,
            **load_json(repo_root() / "data/wiki/live2d-aliases.json", {}),
        },
    )
    pet_root = default_pet()
    variant_cleared = repair_misbound_variant_pets(conn)
    pet_bound = bind_pet_folder_models(
        conn,
        pet_models=pet_root,
        alias_map=alias_map,
    )
    oath_bound = bind_oath_h_pets(
        conn,
        pet_models=pet_root,
        alias_map=alias_map,
    )
    conn.commit()
    left = conn.execute(
        "SELECT count(*) FROM skins WHERE id LIKE '%-L2D-%'"
    ).fetchone()[0]
    conn.close()
    emit(
        {
            "ok": True,
            **stats,
            "variant_pet_cleared": variant_cleared,
            "pet_folder_bound": pet_bound,
            "oath_h_bound": oath_bound,
            "l2d_skins_remaining": left,
            "db": str(db),
        }
    )
    return 0

