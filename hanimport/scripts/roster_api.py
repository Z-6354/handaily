#!/usr/bin/env python3
"""Roster HTTP API handlers (pure functions for /api/roster/*)."""
from __future__ import annotations

import re
from pathlib import Path
from urllib.parse import unquote

from roster_db import (
    apply_schema,
    bundled_roster_dir,
    connect,
    default_local_db,
    fill_english_names,
    run_import_wiki,
    run_publish_bundled,
    run_sync_appdata,
    upsert_character,
    upsert_skin,
)
from avatar_fetch import avatar_public_url
from skin_probe import enrich_skin
import json

WRITE_METHODS = frozenset({"POST", "PUT", "DELETE", "PATCH"})
LOCAL_ONLY_OPS = frozenset(
    {
        "import-wiki",
        "wiki-pipeline",
        "sync-appdata",
        "publish-bundled",
        "fetch-avatars",
        "fetch-wiki-lines",
    }
)


def resolve_path(db: str) -> Path:
    if db == "local":
        return default_local_db()
    if db == "bundled":
        return bundled_roster_dir() / "handaily-roster.sqlite"
    raise ValueError(f"unknown db: {db}")


def require_bundled_confirm(db: str, body: dict) -> str | None:
    if db == "bundled" and not body.get("confirm_bundled"):
        return "写入自带库需要 confirm_bundled=true"
    return None


def require_write(db: str, body: dict) -> str | None:
    return require_bundled_confirm(db, body)


def _row_to_dict(row) -> dict:
    return {k: row[k] for k in row.keys()}


def _attach_lines_status(skin: dict, line_count: int = 0) -> dict:
    out = dict(skin)
    status = None
    wiki_skin = None
    try:
        meta = json.loads(skin.get("meta_json") or "{}")
        li = meta.get("lines_import") if isinstance(meta, dict) else None
        if isinstance(li, dict):
            status = li.get("status")
            wiki_skin = li.get("wiki_skin")
    except (TypeError, json.JSONDecodeError):
        pass
    if not status:
        status = "ready" if line_count > 0 else "empty"
    out["lines_status"] = status
    out["lines_wiki_skin"] = wiki_skin
    out["lines_count"] = line_count
    return out


def _enrich_full_skin(skin: dict, line_count: int = 0) -> dict:
    return _attach_lines_status(enrich_skin(skin), line_count)


def _db_key(query: dict, body: dict) -> str:
    raw = query.get("db") or body.get("db") or "local"
    return str(raw).strip() or "local"


def _open(db: str):
    path = resolve_path(db)
    conn = connect(path)
    apply_schema(conn)
    return path, conn


def _meta(db: str) -> tuple[int, dict]:
    path, conn = _open(db)
    try:
        counts = {
            "characters": conn.execute("SELECT count(*) FROM characters").fetchone()[0],
            "skins": conn.execute("SELECT count(*) FROM skins").fetchone()[0],
            "lines": conn.execute("SELECT count(*) FROM skin_lines").fetchone()[0],
        }
        meta_rows = {
            r["key"]: r["value"] for r in conn.execute("SELECT key, value FROM meta")
        }
        return 200, {
            "ok": True,
            "db": db,
            "path": str(path.resolve()),
            "counts": counts,
            "meta": meta_rows,
        }
    finally:
        conn.close()


