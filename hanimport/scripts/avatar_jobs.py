"""Background fetch-avatars job runner."""
from __future__ import annotations

import sqlite3
import threading
import time
from pathlib import Path
from typing import Any

from avatar_fetch import (
    default_wiki_db,
    fetch_one,
    list_missing_character_ids,
    resolve_avatar_file,
)
from job_store import (
    append_log,
    create_job,
    is_pause_requested,
    update_job,
)
from roster_db import apply_schema, connect, default_local_db


def _wait_if_paused(job_id: str) -> None:
    while is_pause_requested(job_id):
        time.sleep(0.25)


def _collect_targets(
    conn: sqlite3.Connection,
    *,
    missing_only: bool,
    ids_raw: str,
) -> list[dict[str, str]]:
    if ids_raw.strip():
        wanted = {x.strip() for x in ids_raw.replace(";", ",").split(",") if x.strip()}
        rows = conn.execute(
            "SELECT id, name_zh, wiki_title FROM characters ORDER BY id"
        ).fetchall()
        out: list[dict[str, str]] = []
        for row in rows:
            cid = str(row["id"])
            if cid not in wanted:
                continue
            if missing_only and resolve_avatar_file(cid):
                continue
            out.append(
                {
                    "id": cid,
                    "name_zh": str(row["name_zh"] or ""),
                    "wiki_title": str(row["wiki_title"] or ""),
                }
            )
        return out
    if missing_only:
        return list_missing_character_ids(conn)
    rows = conn.execute(
        "SELECT id, name_zh, wiki_title FROM characters ORDER BY id"
    ).fetchall()
    return [
        {
            "id": str(r["id"]),
            "name_zh": str(r["name_zh"] or ""),
            "wiki_title": str(r["wiki_title"] or ""),
        }
        for r in rows
    ]


def run_fetch_avatars_job(job_id: str, body: dict[str, Any]) -> None:
    update_job(job_id, status="running", phase="fetch")
    db_path = Path(body["db_path"]) if body.get("db_path") else default_local_db()
    wiki_db = Path(body["wiki_db"]) if body.get("wiki_db") else default_wiki_db()
    missing_only = True if "missing_only" not in body else bool(body.get("missing_only"))
    ids_raw = str(body.get("ids") or "")

    try:
        conn = connect(db_path)
        apply_schema(conn)
        targets = _collect_targets(conn, missing_only=missing_only, ids_raw=ids_raw)
        conn.close()
    except Exception as exc:  # noqa: BLE001
        update_job(job_id, status="error", error=str(exc), phase="")
        append_log(job_id, f"error: {exc}")
        return

    total = len(targets)
    update_job(job_id, total=total, current=0, ok_count=0, fail_count=0, skip_count=0)
    append_log(job_id, f"queue size={total} missing_only={missing_only}")

    if total == 0:
        update_job(job_id, status="done", phase="", current_item="")
        append_log(job_id, "nothing to fetch")
        return

    ok_n = fail_n = skip_n = 0
    for i, ch in enumerate(targets, start=1):
        _wait_if_paused(job_id)
        # after resume, status may still be paused until we set running
        update_job(
            job_id,
            status="running",
            current=i,
            current_item=ch.get("name_zh") or ch["id"],
        )
        result = fetch_one(ch, wiki_db=wiki_db)
        st = result.get("status")
        if st == "ok":
            ok_n += 1
            append_log(job_id, f"ok {ch['id']}")
        elif st == "skipped":
            skip_n += 1
            append_log(job_id, f"skip {ch['id']} ({result.get('reason')})")
        else:
            fail_n += 1
            append_log(job_id, f"fail {ch['id']}: {result.get('error')}")
        update_job(
            job_id,
            ok_count=ok_n,
            fail_count=fail_n,
            skip_count=skip_n,
        )
        # gentle pacing for CDN
        time.sleep(0.05)

    update_job(
        job_id,
        status="done",
        phase="",
        current_item="",
        current=total,
        ok_count=ok_n,
        fail_count=fail_n,
        skip_count=skip_n,
    )
    append_log(job_id, f"done ok={ok_n} skip={skip_n} fail={fail_n}")


def start_fetch_avatars_job(body: dict[str, Any] | None = None) -> str:
    body = dict(body or {})
    jid = create_job("fetch-avatars")
    threading.Thread(
        target=run_fetch_avatars_job,
        args=(jid, body),
        daemon=True,
        name=f"fetch-avatars-{jid}",
    ).start()
    return jid
