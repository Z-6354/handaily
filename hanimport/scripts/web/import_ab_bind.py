"""AbImport Facade: scan → unpack → bind (import_ab_bind job)."""
from __future__ import annotations

import os
import threading
import time
from pathlib import Path
from typing import Any

from common.job_store import (
    append_log,
    get_or_create_active_job,
    is_pause_requested,
    update_job,
)
from common.path_policy import assert_under_allowed
from roster.aliases import LIVE2D_ALIASES
from roster.bind_pipeline import bind_unpacked_models
from roster.schema import (
    apply_schema,
    connect,
    default_local_db,
    load_json,
    repo_root,
)

KIND = "import_ab_bind"


def _wait_if_paused(job_id: str) -> None:
    while is_pause_requested(job_id):
        time.sleep(0.25)


def _alias_map() -> dict[str, str]:
    return {
        **LIVE2D_ALIASES,
        **load_json(repo_root() / "data/wiki/live2d-aliases.json", {}),
    }


def _collect_inputs(body: dict[str, Any]) -> list[str]:
    from web.serve_web import collect_scan_inputs

    scan_inputs = collect_scan_inputs(body)
    if not scan_inputs:
        raise ValueError("input required")
    for p in scan_inputs:
        assert_under_allowed(p, label="input")
    return scan_inputs


def start_import(source: dict, *, opts: dict | None = None) -> dict:
    """Start or reuse an import_ab_bind job.

    source: {input: str} | {inputs: list[str]}
    opts: create_missing, continue_on_error, generate_config, output, jobs, ...
    """
    body = dict(source)
    if opts:
        body.update(opts)
    _collect_inputs(body)

    jid, created = get_or_create_active_job(KIND)
    if created:
        threading.Thread(
            target=run_import_ab_bind_job,
            args=(jid, body),
            daemon=True,
            name=f"{KIND}-{jid}",
        ).start()
    return {"ok": True, "job_id": jid, "reused": not created}


def get_status(job_id: str) -> dict[str, Any] | None:
    import job_store

    return job_store.get_job(job_id)