def _list_characters(db: str, query: dict) -> tuple[int, dict]:
    q = (query.get("q") or "").strip()
    try:
        offset = max(0, int(query.get("offset") or 0))
    except (TypeError, ValueError):
        offset = 0
    try:
        limit = min(500, max(1, int(query.get("limit") or 100)))
    except (TypeError, ValueError):
        limit = 100

    _path, conn = _open(db)
    try:
        if q:
            like = f"%{q}%"
            rows = conn.execute(
                """
                SELECT * FROM characters
                WHERE id LIKE ? OR name_zh LIKE ? OR name_en LIKE ?
                ORDER BY id LIMIT ? OFFSET ?
                """,
                (like, like, like, limit, offset),
            ).fetchall()
            total = conn.execute(
                """
                SELECT count(*) FROM characters
                WHERE id LIKE ? OR name_zh LIKE ? OR name_en LIKE ?
                """,
                (like, like, like),
            ).fetchone()[0]
        else:
            rows = conn.execute(
                "SELECT * FROM characters ORDER BY id LIMIT ? OFFSET ?",
                (limit, offset),
            ).fetchall()
            total = conn.execute("SELECT count(*) FROM characters").fetchone()[0]
        characters = []
        for r in rows:
            d = _row_to_dict(r)
            d["avatar_url"] = avatar_public_url(str(d.get("id") or ""))
            characters.append(d)
        return 200, {
            "ok": True,
            "db": db,
            "characters": characters,
            "total": total,
            "offset": offset,
            "limit": limit,
        }
    finally:
        conn.close()


def _get_character(db: str, cid: str) -> tuple[int, dict]:
    _path, conn = _open(db)
    try:
        row = conn.execute("SELECT * FROM characters WHERE id=?", (cid,)).fetchone()
        if not row:
            return 404, {"ok": False, "error": f"character not found: {cid}"}
        counts = {
            r["skin_id"]: r["n"]
            for r in conn.execute(
                """
                SELECT skin_id, count(*) AS n FROM skin_lines
                WHERE skin_id IN (SELECT id FROM skins WHERE character_id=?)
                GROUP BY skin_id
                """,
                (cid,),
            )
        }
        skins = [
            _enrich_full_skin(_row_to_dict(s), counts.get(s["id"], 0))
            for s in conn.execute(
                "SELECT * FROM skins WHERE character_id=? ORDER BY sort_order, id",
                (cid,),
            )
        ]
        return 200, {
            "ok": True,
            "db": db,
            "character": _row_to_dict(row),
            "skins": skins,
        }
    finally:
        conn.close()


def _skin_matches_filter(skin: dict, filt: str) -> bool:
    if not filt:
        return True
    if filt == "missing":
        return skin.get("pet_status") == "missing" or skin.get("kanmusu_status") == "missing"
    if filt == "dual_ready":
        return (
            skin.get("pet_status") == "ready" and skin.get("kanmusu_status") == "ready"
        )
    if filt in ("unbound", "ready") and filt != "ready":
        return (
            skin.get("pet_status") == filt or skin.get("kanmusu_status") == filt
        )
    if filt == "ready":
        # ambiguous historically used for asset ready — keep dual meaning via pet/km
        return (
            skin.get("pet_status") == "ready" or skin.get("kanmusu_status") == "ready"
        )
    if filt == "lines_issue":
        return skin.get("lines_status") in ("empty", "unmatched", "stale_flat")
    if filt == "lines_ready":
        return skin.get("lines_status") == "ready"
    return True


def _list_skins(db: str, query: dict) -> tuple[int, dict]:
    q = (query.get("q") or "").strip()
    cid = (query.get("character_id") or "").strip()
    filt = (query.get("filter") or query.get("status") or "").strip()
    try:
        offset = max(0, int(query.get("offset") or 0))
    except (TypeError, ValueError):
        offset = 0
    try:
        limit = min(500, max(1, int(query.get("limit") or 50)))
    except (TypeError, ValueError):
        limit = 50

    _path, conn = _open(db)
    try:
        sql = """
            SELECT s.*, c.name_zh AS character_name_zh, c.name_en AS character_name_en
            FROM skins s
            JOIN characters c ON c.id = s.character_id
            WHERE 1=1
        """
        params: list = []
        if cid:
            sql += " AND s.character_id = ?"
            params.append(cid)
        if q:
            like = f"%{q}%"
            sql += """
                AND (
                  s.id LIKE ? OR s.name_zh LIKE ? OR s.name_en LIKE ?
                  OR s.pet_model_id LIKE ? OR s.kanmusu_dir LIKE ?
                  OR c.name_zh LIKE ? OR c.name_en LIKE ? OR c.id LIKE ?
                )
            """
            params.extend([like] * 8)
        sql += " ORDER BY s.character_id, s.sort_order, s.id"
        rows = conn.execute(sql, params).fetchall()
        ids = [r["id"] for r in rows]
        counts: dict[str, int] = {}
        if ids:
            placeholders = ",".join("?" * len(ids))
            for r in conn.execute(
                f"SELECT skin_id, count(*) AS n FROM skin_lines WHERE skin_id IN ({placeholders}) GROUP BY skin_id",
                ids,
            ):
                counts[r["skin_id"]] = r["n"]
        enriched = []
        for r in rows:
            d = _enrich_full_skin(_row_to_dict(r), counts.get(r["id"], 0))
            if _skin_matches_filter(d, filt):
                enriched.append(d)
        total = len(enriched)
        page = enriched[offset : offset + limit]
        return 200, {
            "ok": True,
            "db": db,
            "skins": page,
            "total": total,
            "offset": offset,
            "limit": limit,
        }
    finally:
        conn.close()


