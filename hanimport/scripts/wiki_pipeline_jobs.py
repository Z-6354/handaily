"""Orchestrate local roster Wiki pipeline: characters → avatars/skins → lines."""
from __future__ import annotations

import threading
import time
from pathlib import Path
from typing import Any

from avatar_fetch import (
    default_wiki_db as avatar_wiki_db,
    fetch_one,
    list_missing_character_ids,
)
from job_store import (
    append_log,
    create_job,
    find_active_job,
    is_pause_requested,
    update_job,
)
from roster_db import (
    apply_schema,
    connect,
    default_local_db,
    list_character_ids_needing_lines,
    purge_folder_like_skins,
    run_import_wiki,
)
from wiki_lines_fetch import (
    default_wiki_db as lines_wiki_db,
    fetch_one as fetch_lines_one,
    list_missing_line_targets,
)


def _wait_if_paused(job_id: str) -> None:
    while is_pause_requested(job_id):
        time.sleep(0.25)


def run_wiki_pipeline_job(job_id: str, body: dict[str, Any]) -> None:
    db_path = Path(body["db_path"]) if body.get("db_path") else default_local_db()
    wiki_db = (
        Path(body["wiki_db"])
        if body.get("wiki_db")
        else (lines_wiki_db() if lines_wiki_db().is_file() else avatar_wiki_db())
    )
    force = bool(body.get("force"))
    incremental = not force
    update_job(job_id, status="running", phase="characters", current=0, total=1)
    append_log(
        job_id,
        f"pipeline start db={db_path} wiki={wiki_db} incremental={incremental}",
    )

    try:
        # Phase 1 — characters
        _wait_if_paused(job_id)
        update_job(
            job_id,
            status="running",
            phase="characters",
            current=0,
            total=1,
            current_item="同步角色",
            ok_count=0,
            fail_count=0,
            skip_count=0,
        )
        r1 = run_import_wiki(
            db=db_path,
            wiki_db=wiki_db,
            phases={"characters"},
            incremental=incremental,
        )
        if not r1.get("ok"):
            raise RuntimeError(r1.get("error") or "characters phase failed")
        append_log(job_id, f"characters ok upserted={r1.get('upserted')}")
        update_job(
            job_id,
            current=1,
            total=1,
            ok_count=int(r1.get("upserted") or 0),
            current_item="角色完成",
        )

        # Phase 2 — purge dirty + skins + bind + avatars
        _wait_if_paused(job_id)
        update_job(
            job_id,
            status="running",
            phase="avatars_skins",
            current=0,
            total=1,
            current_item="清理脏皮 / 同步皮肤",
            ok_count=0,
            fail_count=0,
            skip_count=0,
        )
        conn = connect(db_path)
        apply_schema(conn)
        purged = purge_folder_like_skins(conn)
        conn.commit()
        conn.close()
        append_log(job_id, f"purge folder-like skins={purged}")

        r2 = run_import_wiki(
            db=db_path,
            wiki_db=wiki_db,
            phases={"skins", "bind"},
            incremental=incremental,
        )
        if not r2.get("ok"):
            raise RuntimeError(r2.get("error") or "skins phase failed")
        append_log(
            job_id,
            f"skins updated={r2.get('skins_updated')} skipped={r2.get('skins_skipped')} "
            f"bound={r2.get('bound_models')}",
        )
        update_job(
            job_id,
            skip_count=int(r2.get("skins_skipped") or 0),
            ok_count=int(r2.get("skins_updated") or 0),
        )

        conn = connect(db_path)
        apply_schema(conn)
        avatars = list_missing_character_ids(conn)
        conn.close()
        av_total = len(avatars)
        update_job(
            job_id,
            phase="avatars_skins",
            current=0,
            total=max(av_total, 1),
            current_item="头像" if av_total else "头像已齐",
            ok_count=0,
            fail_count=0,
            skip_count=0,
        )
        av_ok = av_fail = av_skip = 0
        for i, ch in enumerate(avatars, start=1):
            _wait_if_paused(job_id)
            update_job(
                job_id,
                status="running",
                phase="avatars_skins",
                current=i,
                total=av_total,
                current_item=ch.get("name_zh") or ch["id"],
            )
            result = fetch_one(ch, wiki_db=wiki_db)
            st = result.get("status")
            if st == "ok":
                av_ok += 1
            elif st == "skipped":
                av_skip += 1
            else:
                av_fail += 1
            update_job(
                job_id,
                ok_count=av_ok,
                fail_count=av_fail,
                skip_count=av_skip,
            )
            if i % 5 == 0:
                time.sleep(0.05)
        append_log(job_id, f"avatars ok={av_ok} skip={av_skip} fail={av_fail}")

        # Phase 3 — fetch missing wiki lines then selective lines import
        _wait_if_paused(job_id)
        conn = connect(db_path)
        apply_schema(conn)
        targets = list_missing_line_targets(conn, wiki_db)
        conn.close()
        fetch_total = len(targets)
        update_job(
            job_id,
            status="running",
            phase="lines",
            current=0,
            total=max(fetch_total, 1),
            current_item="抓取台词" if fetch_total else "Wiki 台词已齐",
            ok_count=0,
            fail_count=0,
            skip_count=0,
        )
        fetched_ids: list[str] = []
        ln_ok = ln_fail = ln_skip = 0
        for i, ch in enumerate(targets, start=1):
            _wait_if_paused(job_id)
            update_job(
                job_id,
                status="running",
                phase="lines",
                current=i,
                total=fetch_total,
                current_item=f"抓取 {ch.get('name_zh') or ch['id']}",
            )
            result = fetch_lines_one(ch, wiki_db=wiki_db)
            st = result.get("status")
            if st == "ok":
                ln_ok += 1
                fetched_ids.append(ch["id"])
            elif st == "skipped":
                ln_skip += 1
            else:
                ln_fail += 1
            update_job(
                job_id,
                ok_count=ln_ok,
                fail_count=ln_fail,
                skip_count=ln_skip,
            )
        append_log(job_id, f"wiki-lines fetch ok={ln_ok} skip={ln_skip} fail={ln_fail}")

        conn = connect(db_path)
        apply_schema(conn)
        need_lines = set(list_character_ids_needing_lines(conn))
        conn.close()
        if force:
            line_ids: list[str] | None = None  # all
        else:
            line_ids = sorted(set(fetched_ids) | need_lines)

        update_job(job_id, current_item="写入台词")
        if line_ids is not None and len(line_ids) == 0:
            append_log(job_id, "lines import skipped (nothing needed)")
            r3 = {
                "ok": True,
                "skins_lines_ok": 0,
                "skins_lines_empty": 0,
                "wiki_skins_unmatched": 0,
                "roster_skins_unmatched": 0,
            }
        else:
            r3 = run_import_wiki(
                db=db_path,
                wiki_db=wiki_db,
                phases={"lines"},
                only_ids=line_ids,
                incremental=incremental,
            )
            if not r3.get("ok"):
                raise RuntimeError(r3.get("error") or "lines phase failed")
        append_log(
            job_id,
            "lines import "
            f"ok={r3.get('skins_lines_ok')} empty={r3.get('skins_lines_empty')} "
            f"wiki_unmatched={r3.get('wiki_skins_unmatched')} "
            f"roster_unmatched={r3.get('roster_skins_unmatched')} "
            f"ids={len(line_ids) if line_ids is not None else 'all'}",
        )

        update_job(
            job_id,
            status="done",
            phase="done",
            current=1,
            total=1,
            current_item="",
            ok_count=int(r3.get("skins_lines_ok") or 0),
            fail_count=int(r3.get("roster_skins_unmatched") or 0)
            + int(r3.get("wiki_skins_unmatched") or 0),
            skip_count=int(r3.get("skins_lines_empty") or 0),
            results=[
                {
                    "skins_lines_ok": r3.get("skins_lines_ok"),
                    "skins_lines_empty": r3.get("skins_lines_empty"),
                    "wiki_skins_unmatched": r3.get("wiki_skins_unmatched"),
                    "roster_skins_unmatched": r3.get("roster_skins_unmatched"),
                    "skins_skipped": r2.get("skins_skipped"),
                    "skins_updated": r2.get("skins_updated"),
                    "purged_folder_skins": purged,
                }
            ],
        )
        append_log(job_id, "pipeline done")
    except Exception as exc:  # noqa: BLE001
        update_job(job_id, status="error", error=str(exc), phase="")
        append_log(job_id, f"error: {exc}")


def start_wiki_pipeline_job(body: dict[str, Any] | None = None) -> str:
    body = dict(body or {})
    force = bool(body.get("force"))
    if not force:
        active = find_active_job("roster-wiki-pipeline")
        if active:
            return str(active["id"])
    jid = create_job("roster-wiki-pipeline")
    threading.Thread(
        target=run_wiki_pipeline_job,
        args=(jid, body),
        daemon=True,
        name=f"roster-wiki-pipeline-{jid}",
    ).start()
    return jid
