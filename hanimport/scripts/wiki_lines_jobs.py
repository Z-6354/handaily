"""Background fetch-wiki-lines job (mirror avatar_jobs)."""
from __future__ import annotations

import sqlite3
import threading
import time
from pathlib import Path
from typing import Any

from job_store import (
    append_log,
    create_job,
    is_pause_requested,
    update_job,
)
from roster_db import apply_schema, connect, default_local_db
from wiki_lines_fetch import (
    default_wiki_db,
    fetch_one,
    list_missing_line_targets,
    ship_has_lines_by_skin,
)


def _wait_if_paused(job_id: str) -> None:
    while is_pause_requested(job_id):
        time.sleep(0.25)


def _collect_targets(
    conn: sqlite3.Connection,
    wiki_db: Path,
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
            name_zh = str(row["name_zh"] or "")
            wiki_title = str(row["wiki_title"] or "")
            page = wiki_title.strip() or name_zh.strip()
            if not page:
                continue
            if missing_only and ship_has_lines_by_skin(
                wiki_db, wiki_title=wiki_title, name_zh=name_zh
            ):
                continue
            out.append({"id": cid, "name_zh": name_zh, "wiki_title": page})
        return out
    if missing_only:
        return list_missing_line_targets(conn, wiki_db)
    rows = conn.execute(
        "SELECT id, name_zh, wiki_title FROM characters ORDER BY id"
    ).fetchall()
    out = []
    for r in rows:
        page = (str(r["wiki_title"] or "").strip() or str(r["name_zh"] or "").strip())
        if not page:
            continue
        out.append(
            {
                "id": str(r["id"]),
                "name_zh": str(r["name_zh"] or ""),
                "wiki_title": page,
            }
        )
    return out


def run_fetch_wiki_lines_job(job_id: str, body: dict[str, Any]) -> None:
    update_job(job_id, status="running", phase="fetch")
    db_path = Path(body["db_path"]) if body.get("db_path") else default_local_db()
    wiki_db = Path(body["wiki_db"]) if body.get("wiki_db") else default_wiki_db()
    missing_only = (
        True if "missing_only" not in body else bool(body.get("missing_only"))
    )
    ids_raw = str(body.get("ids") or "")

    try:
        conn = connect(db_path)
        apply_schema(conn)
        targets = _collect_targets(
            conn, wiki_db, missing_only=missing_only, ids_raw=ids_raw
        )
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
            append_log(
                job_id,
                f"ok {ch['id']} groups={result.get('groups')} lines={result.get('lines')}",
            )
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


def start_fetch_wiki_lines_job(body: dict[str, Any] | None = None) -> str:
    body = dict(body or {})
    jid = create_job("fetch-wiki-lines")
    threading.Thread(
        target=run_fetch_wiki_lines_job,
        args=(jid, body),
        daemon=True,
        name=f"fetch-wiki-lines-{jid}",
    ).start()
    return jid