def _create_character(db: str, body: dict) -> tuple[int, dict]:
    cid = (body.get("id") or "").strip()
    if not cid:
        return 400, {"ok": False, "error": "id required"}
    name_zh = (body.get("name_zh") or body.get("name") or "").strip()
    if not name_zh:
        return 400, {"ok": False, "error": "name_zh required"}

    _path, conn = _open(db)
    try:
        existing = conn.execute("SELECT id FROM characters WHERE id=?", (cid,)).fetchone()
        if existing:
            return 409, {"ok": False, "error": f"character already exists: {cid}"}
        upsert_character(
            conn,
            {
                "id": cid,
                "name_zh": name_zh,
                "name_en": body.get("name_en") or "",
                "wiki_title": body.get("wiki_title") or "",
                "cv": body.get("cv") or "",
                "faction": body.get("faction") or "",
                "ship_type": body.get("ship_type") or "",
                "rarity": body.get("rarity") or "",
                "persona_id": body.get("persona_id") or cid,
                "source": body.get("source") or "manual",
                "description": body.get("description") or "",
                "meta_json": body.get("meta_json") or "{}",
            },
        )
        conn.commit()
        row = conn.execute("SELECT * FROM characters WHERE id=?", (cid,)).fetchone()
        return 200, {"ok": True, "character": _row_to_dict(row)}
    finally:
        conn.close()


def _update_character(db: str, cid: str, body: dict) -> tuple[int, dict]:
    _path, conn = _open(db)
    try:
        row = conn.execute("SELECT * FROM characters WHERE id=?", (cid,)).fetchone()
        if not row:
            return 404, {"ok": False, "error": f"character not found: {cid}"}
        cur = _row_to_dict(row)
        name_zh = body.get("name_zh")
        if name_zh is None:
            name_zh = cur["name_zh"]
        name_en = body["name_en"] if "name_en" in body else cur["name_en"]
        upsert_character(
            conn,
            {
                "id": cid,
                "name_zh": name_zh,
                "name_en": name_en,
                "wiki_title": body.get("wiki_title", cur["wiki_title"]),
                "cv": body.get("cv", cur["cv"]),
                "faction": body.get("faction", cur["faction"]),
                "ship_type": body.get("ship_type", cur["ship_type"]),
                "rarity": body.get("rarity", cur["rarity"]),
                "persona_id": body.get("persona_id", cur["persona_id"]),
                "source": body.get("source", cur["source"]),
                "description": body.get("description", cur["description"]),
                "meta_json": body.get("meta_json", cur["meta_json"]),
            },
        )
        conn.commit()
        updated = conn.execute("SELECT * FROM characters WHERE id=?", (cid,)).fetchone()
        return 200, {"ok": True, "character": _row_to_dict(updated)}
    finally:
        conn.close()


def _delete_character(db: str, cid: str) -> tuple[int, dict]:
    _path, conn = _open(db)
    try:
        cur = conn.execute("DELETE FROM characters WHERE id=?", (cid,))
        conn.commit()
        if cur.rowcount == 0:
            return 404, {"ok": False, "error": f"character not found: {cid}"}
        return 200, {"ok": True, "deleted": cid}
    finally:
        conn.close()


