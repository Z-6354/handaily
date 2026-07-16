#!/usr/bin/env python3
"""Handaily roster DB: local private SQLite + allowlisted bundled preview export.

Commands:
  init | import-wiki | import-bundled-seed | sync-appdata | publish-bundled | export-pack | verify
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shutil
import sqlite3
import sys
import zipfile
from pathlib import Path

from line_skin_match import apply_lines_by_skin, merge_meta_json

SKIN_SUFFIX = re.compile(
    r"_(?:\d+|h|g|hx|doa|painting|idol|younv|summer|school|winter|swimsuit|wedding|newyear|cn|jp|en|super)$",
    re.I,
)
LATIN_RE = re.compile(
    r"[A-Za-zÄÖÜäöüßÁÉÍÓÚáéíóúÀÈÌÒÙàèìòùÂÊÎÔÛâêîôûÃÑÕãñõÅåÆæØø]"
)
CJK_RE = re.compile(r"[\u3040-\u30ff\u3400-\u9fff\uf900-\ufaff]")

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

# 多拼音别名共用中文名时的权威角色 id（避免 aijiang / aijier 双开）
ALIAS_PRIMARY_BY_CN: dict[str, str] = {
    "埃吉尔": "aijiang",
}


def preferred_slug_for_cn(cn: str, alias_map: dict[str, str] | None = None) -> str | None:
    """同一中文名下多个拼音别名 → 唯一权威 id。"""
    cn = (cn or "").strip()
    if not cn:
        return None
    alias_map = alias_map or LIVE2D_ALIASES
    slugs = [s for s, c in alias_map.items() if (c or "").strip() == cn and s]
    if not slugs:
        return None
    primary = ALIAS_PRIMARY_BY_CN.get(cn)
    if primary and primary in slugs:
        return primary
    for s, c in alias_map.items():
        if (c or "").strip() == cn and s:
            return s
    return slugs[0]


def alias_redirect_id(cid: str, alias_map: dict[str, str] | None = None) -> str:
    """若 cid 是次要别名，改写成权威 id。"""
    cid = (cid or "").strip()
    alias_map = alias_map or LIVE2D_ALIASES
    cn = (alias_map.get(cid) or "").strip()
    if not cn:
        return cid
    pref = preferred_slug_for_cn(cn, alias_map)
    return pref or cid


HASH_PERSONA_ID = re.compile(r"^p[0-9a-f]{8}$", re.I)


def dedupe_characters_by_name(characters: list) -> list:
    """Collapse BWIKI-hash + pinyin duplicates (same zh name) into pinyin id."""
    from collections import defaultdict

    # 与 merge_duplicate_characters 共用序号合并，避免导入后 Spine/舰娘分两行
    try:
        from merge_duplicate_characters import coalesce_skins
    except ImportError:
        coalesce_skins = None  # type: ignore

    by_name: dict[str, list] = defaultdict(list)
    orphans = []
    for c in characters:
        if not isinstance(c, dict):
            continue
        name = (c.get("name") or "").strip()
        if not name:
            orphans.append(c)
            continue
        by_name[name].append(c)

    out: list = []
    for name, group in by_name.items():
        if len(group) == 1:
            c = dict(group[0])
            cid = str(c.get("id") or "")
            skins = list(c.get("skins") or [])
            if coalesce_skins and skins:
                c["skins"] = coalesce_skins(skins, cid)
            out.append(c)
            continue
        non_hash = [c for c in group if not HASH_PERSONA_ID.match(str(c.get("id") or ""))]
        canon = dict(non_hash[0] if non_hash else group[0])
        donors = [c for c in group if c.get("id") != canon.get("id")]
        skins_by_id = {
            s.get("id"): dict(s)
            for s in (canon.get("skins") or [])
            if isinstance(s, dict) and s.get("id")
        }
        for d in donors:
            for s in d.get("skins") or []:
                if not isinstance(s, dict) or not s.get("id"):
                    continue
                sid = s["id"]
                if sid not in skins_by_id:
                    skins_by_id[sid] = dict(s)
                    continue
                cur = skins_by_id[sid]
                if not (cur.get("model_id") or "").strip() and (s.get("model_id") or "").strip():
                    cur["model_id"] = s["model_id"]
                if not (cur.get("kanmusu_dir") or "").strip() and (s.get("kanmusu_dir") or "").strip():
                    cur["kanmusu_dir"] = s["kanmusu_dir"]
                if cur.get("skin_index") is None and s.get("skin_index") is not None:
                    cur["skin_index"] = s["skin_index"]
            if not (canon.get("english_name") or "").strip() and (d.get("english_name") or "").strip():
                canon["english_name"] = d["english_name"]
            if not (canon.get("wiki_title") or "").strip() and (d.get("wiki_title") or "").strip():
                canon["wiki_title"] = d["wiki_title"]
        skins = list(skins_by_id.values())
        cid = str(canon.get("id") or "")
        canon["skins"] = coalesce_skins(skins, cid) if coalesce_skins else skins
        canon["persona_id"] = canon.get("id")
        out.append(canon)
    out.extend(orphans)
    return out


def repo_root() -> Path:
    return Path(__file__).resolve().parents[2]


def roster_dir() -> Path:
    return repo_root() / "data" / "roster"


def schema_path() -> Path:
    return roster_dir() / "schema.sql"


def default_local_db() -> Path:
    override = os.environ.get("HANDAILY_ROSTER_DB", "").strip()
    if override:
        return Path(override)
    return roster_dir() / "handaily-roster.sqlite"


def appdata_data_dir() -> Path:
    override = os.environ.get("HANDAILY_DATA_DIR", "").strip()
    if override:
        return Path(override)
    appdata = os.environ.get("APPDATA") or ""
    return Path(appdata) / "xiaohan-daily" / "data"


def bundled_roster_dir() -> Path:
    return repo_root() / "hanpet" / "bundled" / "roster"


def allowlist_path() -> Path:
    return roster_dir() / "bundled-allowlist.json"


def emit(obj) -> None:
    sys.stdout.buffer.write((json.dumps(obj, ensure_ascii=False, indent=2) + "\n").encode("utf-8"))


def load_json(path: Path, default):
    if not path.is_file():
        return default
    return json.loads(path.read_text(encoding="utf-8-sig"))


def connect(db_path: Path) -> sqlite3.Connection:
    db_path.parent.mkdir(parents=True, exist_ok=True)
    conn = sqlite3.connect(str(db_path))
    conn.row_factory = sqlite3.Row
    conn.execute("PRAGMA foreign_keys = ON")
    return conn


def apply_schema(conn: sqlite3.Connection) -> None:
    sql = schema_path().read_text(encoding="utf-8")
    conn.executescript(sql)
    conn.execute(
        "INSERT INTO meta(key, value) VALUES(?, ?) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        ("schema_version", "1"),
    )
    conn.commit()


def cmd_init(args: argparse.Namespace) -> int:
    db = Path(args.db) if args.db else default_local_db()
    if db.is_file() and not args.force:
        emit({"ok": True, "db": str(db), "note": "already exists (use --force to recreate)"})
        return 0
    if db.is_file() and args.force:
        db.unlink()
    conn = connect(db)
    apply_schema(conn)
    conn.close()
    emit({"ok": True, "db": str(db), "action": "init"})
    return 0


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


def is_folder_like_character_id(cid: str) -> bool:
    """True when id looks like an unpack folder (base + skin suffix), not a character."""
    _base, suffix = strip_skin((cid or "").strip())
    return bool(suffix)


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


def skin_label(suffix: str) -> str:
    if not suffix:
        return "默认"
    if suffix.isdigit():
        return f"皮肤{suffix}"
    return f"变体_{suffix}"


def clean_cv(raw: str) -> str:
    """Wiki CV field is often polluted by scrape noise; keep short readable text only."""
    s = (raw or "").strip()
    if not s:
        return ""
    if any(tok in s for tok in (".jpg", ".png", ".webp", "decoding=", "<img", "Skillicon", "%E")):
        return ""
    # Prefer segment before "画师" if present
    if "画师" in s:
        s = s.split("画师", 1)[0].strip(" /|")
    if len(s) > 160:
        s = s[:160].rstrip() + "…"
    return s


def pick_english(aliases: list, fallback: str) -> str:
    best = ""
    for a in aliases or []:
        if not isinstance(a, str):
            continue
        s = a.strip()
        if not s or not LATIN_RE.search(s):
            continue
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
    labeled = [s for s in skins if "换装" in str(s.get("name"))]
    pool = labeled or skins
    if not pool:
        return fallback
    if skin_index is None or skin_index <= 0:
        return Path(str(pool[0]["name"])).stem.replace(".jpg", "")
    for s in pool:
        stem = Path(str(s["name"])).stem
        m = re.search(r"换装\s*(\d+)", stem)
        if m and int(m.group(1)) + 1 == skin_index:
            return stem
        if m and int(m.group(1)) == skin_index:
            return stem
    if skin_index == 2 and pool:
        for s in pool:
            stem = Path(str(s["name"])).stem
            if re.search(r"换装\s*$", stem) or stem.endswith("换装"):
                return stem
    return fallback


def _skin_row_dicts(conn: sqlite3.Connection, cid: str) -> list[dict]:
    rows = conn.execute(
        "SELECT * FROM skins WHERE character_id=? ORDER BY sort_order, id", (cid,)
    ).fetchall()
    return [{k: r[k] for k in r.keys()} for r in rows]


def _wiki_line_groups(row: sqlite3.Row, ship_cols: set[str]) -> list[dict]:
    if "lines_by_skin_json" not in ship_cols:
        return []
    try:
        raw = row["lines_by_skin_json"]
    except (IndexError, KeyError):
        return []
    try:
        data = json.loads(raw or "[]")
    except json.JSONDecodeError:
        return []
    return data if isinstance(data, list) else []


def _wiki_skin_slots(row: sqlite3.Row, ship_cols: set[str]) -> list[dict]:
    if "skins_json" not in ship_cols:
        return []
    try:
        raw = row["skins_json"]
    except (IndexError, KeyError):
        return []
    try:
        data = json.loads(raw or "[]")
    except json.JSONDecodeError:
        return []
    return data if isinstance(data, list) else []


def _delete_skins_not_in(
    conn: sqlite3.Connection, cid: str, keep_ids: set[str]
) -> int:
    """删除该角色不在 keep 集合内的皮肤行（台词经 FK CASCADE）。"""
    rows = conn.execute(
        "SELECT id FROM skins WHERE character_id = ?", (cid,)
    ).fetchall()
    deleted = 0
    for (sid,) in rows:
        if sid in keep_ids:
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
        if not key or kind == "retrofit" or "改造" in label:
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
    keep = expected_skin_ids_from_slots(cid, slots)
    if not keep:
        return False
    return local_skin_ids(conn, cid) == keep


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


def _upsert_skins_from_slots(
    conn: sqlite3.Connection, cid: str, slots: list[dict]
) -> set[str]:
    """按 Wiki TabContainer 槽位 upsert 皮肤，并删除旧错误项。返回保留的 skin id。"""
    keep = expected_skin_ids_from_slots(cid, slots)
    # re-walk slots for upsert metadata (keep already computed)
    for slot in slots:
        if not isinstance(slot, dict):
            continue
        key = str(slot.get("key") or "").strip()
        label = str(slot.get("label") or key).strip()
        kind = str(slot.get("kind") or "skin")
        if not key or kind == "retrofit" or "改造" in label:
            continue
        sort_order = int(slot.get("sort_order") or 0)
        if key == "default" or kind == "default":
            sid = skin_db_id(cid, "default")
            is_default = True
            name_zh = "默认" if label in ("通常", "默认", "") else label
            skin_index = 0
        elif key == "oath" or kind == "oath":
            sid = f"{cid}-oath"
            is_default = False
            name_zh = label or "誓约"
            skin_index = 100 + sort_order
        else:
            sid = f"{cid}-{key}"
            is_default = False
            name_zh = label or key
            skin_index = sort_order
        keep.add(sid)
        meta_obj: dict = {
            "slot_key": key,
            "lines_import": {
                "status": "empty",
                "wiki_skin": label,
                "matched_by": "slot",
            },
        }
        if slot.get("image_url"):
            meta_obj["wiki_image_url"] = slot.get("image_url")
        upsert_skin(
            conn,
            {
                "id": sid,
                "character_id": cid,
                "name_zh": name_zh,
                "name_en": "",
                "skin_index": skin_index,
                "pet_model_id": "",
                "kanmusu_dir": "",
                "sort_order": sort_order,
                "is_default": is_default,
                "meta_json": json.dumps(meta_obj, ensure_ascii=False),
                "lines": [],
            },
            replace_lines=False,
        )
    # 仅在解析出至少一个权威皮肤时整表替换，避免空 keep 误删全量
    if keep:
        _delete_skins_not_in(conn, cid, keep)
    return keep


def _apply_lines_import(
    conn: sqlite3.Connection,
    cid: str,
    groups: list[dict],
    flat_raw: list,
    stats: dict,
) -> None:
    """Write per-skin lines + meta; update stats counters / report lists."""
    skins = _skin_row_dicts(conn, cid)
    if not skins:
        return

    def _bump(key: str, n: int = 1) -> None:
        stats[key] = int(stats.get(key) or 0) + n

    report_cap = 40

    def _note(bucket: str, item: dict) -> None:
        lst = stats.setdefault(bucket, [])
        if len(lst) < report_cap:
            lst.append(item)

    if groups:
        report = apply_lines_by_skin(groups, skins, lines_rows_from_wiki)
        by_id = {s["id"]: s for s in skins}
        for a in report["assignments"]:
            sk = by_id.get(a["skin_id"]) or {"id": a["skin_id"], "character_id": cid}
            meta = merge_meta_json(
                sk.get("meta_json"),
                {
                    "status": a["status"],
                    "wiki_skin": a.get("wiki_skin"),
                    "matched_by": a.get("matched_by"),
                },
            )
            upsert_skin(
                conn,
                {
                    **{k: sk.get(k) for k in (
                        "id", "character_id", "name_zh", "name_en", "skin_index",
                        "pet_model_id", "kanmusu_dir", "sort_order", "is_default",
                    )},
                    "id": a["skin_id"],
                    "character_id": cid,
                    "name_zh": sk.get("name_zh") or a["skin_id"],
                    "meta_json": meta,
                    "lines": a["lines"],
                },
                replace_lines=True,
            )
            if a["status"] == "ready":
                _bump("skins_lines_ok")
            else:
                _bump("skins_lines_empty")
        for u in report["wiki_unmatched"]:
            _bump("wiki_skins_unmatched")
            _note(
                "lines_report",
                {"type": "wiki_unmatched", "character_id": cid, **u},
            )
        for sid in report["roster_unmatched_ids"]:
            sk = by_id.get(sid)
            if not sk:
                continue
            _bump("roster_skins_unmatched")
            meta = merge_meta_json(
                sk.get("meta_json"),
                {
                    "status": "unmatched",
                    "wiki_skin": None,
                    "matched_by": None,
                },
            )
            upsert_skin(
                conn,
                {
                    "id": sid,
                    "character_id": cid,
                    "name_zh": sk.get("name_zh") or sid,
                    "name_en": sk.get("name_en") or "",
                    "skin_index": sk.get("skin_index"),
                    "pet_model_id": sk.get("pet_model_id") or "",
                    "kanmusu_dir": sk.get("kanmusu_dir") or "",
                    "sort_order": sk.get("sort_order") or 0,
                    "is_default": sk.get("is_default"),
                    "meta_json": meta,
                    "lines": [],
                },
                replace_lines=False,
            )
            _note(
                "lines_report",
                {
                    "type": "roster_unmatched",
                    "character_id": cid,
                    "skin_id": sid,
                    "name_zh": sk.get("name_zh") or "",
                },
            )
        return

    # Flat-only / legacy wiki rows
    lines = lines_rows_from_wiki(flat_raw)
    for sk in skins:
        is_def = bool(sk.get("is_default")) or str(sk.get("id") or "").endswith(
            "-default"
        )
        if is_def and lines:
            meta = merge_meta_json(
                sk.get("meta_json"),
                {"status": "stale_flat", "wiki_skin": "default", "matched_by": "default"},
            )
            upsert_skin(
                conn,
                {
                    "id": sk["id"],
                    "character_id": cid,
                    "name_zh": sk.get("name_zh") or sk["id"],
                    "name_en": sk.get("name_en") or "",
                    "skin_index": sk.get("skin_index"),
                    "pet_model_id": sk.get("pet_model_id") or "",
                    "kanmusu_dir": sk.get("kanmusu_dir") or "",
                    "sort_order": sk.get("sort_order") or 0,
                    "is_default": True,
                    "meta_json": meta,
                    "lines": lines,
                },
                replace_lines=True,
            )
            _bump("skins_lines_ok")
        else:
            meta = merge_meta_json(
                sk.get("meta_json"),
                {
                    "status": "stale_flat" if is_def else "empty",
                    "wiki_skin": None,
                    "matched_by": None,
                },
            )
            upsert_skin(
                conn,
                {
                    "id": sk["id"],
                    "character_id": cid,
                    "name_zh": sk.get("name_zh") or sk["id"],
                    "name_en": sk.get("name_en") or "",
                    "skin_index": sk.get("skin_index"),
                    "pet_model_id": sk.get("pet_model_id") or "",
                    "kanmusu_dir": sk.get("kanmusu_dir") or "",
                    "sort_order": sk.get("sort_order") or 0,
                    "is_default": sk.get("is_default"),
                    "meta_json": meta,
                    "lines": [],
                },
                replace_lines=False,
            )
            if not is_def:
                _bump("skins_lines_empty")


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


def skin_db_id(character_id: str, skin_id: str) -> str:
    """SQLite skins.id is global PK — map JSON 'default' → '{char}-default'."""
    sid = (skin_id or "").strip() or "default"
    if sid == "default":
        return f"{character_id}-default"
    return sid


def skin_manifest_id(character_id: str, db_skin_id: str) -> str:
    """Export '{char}-default' back to 'default' for AppData / bundled JSON."""
    if db_skin_id == f"{character_id}-default":
        return "default"
    return db_skin_id


def normalize_name_en(name_en: str, id_: str) -> str:
    s = (name_en or "").strip()
    return s if s else id_


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


def _parse_id_filter(only_ids: list[str] | str | None) -> set[str] | None:
    if only_ids is None or only_ids == "":
        return None
    if isinstance(only_ids, str):
        parts = [x.strip() for x in only_ids.replace(";", ",").split(",") if x.strip()]
    else:
        parts = [str(x).strip() for x in only_ids if str(x).strip()]
    return set(parts) if parts else None


def _stable_wiki_id(cn: str) -> str:
    """无拼音 slug 时用稳定 hash id（与历史 AppData pXXXXXXXX 风格一致）。"""
    digest = hashlib.md5((cn or "").encode("utf-8")).hexdigest()[:8]
    return f"p{digest}"


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


def merge_roster_duplicates_by_name(
    conn: sqlite3.Connection, alias_map: dict[str, str] | None = None
) -> int:
    """合并同中文名多角色行（如 aijiang+aijier）。返回合并掉的 donor 数。"""
    from collections import defaultdict

    alias_map = alias_map or LIVE2D_ALIASES
    by_name: dict[str, list[str]] = defaultdict(list)
    for row in conn.execute("SELECT id, name_zh FROM characters"):
        name = (row["name_zh"] or "").strip()
        if name:
            by_name[name].append(str(row["id"]))
    merged = 0
    for name, ids in by_name.items():
        if len(ids) < 2:
            continue
        pref = preferred_slug_for_cn(name, alias_map)
        non_hash = [i for i in ids if not HASH_PERSONA_ID.match(i)]
        if pref and pref in ids:
            canon = pref
        elif non_hash:
            canon = max(
                non_hash,
                key=lambda i: conn.execute(
                    "SELECT count(*) FROM skins WHERE character_id=?", (i,)
                ).fetchone()[0],
            )
        else:
            canon = max(
                ids,
                key=lambda i: conn.execute(
                    "SELECT count(*) FROM skins WHERE character_id=?", (i,)
                ).fetchone()[0],
            )
        for donor in ids:
            if donor == canon:
                continue
            _merge_character_into(conn, donor, canon)
            _repoint_avatar_file(donor, canon)
            merged += 1
    return merged


def enrich_unpacked_character_names(
    conn: sqlite3.Connection, wiki_db: Path | None = None
) -> int:
    """Fill Chinese name/wiki_title for stubs where name_zh == id (pinyin folder).

    Uses catalog pinyin slug index. Returns number of characters updated.
    """
    from avatar_fetch import default_wiki_db, resolve_catalog_by_slug

    wiki_db = Path(wiki_db) if wiki_db else default_wiki_db()
    if not wiki_db.is_file():
        return 0
    updated = 0
    rows = conn.execute(
        "SELECT id, name_zh, wiki_title FROM characters ORDER BY id"
    ).fetchall()
    for row in rows:
        cid = str(row["id"])
        name_zh = (row["name_zh"] or "").strip()
        wiki_title = (row["wiki_title"] or "").strip()
        # Only touch obvious stubs (name still equals slug / empty CN)
        if name_zh and name_zh != cid and not name_zh.isascii():
            continue
        if wiki_title and wiki_title != cid and not wiki_title.isascii():
            continue
        base, _ = strip_skin(cid)
        hit = resolve_catalog_by_slug(wiki_db, cid) or resolve_catalog_by_slug(
            wiki_db, base
        )
        if not hit:
            continue
        display = (hit.get("display_name") or hit.get("wiki_title") or "").strip()
        title = (hit.get("wiki_title") or display).strip()
        if not display:
            continue
        conn.execute(
            """
            UPDATE characters SET
              name_zh=?,
              wiki_title=?,
              source=CASE WHEN source='unpacked' THEN 'wiki' ELSE source END,
              updated_at=datetime('now')
            WHERE id=?
            """,
            (display, title, cid),
        )
        updated += 1
    return updated


def _repoint_avatar_file(donor_cid: str, canon_cid: str) -> None:
    """If donor has an avatar file and canon does not, rename/copy to canon id."""
    if donor_cid == canon_cid:
        return
    try:
        from avatar_fetch import avatars_dir, resolve_avatar_file
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
    except OSError:
        try:
            import shutil

            shutil.copy2(src, dest)
        except OSError:
            pass


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
    for row in conn.execute("SELECT id, name_zh, wiki_title FROM characters"):
        cid = alias_redirect_id(str(row["id"] or ""), alias_map)
        for key in ((row["name_zh"] or "").strip(), (row["wiki_title"] or "").strip()):
            if key and cid and key not in rev:
                rev[key] = cid
    return rev


def _resolve_character_id(cn: str, cn_to_slug: dict[str, str]) -> str:
    cn = (cn or "").strip()
    if not cn:
        return _stable_wiki_id("unknown")
    if cn in cn_to_slug:
        return alias_redirect_id(cn_to_slug[cn])
    return _stable_wiki_id(cn)


def _parse_import_phases(phases: set[str] | list[str] | str | None) -> set[str]:
    """characters | skins | lines | bind；默认全开。"""
    all_phases = {"characters", "skins", "lines", "bind"}
    if phases is None or phases == "" or phases == "all":
        return set(all_phases)
    if isinstance(phases, str):
        parts = {p.strip() for p in phases.replace(";", ",").split(",") if p.strip()}
    else:
        parts = {str(p).strip() for p in phases if str(p).strip()}
    parts = parts & all_phases
    return parts if parts else set(all_phases)


def resolve_bind_skin_id(
    conn: sqlite3.Connection, cid: str, suffix: str
) -> str | None:
    """Map unpacked folder suffix to an existing roster skin id (no create)."""
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

    # Pure digit → skinN / skin_index. Compound (3_1 / 3_hx) intentionally
    # does not fall back to the parent skin — those get dedicated L2D rows.
    if suffix.isdigit():
        sid = f"{cid}-skin{suffix}"
        if conn.execute("SELECT 1 FROM skins WHERE id=?", (sid,)).fetchone():
            return sid
        row = conn.execute(
            "SELECT id FROM skins WHERE character_id=? AND skin_index=? LIMIT 1",
            (cid, int(suffix)),
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


def ensure_skin_for_unpacked(
    conn: sqlite3.Connection,
    cid: str,
    folder: str,
    suffix: str,
) -> str:
    """Create a roster skin row for an unpacked folder when Wiki has no match."""
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
    if not unpacked.is_dir():
        return 0
    alias_map = alias_map or {}
    cn_to_slug = cn_to_slug or {}
    pet_models = pet_models or (appdata_data_dir() / "pet-models")
    bound = 0
    folders = sorted(
        p.name for p in unpacked.iterdir() if p.is_dir() and not p.name.startswith(".")
    )
    for folder in folders:
        base, suffix = strip_skin(folder)
        if only_set and base not in only_set and folder not in only_set:
            continue
        cn = alias_map.get(base)
        cid: str | None = None
        if cn:
            cid = cn_to_slug.get(cn) or _resolve_character_id(cn, cn_to_slug)
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
                continue
        else:
            cid = alias_redirect_id(cid, alias_map)
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
        pet_model_id = ""
        if (pet_models / folder).is_dir():
            pet_model_id = folder
        elif (pet_models / f"skin-{folder}").is_dir():
            pet_model_id = f"skin-{folder}"
        conn.execute(
            """
            UPDATE skins SET
              kanmusu_dir=?,
              pet_model_id=CASE WHEN ?!='' THEN ? ELSE pet_model_id END,
              updated_at=datetime('now')
            WHERE id=?
            """,
            (folder, pet_model_id, pet_model_id, sid),
        )
        bound += 1
    return bound


def run_import_wiki(
    db: Path | None = None,
    wiki_db: Path | None = None,
    unpacked: Path | None = None,
    en_map: Path | None = None,
    only_ids: list[str] | str | None = None,
    scope: str = "all",
    phases: set[str] | list[str] | str | None = None,
    incremental: bool = False,
) -> dict:
    """从 BWIKI sqlite 写入自用库。

    scope:
      - all（默认）：导入 ships 表全部舰船（阵营/CV/稀有度/台词等）
      - unpacked：旧行为，仅扫 data/model/unpacked 文件夹

    phases: characters | skins | lines | bind（默认全部）
    incremental: 皮肤已与 skins_json 对齐则跳过该角色
    """
    db = Path(db) if db else default_local_db()
    wiki_db = Path(wiki_db) if wiki_db else (repo_root() / "mcp/blhx-wiki/data/blhx.sqlite")
    unpacked = Path(unpacked) if unpacked else (repo_root() / "data/model/unpacked")
    en_map_path = Path(en_map) if en_map else (repo_root() / "data/wiki/ship-en-names.json")
    en_map_data = load_json(en_map_path, {})
    alias_map = {
        **LIVE2D_ALIASES,
        **load_json(repo_root() / "mcp/blhx-wiki/data/live2d-aliases.json", {}),
    }
    only_set = _parse_id_filter(only_ids)
    phase_set = _parse_import_phases(phases)
    do_chars = "characters" in phase_set
    do_skins = "skins" in phase_set
    do_lines = "lines" in phase_set
    do_bind = "bind" in phase_set
    scope = (scope or "all").strip().lower()
    if scope not in ("all", "unpacked"):
        scope = "all"

    if not wiki_db.is_file():
        return {"ok": False, "error": f"wiki db missing: {wiki_db}"}

    conn = connect(db)
    apply_schema(conn)
    # 先合并同名别名双开（aijiang/aijier），再清 folder-like 假角色，再继续导入
    merge_roster_duplicates_by_name(conn, alias_map)
    purge_folder_like_characters(conn, alias_map)
    wiki = sqlite3.connect(str(wiki_db))
    wiki.row_factory = sqlite3.Row
    ship_cols = {r[1] for r in wiki.execute("PRAGMA table_info(ships)")}
    select_cols = [
        "wiki_title",
        "display_name",
        "aliases_json",
        "lines_json",
        "assets_json",
    ]
    for opt in (
        "cv",
        "faction",
        "ship_type",
        "rarity",
        "persona_reference",
        "lines_by_skin_json",
        "skins_json",
    ):
        if opt in ship_cols:
            select_cols.append(opt)
    select_sql = ", ".join(select_cols)

    cn_to_slug = _build_cn_to_slug(conn, wiki, alias_map)
    pet_models = appdata_data_dir() / "pet-models"
    upserted: list[dict] = []
    chars_seen: set[str] = set()
    lines_stats: dict = {
        "skins_lines_ok": 0,
        "skins_lines_empty": 0,
        "wiki_skins_unmatched": 0,
        "roster_skins_unmatched": 0,
        "lines_report": [],
        "skins_skipped": 0,
        "skins_updated": 0,
    }

    def import_ship_row(row: sqlite3.Row) -> str | None:
        display = (row["display_name"] or row["wiki_title"] or "").strip()
        wiki_title = (row["wiki_title"] or display).strip()
        if not display:
            return None
        cid = _resolve_character_id(display, cn_to_slug)
        if only_set:
            matched = False
            for oid in only_set:
                if oid == cid or oid == display or alias_map.get(oid) == display:
                    if oid in alias_map:
                        cid = alias_redirect_id(oid, alias_map)
                    matched = True
                    break
            if not matched:
                return None

        aliases = json.loads(row["aliases_json"] or "[]")
        english = pick_english(aliases, en_map_data.get(cid, "") or en_map_data.get(display, ""))
        keys = set(row.keys())
        cv = clean_cv(row["cv"] or "") if "cv" in keys else ""
        faction = (row["faction"] or "").strip() if "faction" in keys else ""
        ship_type = (row["ship_type"] or "").strip() if "ship_type" in keys else ""
        rarity = (row["rarity"] or "").strip() if "rarity" in keys else ""
        desc = ""
        if "persona_reference" in keys and row["persona_reference"]:
            desc = str(row["persona_reference"]).strip()
            if len(desc) > 4000:
                desc = desc[:4000].rstrip() + "…"

        if do_chars or do_skins or do_lines:
            # skins/lines 阶段也确保角色行存在
            upsert_character(
                conn,
                {
                    "id": cid,
                    "name_zh": display,
                    "name_en": english,
                    "wiki_title": wiki_title,
                    "cv": cv,
                    "faction": faction,
                    "ship_type": ship_type,
                    "rarity": rarity,
                    "persona_id": cid,
                    "source": "wiki",
                    "description": desc,
                },
            )
        else:
            return None

        if do_skins:
            slots = _wiki_skin_slots(row, ship_cols)
            if (
                incremental
                and slots
                and character_skins_in_sync(conn, cid, slots)
            ):
                lines_stats["skins_skipped"] = int(lines_stats["skins_skipped"]) + 1
            elif slots:
                # 权威皮肤清单：整角色替换（删除以往错误/残留皮肤），非纯增量
                _upsert_skins_from_slots(conn, cid, slots)
                lines_stats["skins_updated"] = int(lines_stats["skins_updated"]) + 1
            else:
                keep_legacy: set[str] = set()
                default_skin_id = skin_db_id(cid, "default")
                keep_legacy.add(default_skin_id)
                upsert_skin(
                    conn,
                    {
                        "id": default_skin_id,
                        "character_id": cid,
                        "name_zh": "默认",
                        "name_en": "",
                        "skin_index": 0,
                        "pet_model_id": "",
                        "kanmusu_dir": "",
                        "sort_order": 0,
                        "is_default": True,
                        "lines": [],
                    },
                    replace_lines=False,
                )
                # legacy: assets only when no TabContainer skins_json
                assets = json.loads(row["assets_json"] or "[]")
                if isinstance(assets, list):
                    for i, asset in enumerate(assets):
                        if not isinstance(asset, dict):
                            continue
                        title = pick_skin_title([asset], None, "") or ""
                        if not title or title in ("默认",) or "改造" in title:
                            continue
                        sid = f"{cid}-skin{i + 1}"
                        keep_legacy.add(sid)
                        upsert_skin(
                            conn,
                            {
                                "id": sid,
                                "character_id": cid,
                                "name_zh": title,
                                "name_en": "",
                                "skin_index": i + 1,
                                "pet_model_id": "",
                                "kanmusu_dir": "",
                                "sort_order": i + 1,
                                "is_default": False,
                                "lines": [],
                            },
                            replace_lines=False,
                        )
                if keep_legacy:
                    _delete_skins_not_in(conn, cid, keep_legacy)

        line_n = 0
        if do_lines:
            lines_raw = json.loads(row["lines_json"] or "[]")
            if not isinstance(lines_raw, list):
                lines_raw = []
            groups = _wiki_line_groups(row, ship_cols)
            _apply_lines_import(conn, cid, groups, lines_raw, lines_stats)
            line_n = conn.execute(
                "SELECT count(*) FROM skin_lines WHERE skin_id IN (SELECT id FROM skins WHERE character_id=?)",
                (cid,),
            ).fetchone()[0]

        cn_to_slug[display] = cid
        chars_seen.add(cid)
        upserted.append(
            {
                "character_id": cid,
                "name_zh": display,
                "name_en": english,
                "faction": faction,
                "cv": cv,
                "lines": line_n,
            }
        )
        return cid

    # —— 全量：wiki ships ——
    if scope == "all" and (do_chars or do_skins or do_lines):
        for row in wiki.execute(f"SELECT {select_sql} FROM ships ORDER BY display_name"):
            import_ship_row(row)

    # —— 解包目录：unpacked 模式下补角色；绑定永不按 folder 新建皮肤 ——
    if not unpacked.is_dir():
        folders: list[str] = []
        if scope == "unpacked":
            conn.close()
            wiki.close()
            return {"ok": False, "error": f"unpacked missing: {unpacked}"}
    else:
        folders = sorted(
            p.name for p in unpacked.iterdir() if p.is_dir() and not p.name.startswith(".")
        )

    if scope == "unpacked" and (do_chars or do_skins or do_lines):
        for folder in folders:
            base, _suffix = strip_skin(folder)
            if only_set and base not in only_set and folder not in only_set:
                continue
            cn = alias_map.get(base)
            row = None
            if cn:
                row = wiki.execute(
                    f"SELECT {select_sql} FROM ships WHERE display_name=? OR wiki_title=?",
                    (cn, cn),
                ).fetchone()
            if row is not None:
                import_ship_row(row)
            elif do_chars:
                display = cn or base
                cid = base
                if only_set and cid not in only_set:
                    continue
                upsert_character(
                    conn,
                    {
                        "id": cid,
                        "name_zh": display,
                        "name_en": en_map_data.get(cid, ""),
                        "wiki_title": display,
                        "persona_id": cid,
                        "source": "unpacked",
                    },
                )
                chars_seen.add(cid)

    if only_set and scope == "all" and (do_chars or do_skins or do_lines):
        # 保证点名 id 即使 wiki 无匹配也至少尝试一次（拼音别名）
        for oid in sorted(only_set):
            if oid in chars_seen:
                continue
            cn = alias_map.get(oid)
            if not cn:
                continue
            row = wiki.execute(
                f"SELECT {select_sql} FROM ships WHERE display_name=? OR wiki_title=?",
                (cn, cn),
            ).fetchone()
            if row:
                import_ship_row(row)

    bind_n = 0
    if do_bind:
        cn_to_slug = _build_cn_to_slug(conn, wiki, alias_map)
        bind_n = bind_unpacked_models(
            conn,
            unpacked,
            pet_models=pet_models,
            alias_map=alias_map,
            cn_to_slug=cn_to_slug,
            only_set=only_set,
        )

    if only_set is None and scope == "all" and do_chars:
        seed_chars = load_json(roster_dir() / "seed" / "characters.json", [])
        for c in seed_chars if isinstance(seed_chars, list) else []:
            if isinstance(c, dict) and c.get("id"):
                upsert_character(
                    conn, {**c, "name_zh": c.get("name_zh") or c.get("name") or c["id"]}
                )
    if only_set is None and scope == "all" and do_skins:
        seed_skins = load_json(roster_dir() / "seed" / "skins.json", [])
        for s in seed_skins if isinstance(seed_skins, list) else []:
            if isinstance(s, dict) and s.get("id") and s.get("character_id"):
                upsert_skin(
                    conn,
                    {
                        **s,
                        "name_zh": s.get("name_zh") or s.get("name") or s["id"],
                        "lines": s.get("lines") or [],
                    },
                    replace_lines=bool(s.get("lines")),
                )

    conn.commit()
    char_count = conn.execute("SELECT count(*) FROM characters").fetchone()[0]
    conn.close()
    wiki.close()
    return {
        "ok": True,
        "db": str(db),
        "scope": scope,
        "phases": sorted(phase_set),
        "bound_models": bind_n,
        "skins_skipped": lines_stats.get("skins_skipped", 0),
        "skins_updated": lines_stats.get("skins_updated", 0),
        "upserted": len(upserted),
        "character_total": char_count,
        "sample": upserted[:8],
        "only_ids": sorted(only_set) if only_set else None,
        "skins_lines_ok": lines_stats["skins_lines_ok"],
        "skins_lines_empty": lines_stats["skins_lines_empty"],
        "wiki_skins_unmatched": lines_stats["wiki_skins_unmatched"],
        "roster_skins_unmatched": lines_stats["roster_skins_unmatched"],
        "lines_report": lines_stats["lines_report"],
    }


def cmd_import_wiki(args: argparse.Namespace) -> int:
    result = run_import_wiki(
        db=Path(args.db) if args.db else None,
        wiki_db=Path(args.wiki_db),
        unpacked=Path(args.unpacked),
        en_map=Path(args.en_map),
        only_ids=getattr(args, "ids", None) or None,
        scope=getattr(args, "scope", None) or "all",
    )
    emit(result)
    return 0 if result.get("ok") else 1


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


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--db", type=Path, default=None)
    sub = ap.add_subparsers(dest="cmd", required=True)

    p_init = sub.add_parser("init")
    p_init.add_argument("--force", action="store_true")

    p_seed = sub.add_parser("import-bundled-seed")

    p_imp = sub.add_parser("import-wiki")
    p_imp.add_argument(
        "--wiki-db",
        type=Path,
        default=repo_root() / "mcp/blhx-wiki/data/blhx.sqlite",
    )
    p_imp.add_argument(
        "--unpacked",
        type=Path,
        default=repo_root() / "data/model/unpacked",
    )
    p_imp.add_argument(
        "--en-map",
        type=Path,
        default=repo_root() / "data/wiki/ship-en-names.json",
    )
    p_imp.add_argument(
        "--ids",
        type=str,
        default="",
        help="仅同步这些角色 id（逗号分隔），默认全部",
    )
    p_imp.add_argument(
        "--scope",
        choices=("all", "unpacked"),
        default="all",
        help="all=Wiki 全舰船；unpacked=仅已解包目录",
    )

    p_sync = sub.add_parser("sync-appdata")
    p_sync.add_argument("--data-dir", type=Path, default=None)
    p_sync.add_argument("--ids", type=str, default="")
    p_sync.add_argument("--force-lines", action="store_true")
    p_sync.add_argument(
        "--merge",
        action="store_true",
        help="合并进 AppData 现有角色（默认改为覆盖：仅保留本次同步的自用库角色）",
    )

    p_pub = sub.add_parser("publish-bundled")
    p_pub.add_argument("--ids", type=str, default="", help="override allowlist")

    p_pack = sub.add_parser("export-pack")
    p_pack.add_argument("--ids", type=str, required=True)
    p_pack.add_argument("-o", "--output", type=Path, required=True)

    sub.add_parser("verify")

    args = ap.parse_args()
    if args.cmd == "init":
        return cmd_init(args)
    if args.cmd == "import-bundled-seed":
        return cmd_import_bundled_seed(args)
    if args.cmd == "import-wiki":
        return cmd_import_wiki(args)
    if args.cmd == "sync-appdata":
        return cmd_sync_appdata(args)
    if args.cmd == "publish-bundled":
        return cmd_publish_bundled(args)
    if args.cmd == "export-pack":
        return cmd_export_pack(args)
    if args.cmd == "verify":
        return cmd_verify(args)
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