def run_import_ab_bind_job(job_id: str, body: dict[str, Any]) -> None:
    from concurrent.futures import ThreadPoolExecutor, as_completed

    from web.serve_web import (
        collect_scan_inputs,
        discover_bundles_many,
        partition_hx_bundles,
        resolve_output,
        run_unpack_one,
        unitypy_installed,
    )

    try:
        scan_inputs = _collect_inputs(body)
    except ValueError as exc:
        update_job(job_id, status="error", phase="", error=str(exc))
        append_log(job_id, f"error: {exc}")
        return

    dry_run = bool(body.get("dry_run"))
    continue_on_error = True if "continue_on_error" not in body else bool(
        body.get("continue_on_error")
    )
    generate_config = True if "generate_config" not in body else bool(
        body.get("generate_config")
    )
    create_missing = True if "create_missing" not in body else bool(
        body.get("create_missing")
    )

    update_job(job_id, status="running", phase="scan")
    append_log(job_id, f"scan inputs: {', '.join(scan_inputs)}")

    missing = [p for p in scan_inputs if not Path(p).exists()]
    if len(missing) == len(scan_inputs):
        update_job(job_id, status="error", phase="", error=f"路径不存在: {missing[0]}")
        append_log(job_id, f"error: path missing {missing[0]}")
        return

    bundles, warn = discover_bundles_many(scan_inputs)
    for w in warn:
        append_log(job_id, w)

    slug_filter = body.get("slugs")
    if isinstance(slug_filter, list) and slug_filter:
        allowed = {str(s).strip().lower() for s in slug_filter if str(s).strip()}
        bundles = [b for b in bundles if b["slug"].lower() in allowed]

    primary = Path(scan_inputs[0])
    try:
        output_root = resolve_output(
            primary, (body.get("output") or "").strip() or None
        )
    except ValueError as exc:
        update_job(job_id, status="error", phase="", error=str(exc))
        append_log(job_id, f"error: {exc}")
        return

    if not bundles:
        update_job(job_id, status="error", phase="", error="未找到 AssetBundle 文件")
        append_log(job_id, "error: no bundles found")
        return

    if not dry_run and not unitypy_installed():
        update_job(
            job_id,
            status="error",
            phase="",
            error="UnityPy 未安装。请运行 hanimport/scripts/setup-env.bat",
        )
        append_log(job_id, "error: UnityPy not installed")
        return

    bundles, hx_bundles = partition_hx_bundles(bundles)
    results: list[dict[str, Any]] = []
    ok_count = fail_count = skip_count = 0
    src_dir = primary if primary.is_dir() else primary.parent
    jobs = int(body.get("jobs") or 0)
    if jobs < 1:
        jobs = min(8, max(2, (os.cpu_count() or 4)))

    total = len(bundles) + len(hx_bundles)
    update_job(
        job_id,
        status="running",
        phase="unpack",
        current=0,
        total=total,
        current_item="",
        ok_count=0,
        fail_count=0,
        skip_count=0,
        results=[],
        error=None,
    )
    append_log(job_id, f"输出: {output_root}{' (dry-run)' if dry_run else ''}")
    append_log(
        job_id,
        f"共 {total} 个 bundle（hx 跳过 {len(hx_bundles)}）· 并发 {jobs}",
    )

    if not dry_run:
        output_root.mkdir(parents=True, exist_ok=True)
        from common.unpack_complete import purge_hx_output_dirs

        for name in purge_hx_output_dirs(output_root):
            append_log(job_id, f"清理(hx) {name}")

    for b in hx_bundles:
        slug = b["slug"]
        skip_count += 1
        results.append(
            {
                "slug": slug,
                "input": b["path"],
                "ok": True,
                "skipped": True,
                "skip_reason": "hx",
            }
        )
        append_log(job_id, f"跳过(hx) {slug}")

    if hx_bundles:
        update_job(
            job_id,
            current=skip_count,
            skip_count=skip_count,
            results=list(results),
        )

    def _unpack_one_item(b: dict[str, Any]) -> dict[str, Any]:
        slug = b["slug"]
        if dry_run:
            from common.unpack_complete import is_unpack_complete

            out_dir = output_root / slug
            skipped = is_unpack_complete(out_dir, slug)
            return {
                "slug": slug,
                "input": b["path"],
                "ok": True,
                "dry_run": True,
                "skipped": skipped,
            }
        data = run_unpack_one(Path(b["path"]), output_root, slug)
        return {"slug": slug, "input": b["path"], **data}

    if bundles:
        with ThreadPoolExecutor(max_workers=jobs) as pool:
            futures = {pool.submit(_unpack_one_item, b): b for b in bundles}
            abort_error: str | None = None
            abort_slug = ""
            for fut in as_completed(futures):
                if abort_error:
                    break
                b = futures[fut]
                slug = b["slug"]
                try:
                    item = fut.result()
                    results.append(item)
                    if item.get("skipped"):
                        skip_count += 1
                        append_log(job_id, f"跳过(已完成) {slug}")
                    elif item.get("ok", True) and not item.get("error"):
                        ok_count += 1
                        append_log(
                            job_id,
                            f"ok ({item.get('kind')}) {slug} -> {item.get('output_dir')}",
                        )
                    else:
                        fail_count += 1
                        err = str(item.get("error") or "unpack failed")
                        append_log(job_id, f"失败 {slug}: {err}")
                        if not continue_on_error:
                            abort_error = err
                            abort_slug = slug
                except Exception as exc:  # noqa: BLE001
                    fail_count += 1
                    append_log(job_id, f"失败 {slug}: {exc}")
                    results.append(
                        {"slug": slug, "input": b["path"], "ok": False, "error": str(exc)}
                    )
                    if not continue_on_error:
                        abort_error = str(exc)
                        abort_slug = slug
                done = ok_count + skip_count + fail_count
                update_job(
                    job_id,
                    current=done,
                    current_item=slug,
                    ok_count=ok_count,
                    fail_count=fail_count,
                    skip_count=skip_count,
                    results=list(results),
                )
            if abort_error:
                for pending in futures:
                    pending.cancel()
                update_job(
                    job_id,
                    status="error",
                    phase="",
                    error=abort_error,
                    current_item=abort_slug,
                    ok_count=ok_count,
                    fail_count=fail_count,
                    skip_count=skip_count,
                    results=list(results),
                    current=ok_count + skip_count + fail_count,
                )
                return

    append_log(
        job_id,
        f"解包阶段完成 ok={ok_count} skip={skip_count} fail={fail_count}",
    )

    if generate_config and not dry_run:
        import serve_web

        config_targets = [
            Path(r["output_dir"])
            for r in results
            if r.get("ok") and r.get("output_dir") and not r.get("skipped")
        ]
        update_job(
            job_id,
            phase="config",
            current=0,
            total=len(config_targets),
            current_item="",
        )
        append_log(job_id, f"生成配置：{len(config_targets)} 个")
        cfg_ok = cfg_fail = 0
        for i, folder in enumerate(config_targets, 1):
            _wait_if_paused(job_id)
            update_job(job_id, current=i, current_item=folder.name)
            append_log(job_id, f"配置 {folder.name} …")
            try:
                item = serve_web._generate_config_for_dir(  # noqa: SLF001
                    folder, src_dir=src_dir, force=False, dry_run=False
                )
                if not item.get("ok", True) and item.get("error"):
                    raise RuntimeError(item["error"])
                append_log(job_id, f"  ok {folder.name}")
                results.append({"phase": "config", **item})
                cfg_ok += 1
            except Exception as exc:  # noqa: BLE001
                cfg_fail += 1
                append_log(job_id, f"  失败: {exc}")
                results.append(
                    {
                        "phase": "config",
                        "slug": folder.name,
                        "ok": False,
                        "error": str(exc),
                    }
                )
                if not continue_on_error:
                    update_job(
                        job_id,
                        status="error",
                        phase="",
                        error=str(exc),
                        current_item=folder.name,
                        results=list(results),
                    )
                    return
        append_log(job_id, f"配置完成 ok={cfg_ok} fail={cfg_fail}")

    if dry_run:
        update_job(
            job_id,
            status="done",
            phase="done",
            current_item="",
            ok_count=ok_count,
            fail_count=fail_count,
            skip_count=skip_count,
            results=list(results),
        )
        append_log(job_id, "dry-run done (bind skipped)")
        return

    update_job(job_id, status="running", phase="bind", current_item="")
    append_log(job_id, "绑皮阶段…")

    try:
        from web.serve_web import default_live2d, default_model_unpacked

        db_path = default_local_db()
        conn = connect(db_path)
        apply_schema(conn)
        alias_map = _alias_map()
        pet_root = default_live2d()
        unpacked_root = default_model_unpacked()
        # Build 中文名→角色 id + 拼音文件夹→中文名，对齐自用库
        cn_to_slug: dict[str, str] = {}
        try:
            import sqlite3

            from common.path_policy import default_wiki_db
            from roster.aliases import enrich_alias_map_from_roster
            from roster.bind_pipeline import _build_cn_to_slug
            from roster.folder_rules import strip_skin

            wiki_path = default_wiki_db()
            wiki: sqlite3.Connection | None = None
            if wiki_path.is_file():
                wiki = sqlite3.connect(f"file:{wiki_path}?mode=ro", uri=True)
                wiki.row_factory = sqlite3.Row
            if wiki is not None:
                try:
                    cn_to_slug = _build_cn_to_slug(conn, wiki, alias_map)
                    for row in wiki.execute(
                        "SELECT folder, display_name, wiki_title FROM live2d_mappings"
                    ):
                        base, _ = strip_skin(row["folder"] or "")
                        cn = (
                            (row["display_name"] or row["wiki_title"] or "")
                            .strip()
                        )
                        if base and cn and base not in alias_map:
                            alias_map[base] = cn
                finally:
                    wiki.close()
            alias_map = enrich_alias_map_from_roster(conn, alias_map)
            for row in conn.execute(
                """
                SELECT id, name_zh, wiki_title, source FROM characters
                ORDER BY CASE source WHEN 'wiki' THEN 1 ELSE 0 END, id
                """
            ):
                cid = str(row[0] or "")
                for key in (
                    (row[1] or "").strip(),
                    (row[2] or "").strip(),
                ):
                    if key and cid:
                        cn_to_slug[key] = cid
            append_log(
                job_id,
                f"bind maps: alias={len(alias_map)} cn={len(cn_to_slug)}",
            )
        except Exception as exc:  # noqa: BLE001
            append_log(job_id, f"warn: cn_to_slug build failed: {exc}")

        bound = bind_unpacked_models(
            conn,
            unpacked_root,
            pet_models=pet_root,
            alias_map=alias_map,
            cn_to_slug=cn_to_slug,
            create_missing=create_missing,
        )
        from roster.merge import merge_roster_duplicates_by_name

        merged = merge_roster_duplicates_by_name(conn, alias_map)
        conn.commit()
        conn.close()
        append_log(
            job_id,
            f"绑皮完成 bound={bound} merged_dups={merged}",
        )
        update_job(
            job_id,
            status="done",
            phase="done",
            current_item="",
            ok_count=ok_count + bound,
            fail_count=fail_count,
            skip_count=skip_count,
            results=list(results)
            + [{"phase": "bind", "bound": bound, "merged_dups": merged}],
        )
        append_log(job_id, "done")
    except Exception as exc:  # noqa: BLE001
        update_job(job_id, status="error", error=str(exc), phase="bind")
        append_log(job_id, f"bind error: {exc}")