def _create_skin(db: str, body: dict) -> tuple[int, dict]:
    sid = (body.get("id") or "").strip()
    cid = (body.get("character_id") or "").strip()
    if not sid or not cid:
        return 400, {"ok": False, "error": "id and character_id required"}
    name_zh = (body.get("name_zh") or body.get("name") or "").strip()
    if not name_zh:
        return 400, {"ok": False, "error": "name_zh required"}

    _path, conn = _open(db)
    try:
        if not conn.execute("SELECT id FROM characters WHERE id=?", (cid,)).fetchone():
            return 404, {"ok": False, "error": f"character not found: {cid}"}
        if conn.execute("SELECT id FROM skins WHERE id=?", (sid,)).fetchone():
            return 409, {"ok": False, "error": f"skin already exists: {sid}"}
        upsert_skin(
            conn,
            {
                "id": sid,
                "character_id": cid,
                "name_zh": name_zh,
                "name_en": body.get("name_en") or "",
                "skin_index": body.get("skin_index"),
                "pet_model_id": body.get("pet_model_id") or "",
                "kanmusu_dir": body.get("kanmusu_dir") or "",
                "sort_order": body.get("sort_order") or 0,
                "is_default": bool(body.get("is_default")),
                "meta_json": body.get("meta_json") or "{}",
            },
            replace_lines=False,
        )
        conn.commit()
        row = conn.execute("SELECT * FROM skins WHERE id=?", (sid,)).fetchone()
        return 200, {"ok": True, "skin": _row_to_dict(row)}
    finally:
        conn.close()


def _update_skin(db: str, sid: str, body: dict) -> tuple[int, dict]:
    _path, conn = _open(db)
    try:
        row = conn.execute("SELECT * FROM skins WHERE id=?", (sid,)).fetchone()
        if not row:
            return 404, {"ok": False, "error": f"skin not found: {sid}"}
        cur = _row_to_dict(row)
        upsert_skin(
            conn,
            {
                "id": sid,
                "character_id": body.get("character_id", cur["character_id"]),
                "name_zh": body.get("name_zh", cur["name_zh"]),
                "name_en": body["name_en"] if "name_en" in body else cur["name_en"],
                "skin_index": body.get("skin_index", cur["skin_index"]),
                "pet_model_id": body.get("pet_model_id", cur["pet_model_id"]),
                "kanmusu_dir": body.get("kanmusu_dir", cur["kanmusu_dir"]),
                "sort_order": body.get("sort_order", cur["sort_order"]),
                "is_default": body.get("is_default", bool(cur["is_default"])),
                "meta_json": body.get("meta_json", cur["meta_json"]),
            },
            replace_lines=False,
        )
        conn.commit()
        updated = conn.execute("SELECT * FROM skins WHERE id=?", (sid,)).fetchone()
        return 200, {"ok": True, "skin": _row_to_dict(updated)}
    finally:
        conn.close()


def _delete_skin(db: str, sid: str) -> tuple[int, dict]:
    _path, conn = _open(db)
    try:
        cur = conn.execute("DELETE FROM skins WHERE id=?", (sid,))
        conn.commit()
        if cur.rowcount == 0:
            return 404, {"ok": False, "error": f"skin not found: {sid}"}
        return 200, {"ok": True, "deleted": sid}
    finally:
        conn.close()


def _list_lines(db: str, skin_id: str) -> tuple[int, dict]:
    _path, conn = _open(db)
    try:
        if not conn.execute("SELECT id FROM skins WHERE id=?", (skin_id,)).fetchone():
            return 404, {"ok": False, "error": f"skin not found: {skin_id}"}
        lines = [
            _row_to_dict(ln)
            for ln in conn.execute(
                "SELECT * FROM skin_lines WHERE skin_id=? ORDER BY sort_order, id",
                (skin_id,),
            )
        ]
        return 200, {"ok": True, "lines": lines}
    finally:
        conn.close()


