"""Orchestrate local roster Wiki pipeline: characters → wiki fetch → skins → lines."""
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
    enrich_unpacked_character_names,
    list_character_ids_needing_lines,
    merge_roster_duplicates_by_name,
    purge_folder_like_characters,
    purge_folder_like_skins,
    run_import_wiki,
)
from wiki_lines_fetch import (
    default_wiki_db as lines_wiki_db,
    fetch_concurrency,
    fetch_many,
    list_missing_line_targets,
)
from wiki_pipeline_validate import validate_roster_wiki_state


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

        conn = connect(db_path)
        apply_schema(conn)
        purged = purge_folder_like_skins(conn)
        purged_chars = purge_folder_like_characters(conn)
        enriched = enrich_unpacked_character_names(conn, wiki_db)
        merged = merge_roster_duplicates_by_name(conn)
        conn.commit()
        conn.close()
        append_log(
            job_id,
            f"purge folder-like skins={purged} chars={purged_chars} "
            f"enrich_names={enriched} merge_name_dupes={merged}",
        )

        # Phase 2 — fetch TabContainer skins + lines_by_skin BEFORE roster skin replace
        _wait_if_paused(job_id)
        conn = connect(db_path)
        apply_schema(conn)
        targets = list_missing_line_targets(conn, wiki_db)
        conn.close()
        fetch_total = len(targets)
        workers = fetch_concurrency()
        resume_label = (
            f"续跑 · 待抓 {fetch_total}（并发 {workers}）"
            if fetch_total
            else "Wiki 皮肤/台词已齐"
        )
        append_log(job_id, resume_label)
        update_job(
            job_id,
            status="running",
            phase="lines",
            current=0,
            total=max(fetch_total, 1),
            current_item=resume_label if fetch_total else "Wiki 皮肤/台词已齐",
            ok_count=0,
            fail_count=0,
            skip_count=0,
        )

        prog_lock = threading.Lock()

        def _on_progress(
            done: int,
            total: int,
            result: dict[str, Any],
            ch: dict[str, str],
        ) -> None:
            _wait_if_paused(job_id)
            with prog_lock:
                # recount from job would race; use delta via update from fetch_many end
                update_job(
                    job_id,
                    status="running",
                    phase="lines",
                    current=done,
                    total=total,
                    current_item=f"抓取 {ch.get('name_zh') or ch.get('id')}",
                )

        fetch_stats = fetch_many(
            targets,
            wiki_db=wiki_db,
            concurrency=workers,
            on_progress=_on_progress if fetch_total else None,
        )
        ln_ok = int(fetch_stats["ok"])
        ln_fail = int(fetch_stats["fail"])
        ln_skip = int(fetch_stats["skip"])
        fetched_ids = list(fetch_stats["fetched_ids"])
        update_job(
            job_id,
            ok_count=ln_ok,
            fail_count=ln_fail,
            skip_count=ln_skip,
            current=fetch_total or 1,
            total=max(fetch_total, 1),
        )
        append_log(
            job_id,
            f"wiki fetch ok={ln_ok} skip={ln_skip} fail={ln_fail} "
            f"concurrency={fetch_stats.get('concurrency')}",
        )
        for sample in fetch_stats.get("fail_samples") or []:
            append_log(job_id, f"  fetch fail · {sample}")

        # Phase 3 — skins + bind (after skins_json filled)
        _wait_if_paused(job_id)
        update_job(
            job_id,
            status="running",
            phase="avatars_skins",
            current=0,
            total=1,
            current_item="同步权威皮肤",
            ok_count=0,
            fail_count=0,
            skip_count=0,
        )
        # Fresh skins_json must rewrite legacy *-skin15 ids; when we fetched any,
        # do a non-incremental skin pass for those chars via full skins phase.
        # force / any successful fetch → prefer full replace alignment.
        skin_incremental = incremental and not fetched_ids
        r2 = run_import_wiki(
            db=db_path,
            wiki_db=wiki_db,
            phases={"skins", "bind"},
            incremental=skin_incremental,
        )
        if not r2.get("ok"):
            raise RuntimeError(r2.get("error") or "skins phase failed")
        append_log(
            job_id,
            f"skins updated={r2.get('skins_updated')} skipped={r2.get('skins_skipped')} "
            f"bound={r2.get('bound_models')} incremental={skin_incremental}",
        )
        update_job(
            job_id,
            skip_count=int(r2.get("skins_skipped") or 0),
            ok_count=int(r2.get("skins_updated") or 0),
            current_item="皮肤完成",
        )

        # Avatars
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

        # Phase 4 — write lines into roster skins
        _wait_if_paused(job_id)
        conn = connect(db_path)
        apply_schema(conn)
        need_lines = set(list_character_ids_needing_lines(conn))
        conn.close()
        if force:
            line_ids: list[str] | None = None  # all
        else:
            line_ids = sorted(set(fetched_ids) | need_lines)

        update_job(
            job_id,
            status="running",
            phase="lines",
            current=0,
            total=1,
            current_item="写入台词",
            ok_count=0,
            fail_count=0,
            skip_count=0,
        )
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
            "台词写入完成："
            f"就绪 {r3.get('skins_lines_ok') or 0} · "
            f"Wiki无该皮台词 {r3.get('skins_lines_empty') or 0} · "
            f"Wiki套未对上 {r3.get('wiki_skins_unmatched') or 0} · "
            f"库皮未对上 {r3.get('roster_skins_unmatched') or 0}"
            + (
                f"（仅写 {len(line_ids)} 个角色）"
                if line_ids is not None
                else "（全量角色）"
            ),
        )

        validation = validate_roster_wiki_state(db_path, wiki_db)
        append_log(job_id, validation.get("summary") or "验收完成")
        for sample in (validation.get("samples") or [])[:5]:
            append_log(job_id, f"  样例 · {sample}")

        update_job(
            job_id,
            status="done",
            phase="done",
            current=1,
            total=1,
            current_item="完成",
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
                    "wiki_fetch_ok": ln_ok,
                    "wiki_fetch_fail": ln_fail,
                    "wiki_fetch_concurrency": fetch_stats.get("concurrency"),
                    "purged_folder_skins": purged,
                    "purged_folder_chars": purged_chars,
                    "validation": validation,
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
