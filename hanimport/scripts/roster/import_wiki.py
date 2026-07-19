"""Roster Wiki import (sheared from db.py C1)."""

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

# --- import_wiki ---

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

def _upsert_skins_from_slots(
    conn: sqlite3.Connection,
    cid: str,
    slots: list[dict],
    *,
    purge_orphans: bool = False,
) -> set[str]:
    """按 Wiki TabContainer 槽位 upsert 皮肤，并删除旧错误项。返回 Wiki keep + 保留手工皮 id。"""
    keep = expected_skin_ids_from_slots(cid, slots)
    # re-walk slots for upsert metadata (keep already computed)
    for slot in slots:
        if not isinstance(slot, dict):
            continue
        key = str(slot.get("key") or "").strip()
        label = str(slot.get("label") or key).strip()
        kind = str(slot.get("kind") or "skin")
        if not key or is_hidden_wiki_skin(kind=kind, label=label):
            continue
        sort_order = int(slot.get("sort_order") or 0)
        if key == "default" or kind == "default":
            sid = skin_db_id(cid, "default")
            is_default = True
            name_zh = "默认皮肤"
            skin_index = 0
        elif key == "oath" or kind == "oath":
            sid = f"{cid}-oath"
            is_default = False
            name_zh = label or "誓约"
            if is_generic_wiki_skin_label(name_zh):
                name_zh = "誓约"
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
            "bind_policy": "pet_only" if (key == "oath" or kind == "oath") else "full",
        }
        if slot.get("image_url"):
            meta_obj["wiki_image_url"] = slot.get("image_url")
        # Preserve non-empty lines_import / extra meta on existing rows
        prev = conn.execute(
            "SELECT meta_json FROM skins WHERE id=?", (sid,)
        ).fetchone()
        if prev:
            try:
                old_meta = json.loads(prev[0] or "{}")
            except json.JSONDecodeError:
                old_meta = {}
            if isinstance(old_meta, dict):
                old_li = old_meta.get("lines_import")
                if isinstance(old_li, dict) and str(old_li.get("status") or "") not in (
                    "",
                    "empty",
                ):
                    meta_obj["lines_import"] = old_li
                for k, v in old_meta.items():
                    if k not in meta_obj:
                        meta_obj[k] = v
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
        # upsert_skin 的 kanmusu CASE 不会用空串覆盖；誓约强制不绑舰娘
        if key == "oath" or kind == "oath":
            conn.execute(
                "UPDATE skins SET kanmusu_dir='' WHERE id=?", (sid,)
            )
    # 仅在解析出至少一个权威皮肤时清理 Wiki 孤儿；默认保留 L2D/手工皮
    if keep:
        _delete_skins_not_in(conn, cid, keep, purge_orphans=purge_orphans)
    return local_skin_ids(conn, cid)

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
            sid = str(a["skin_id"])
            wiki_skin = str(a.get("wiki_skin") or "").strip()
            is_def = bool(sk.get("is_default")) or sid.endswith("-default")
            if is_def:
                name_zh = "默认皮肤"
            elif wiki_skin and not is_generic_wiki_skin_label(wiki_skin):
                name_zh = wiki_skin
            else:
                name_zh = sk.get("name_zh") or sid
            if is_oath_skin_id(sid) and is_generic_wiki_skin_label(name_zh):
                name_zh = "誓约"
            meta = merge_meta_json(
                sk.get("meta_json"),
                {
                    "status": a["status"],
                    "wiki_skin": a.get("wiki_skin"),
                    "matched_by": a.get("matched_by"),
                },
            )
            kanmusu = "" if is_oath_skin_id(sid) else (sk.get("kanmusu_dir") or "")
            upsert_skin(
                conn,
                {
                    **{k: sk.get(k) for k in (
                        "id", "character_id", "name_zh", "name_en", "skin_index",
                        "pet_model_id", "kanmusu_dir", "sort_order", "is_default",
                    )},
                    "id": sid,
                    "character_id": cid,
                    "name_zh": name_zh,
                    "kanmusu_dir": kanmusu,
                    "meta_json": meta,
                    "lines": a["lines"],
                },
                replace_lines=True,
            )
            if is_oath_skin_id(sid):
                conn.execute(
                    "UPDATE skins SET kanmusu_dir='' WHERE id=?", (sid,)
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
      - unpacked：旧行为，仅扫 data/skin 文件夹

    phases: characters | skins | lines | bind（默认全部）
    incremental: 皮肤已与 skins_json 对齐则跳过该角色
    """
    db = Path(db) if db else default_local_db()
    if wiki_db is None:
        from common.path_policy import default_wiki_db as _default_wiki_db

        wiki_db = _default_wiki_db()
    else:
        wiki_db = Path(wiki_db)
    unpacked = Path(unpacked) if unpacked else (repo_root() / "data/skin")
    en_map_path = Path(en_map) if en_map else (repo_root() / "data/wiki/ship-en-names.json")
    en_map_data = load_json(en_map_path, {})
    alias_map = {
        **LIVE2D_ALIASES,
        **load_json(repo_root() / "data/wiki/live2d-aliases.json", {}),
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
    wiki: sqlite3.Connection | None = None
    try:
        # 先合并同名别名双开（aijiang/aijier），再清 folder-like 假角色，再继续导入
        merge_roster_duplicates_by_name(conn, alias_map)
        purge_folder_like_characters(conn, alias_map)
        wiki = sqlite3.connect(str(wiki_db), timeout=60.0)
        wiki.row_factory = sqlite3.Row
        wiki.execute("PRAGMA busy_timeout=60000")
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
                            "name_zh": "默认皮肤",
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

        if do_skins:
            # 原皮统一显示名（不依赖 Wiki 重抓）
            conn.execute(
                """
                UPDATE skins SET name_zh='默认皮肤', updated_at=datetime('now')
                WHERE is_default=1
                  AND (name_zh IS NULL OR trim(name_zh)='' OR name_zh IN ('默认','通常','default'))
                """
            )
            conn.execute(
                """
                UPDATE skins SET kanmusu_dir='', updated_at=datetime('now')
                WHERE id LIKE '%-oath' AND IFNULL(kanmusu_dir,'') != ''
                """
            )

        conn.commit()
        char_count = conn.execute("SELECT count(*) FROM characters").fetchone()[0]
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

    finally:
        try:
            conn.close()
        except Exception as exc:  # noqa: BLE001
            logging.debug("roster conn close: %s", exc)
        if wiki is not None:
            try:
                wiki.close()
            except Exception as exc:  # noqa: BLE001
                logging.debug("wiki conn close: %s", exc)

def cmd_import_wiki(args: argparse.Namespace) -> int:
    result = run_import_wiki(
        db=Path(args.db) if args.db else None,
        wiki_db=Path(args.wiki_db) if args.wiki_db else None,
        unpacked=Path(args.unpacked),
        en_map=Path(args.en_map),
        only_ids=getattr(args, "ids", None) or None,
        scope=getattr(args, "scope", None) or "all",
    )
    emit(result)
    return 0 if result.get("ok") else 1