def _create_line(db: str, skin_id: str, body: dict) -> tuple[int, dict]:
    text = (body.get("text") or "").strip()
    if not text:
        return 400, {"ok": False, "error": "text required"}
    _path, conn = _open(db)
    try:
        if not conn.execute("SELECT id FROM skins WHERE id=?", (skin_id,)).fetchone():
            return 404, {"ok": False, "error": f"skin not found: {skin_id}"}
        cur = conn.execute(
            """
            INSERT INTO skin_lines(
              skin_id, wiki_key, label, lang, text, animation, audio_url, audio_relpath, sort_order
            ) VALUES (?,?,?,?,?,?,?,?,?)
            """,
            (
                skin_id,
                body.get("wiki_key") or "",
                body.get("label") or "",
                body.get("lang") or "",
                text,
                body.get("animation") or "",
                body.get("audio_url") or "",
                body.get("audio_relpath") or "",
                int(body.get("sort_order") or 0),
            ),
        )
        conn.commit()
        line_id = cur.lastrowid
        row = conn.execute("SELECT * FROM skin_lines WHERE id=?", (line_id,)).fetchone()
        return 200, {"ok": True, "line": _row_to_dict(row)}
    finally:
        conn.close()


def _update_line(db: str, line_id: int, body: dict) -> tuple[int, dict]:
    _path, conn = _open(db)
    try:
        row = conn.execute("SELECT * FROM skin_lines WHERE id=?", (line_id,)).fetchone()
        if not row:
            return 404, {"ok": False, "error": f"line not found: {line_id}"}
        cur = _row_to_dict(row)
        conn.execute(
            """
            UPDATE skin_lines SET
              wiki_key=?, label=?, lang=?, text=?, animation=?,
              audio_url=?, audio_relpath=?, sort_order=?
            WHERE id=?
            """,
            (
                body.get("wiki_key", cur["wiki_key"]),
                body.get("label", cur["label"]),
                body.get("lang", cur["lang"]),
                body.get("text", cur["text"]),
                body.get("animation", cur["animation"]),
                body.get("audio_url", cur["audio_url"]),
                body.get("audio_relpath", cur["audio_relpath"]),
                int(body.get("sort_order", cur["sort_order"]) or 0),
                line_id,
            ),
        )
        conn.commit()
        updated = conn.execute("SELECT * FROM skin_lines WHERE id=?", (line_id,)).fetchone()
        return 200, {"ok": True, "line": _row_to_dict(updated)}
    finally:
        conn.close()


def _delete_line(db: str, line_id: int) -> tuple[int, dict]:
    _path, conn = _open(db)
    try:
        cur = conn.execute("DELETE FROM skin_lines WHERE id=?", (line_id,))
        conn.commit()
        if cur.rowcount == 0:
            return 404, {"ok": False, "error": f"line not found: {line_id}"}
        return 200, {"ok": True, "deleted": line_id}
    finally:
        conn.close()


def _ops_fill_english(db: str) -> tuple[int, dict]:
    _path, conn = _open(db)
    try:
        filled = fill_english_names(conn)
        return 200, {"ok": True, "filled": filled}
    finally:
        conn.close()


def _ops_local(
    op: str, body: dict
) -> tuple[int, dict]:
    if op == "import-wiki":
        only = body.get("ids") or body.get("character_id") or body.get("only_ids")
        result = run_import_wiki(
            db=Path(body["db_path"]) if body.get("db_path") else None,
            wiki_db=Path(body["wiki_db"]) if body.get("wiki_db") else None,
            unpacked=Path(body["unpacked"]) if body.get("unpacked") else None,
            en_map=Path(body["en_map"]) if body.get("en_map") else None,
            only_ids=only,
            scope=body.get("scope") or "all",
        )
    elif op == "sync-appdata":
        # body.replace 默认 True：导入到桌宠时只用自用库角色，清掉 AppData 里几百个历史 wiki 条目
        replace = True if "replace" not in body else bool(body.get("replace"))
        result = run_sync_appdata(
            db=Path(body["db_path"]) if body.get("db_path") else None,
            data_dir=Path(body["data_dir"]) if body.get("data_dir") else None,
            ids=body.get("ids") or "",
            force_lines=bool(body.get("force_lines")),
            replace=replace,
        )
    elif op == "publish-bundled":
        # publish always needs an explicit confirm (writes bundled preview)
        if not body.get("confirm_bundled"):
            return 403, {"ok": False, "error": "写入自带库需要 confirm_bundled=true"}
        result = run_publish_bundled(
            db=Path(body["db_path"]) if body.get("db_path") else None,
            ids=body.get("ids") or "",
        )
    elif op == "fetch-avatars":
        from avatar_jobs import start_fetch_avatars_job

        jid = start_fetch_avatars_job(body)
        return 200, {"ok": True, "job_id": jid}
    elif op == "fetch-wiki-lines":
        from wiki_lines_jobs import start_fetch_wiki_lines_job

        jid = start_fetch_wiki_lines_job(body)
        return 200, {"ok": True, "job_id": jid}
    elif op == "wiki-pipeline":
        from wiki_pipeline_jobs import start_wiki_pipeline_job

        jid = start_wiki_pipeline_job(body)
        return 200, {"ok": True, "job_id": jid}
    else:
        return 404, {"ok": False, "error": f"unknown op: {op}"}
    code = 200 if result.get("ok") else 400
    # import-wiki kept for CLI/debug; UI uses wiki-pipeline (no auto side jobs)
    return code, result


def handle(
    method: str, path: str, query: dict | None, body: dict | None
) -> tuple[int, dict]:
    method = (method or "GET").upper()
    query = dict(query or {})
    body = dict(body or {})
    path = unquote(path or "")
    db = _db_key(query, body)

    if db not in ("local", "bundled"):
        return 400, {"ok": False, "error": "db must be local|bundled"}

    # publish-bundled is local-db scoped but writes bundled — confirm checked inside
    is_write = method in WRITE_METHODS
    if is_write and db == "bundled":
        err = require_bundled_confirm(db, body)
        if err:
            return 403, {"ok": False, "error": err}

    m = re.fullmatch(r"/api/roster/meta", path)
    if m and method == "GET":
        return _meta(db)

    m = re.fullmatch(r"/api/roster/characters", path)
    if m:
        if method == "GET":
            return _list_characters(db, query)
        if method == "POST":
            return _create_character(db, body)

    m = re.fullmatch(r"/api/roster/characters/([^/]+)", path)
    if m:
        cid = m.group(1)
        if method == "GET":
            return _get_character(db, cid)
        if method == "PUT":
            return _update_character(db, cid, body)
        if method == "DELETE":
            return _delete_character(db, cid)

    m = re.fullmatch(r"/api/roster/skins", path)
    if m:
        if method == "GET":
            return _list_skins(db, query)
        if method == "POST":
            return _create_skin(db, body)

    m = re.fullmatch(r"/api/roster/skins/([^/]+)", path)
    if m:
        sid = m.group(1)
        if method == "PUT":
            return _update_skin(db, sid, body)
        if method == "DELETE":
            return _delete_skin(db, sid)

    m = re.fullmatch(r"/api/roster/skins/([^/]+)/lines", path)
    if m:
        sid = m.group(1)
        if method == "GET":
            return _list_lines(db, sid)
        if method == "POST":
            return _create_line(db, sid, body)

    m = re.fullmatch(r"/api/roster/lines/([^/]+)", path)
    if m:
        try:
            line_id = int(m.group(1))
        except ValueError:
            return 400, {"ok": False, "error": "invalid line id"}
        if method == "PUT":
            return _update_line(db, line_id, body)
        if method == "DELETE":
            return _delete_line(db, line_id)

    m = re.fullmatch(r"/api/roster/ops/([^/]+)", path)
    if m and method == "POST":
        op = m.group(1)
        if op == "fill-english":
            return _ops_fill_english(db)
        if op in LOCAL_ONLY_OPS:
            if db != "local":
                return 400, {
                    "ok": False,
                    "error": f"{op} 仅支持自用库 (db=local)",
                }
            return _ops_local(op, body)
        return 404, {"ok": False, "error": f"unknown op: {op}"}

    return 404, {"ok": False, "error": f"no route: {method} {path}"}
