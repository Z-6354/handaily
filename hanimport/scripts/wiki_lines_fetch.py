#!/usr/bin/env python3
"""Fetch BWIKI ship pages and fill lines_by_skin_json (skip if already present)."""
from __future__ import annotations

import json
import os
import sqlite3
import subprocess
import tempfile
import threading
import time
import urllib.error
import urllib.parse
import urllib.request
from concurrent.futures import ThreadPoolExecutor, as_completed
from pathlib import Path
from typing import Any, Callable

from roster_db import repo_root

_USER_AGENT = (
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 "
    "(KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36 HANDAILY-hanimport-wiki/1.0"
)
_WIKI_BASE = "https://wiki.biligame.com/blhx"
_WIKI_API = f"{_WIKI_BASE}/api.php"
_REQUEST_DELAY = float(os.environ.get("BLHX_WIKI_DELAY_MS", "350")) / 1000.0
_MAX_RETRIES = int(os.environ.get("BLHX_WIKI_MAX_RETRIES", "3"))
_throttle_lock = threading.Lock()
_last_request_at = 0.0
_wiki_db_write_lock = threading.Lock()


def fetch_concurrency() -> int:
    raw = os.environ.get("BLHX_WIKI_FETCH_CONCURRENCY", "2")
    try:
        n = int(raw)
    except ValueError:
        n = 2
    return max(1, min(4, n))


def wiki_request_headers(page_title: str = "") -> dict[str, str]:
    """BWIKI rejects bare UA with HTTP 567; Referer + Accept are required."""
    title = (page_title or "").strip() or "舰船图鉴"
    referer = f"{_WIKI_BASE}/{urllib.parse.quote(title)}"
    return {
        "User-Agent": _USER_AGENT,
        "Accept": "application/json,text/html,*/*",
        "Accept-Language": "zh-CN,zh;q=0.9",
        "Referer": referer,
    }


def _shared_throttle() -> None:
    """Global min interval across concurrent wiki HTTP calls."""
    global _last_request_at
    with _throttle_lock:
        now = time.monotonic()
        wait = _REQUEST_DELAY - (now - _last_request_at)
        if wait > 0:
            time.sleep(wait)
        _last_request_at = time.monotonic()



def default_wiki_db() -> Path:
    root = repo_root()
    for rel in (
        "mcp/blhx-wiki/data/blhx.sqlite",
        "data/wiki/blhx.sqlite",
    ):
        p = root / rel
        if p.is_file():
            return p
    return root / "mcp/blhx-wiki/data/blhx.sqlite"


def ensure_lines_by_skin_column(wiki_db: Path) -> None:
    conn = sqlite3.connect(str(wiki_db))
    try:
        cols = {r[1] for r in conn.execute("PRAGMA table_info(ships)")}
        if "lines_by_skin_json" not in cols:
            conn.execute(
                "ALTER TABLE ships ADD COLUMN lines_by_skin_json TEXT NOT NULL DEFAULT '[]'"
            )
        if "skins_json" not in cols:
            conn.execute(
                "ALTER TABLE ships ADD COLUMN skins_json TEXT NOT NULL DEFAULT '[]'"
            )
        conn.commit()
    finally:
        conn.close()


def _groups_nonempty(raw: str | None) -> bool:
    try:
        data = json.loads(raw or "[]")
    except json.JSONDecodeError:
        return False
    return isinstance(data, list) and len(data) > 0


def ship_has_lines_by_skin(
    wiki_db: Path, *, wiki_title: str = "", name_zh: str = ""
) -> bool:
    """Skip when both TabContainer skins and per-skin lines are present."""
    if not wiki_db.is_file():
        return False
    keys = []
    for k in (wiki_title, name_zh):
        s = (k or "").strip()
        if s and s not in keys:
            keys.append(s)
    if not keys:
        return False
    conn = sqlite3.connect(str(wiki_db))
    try:
        cols = {r[1] for r in conn.execute("PRAGMA table_info(ships)")}
        if "lines_by_skin_json" not in cols:
            return False
        has_skins_col = "skins_json" in cols
        for key in keys:
            row = conn.execute(
                f"""
                SELECT lines_by_skin_json
                {", skins_json" if has_skins_col else ""}
                FROM ships
                WHERE wiki_title = ? OR display_name = ?
                LIMIT 1
                """,
                (key, key),
            ).fetchone()
            if not row:
                continue
            if not _groups_nonempty(row[0]):
                continue
            if has_skins_col and not _groups_nonempty(row[1] if len(row) > 1 else None):
                continue
            return True
    finally:
        conn.close()
    return False


def list_missing_line_targets(
    roster_conn: sqlite3.Connection, wiki_db: Path
) -> list[dict[str, str]]:
    ensure_lines_by_skin_column(wiki_db)
    out: list[dict[str, str]] = []
    for row in roster_conn.execute(
        "SELECT id, name_zh, wiki_title FROM characters ORDER BY id"
    ):
        cid = str(row["id"])
        name_zh = str(row["name_zh"] or "")
        wiki_title = str(row["wiki_title"] or "")
        if ship_has_lines_by_skin(wiki_db, wiki_title=wiki_title, name_zh=name_zh):
            continue
        page = wiki_title.strip() or name_zh.strip()
        if not page:
            continue
        out.append({"id": cid, "name_zh": name_zh, "wiki_title": page})
    return out


def fetch_wiki_html(page_title: str, timeout: float = 45.0) -> str:
    qs = urllib.parse.urlencode(
        {
            "action": "parse",
            "page": page_title,
            "prop": "text",
            "format": "json",
        }
    )
    url = f"{_WIKI_API}?{qs}"
    headers = wiki_request_headers(page_title)
    last_err: Exception | None = None
    for attempt in range(max(1, _MAX_RETRIES)):
        try:
            _shared_throttle()
            req = urllib.request.Request(url, headers=headers)
            with urllib.request.urlopen(req, timeout=timeout) as resp:
                raw = resp.read().decode("utf-8", "replace")
            data = json.loads(raw)
            if data.get("error"):
                raise RuntimeError(data["error"].get("info") or str(data["error"]))
            html = (((data.get("parse") or {}).get("text") or {}).get("*")) or ""
            if not html.strip():
                raise RuntimeError(f"empty parse text for {page_title}")
            return html
        except Exception as exc:  # noqa: BLE001
            last_err = exc
            msg = str(exc)
            retryable = any(
                code in msg for code in ("567", "429", "500", "502", "503", "504")
            ) or isinstance(exc, (TimeoutError, urllib.error.URLError))
            if not retryable or attempt >= _MAX_RETRIES - 1:
                break
            time.sleep(0.8 * (attempt + 1))
    assert last_err is not None
    raise last_err


def _find_tsx() -> list[str]:
    root = repo_root() / "mcp" / "blhx-wiki"
    if os.name == "nt":
        cand = root / "node_modules" / ".bin" / "tsx.cmd"
        if cand.is_file():
            return [str(cand)]
    cand = root / "node_modules" / ".bin" / "tsx"
    if cand.is_file():
        return [str(cand)]
    return ["npx", "--yes", "tsx"]


def parse_lines_via_node(html: str) -> dict[str, Any]:
    script = (
        repo_root()
        / "mcp"
        / "blhx-wiki"
        / "scripts"
        / "parse-ship-lines-cli.ts"
    )
    if not script.is_file():
        raise FileNotFoundError(f"missing {script}")
    with tempfile.NamedTemporaryFile(
        "w", encoding="utf-8", suffix=".html", delete=False
    ) as tmp:
        tmp.write(html)
        tmp_path = tmp.name
    try:
        cmd = [*_find_tsx(), str(script), tmp_path]
        proc = subprocess.run(
            cmd,
            capture_output=True,
            text=True,
            encoding="utf-8",
            cwd=str(script.parent.parent),
            timeout=120,
            shell=(os.name == "nt" and cmd[0].endswith(".cmd")),
        )
        if proc.returncode != 0:
            err = (proc.stderr or proc.stdout or "").strip()[:500]
            raise RuntimeError(f"parse-ship-lines failed: {err}")
        return json.loads(proc.stdout)
    finally:
        try:
            Path(tmp_path).unlink(missing_ok=True)
        except OSError:
            pass


def save_ship_lines(
    wiki_db: Path,
    *,
    wiki_title: str,
    display_name: str,
    groups: list,
    lines: list,
    skins: list | None = None,
) -> None:
    ensure_lines_by_skin_column(wiki_db)
    with _wiki_db_write_lock:
        conn = sqlite3.connect(str(wiki_db), timeout=60)
        try:
            groups_json = json.dumps(groups, ensure_ascii=False)
            lines_json = json.dumps(lines, ensure_ascii=False)
            skins_json = json.dumps(skins or [], ensure_ascii=False)
            row = conn.execute(
                "SELECT wiki_title FROM ships WHERE wiki_title=? OR display_name=? LIMIT 1",
                (wiki_title, display_name),
            ).fetchone()
            now = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
            if row:
                conn.execute(
                    """
                    UPDATE ships SET lines_by_skin_json=?, lines_json=?, skins_json=?, fetched_at=?
                    WHERE wiki_title=?
                    """,
                    (groups_json, lines_json, skins_json, now, row[0]),
                )
            else:
                url = (
                    "https://wiki.biligame.com/blhx/"
                    + urllib.parse.quote(wiki_title)
                )
                conn.execute(
                    """
                    INSERT INTO ships(
                      wiki_title, wiki_url, display_name, aliases_json,
                      character_info_json, sections_json, lines_json, lines_by_skin_json,
                      skins_json, assets_json, persona_reference, html_hash, fetched_at
                    ) VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?)
                    """,
                    (
                        wiki_title,
                        url,
                        display_name or wiki_title,
                        "[]",
                        "[]",
                        "[]",
                        lines_json,
                        groups_json,
                        skins_json,
                        "[]",
                        "",
                        "",
                        now,
                    ),
                )
            conn.commit()
        finally:
            conn.close()


def fetch_one(ch: dict[str, str], *, wiki_db: Path | None = None) -> dict[str, Any]:
    """Mirror avatar_fetch.fetch_one status shape: ok|skipped|error."""
    wiki_db = wiki_db or default_wiki_db()
    page = (ch.get("wiki_title") or ch.get("name_zh") or "").strip()
    if not page:
        return {"status": "skipped", "reason": "no wiki title"}
    if ship_has_lines_by_skin(
        wiki_db, wiki_title=page, name_zh=ch.get("name_zh") or ""
    ):
        return {"status": "skipped", "reason": "already has skins+lines_by_skin"}
    try:
        html = fetch_wiki_html(page)
        parsed = parse_lines_via_node(html)
        groups = parsed.get("groups") or []
        lines = parsed.get("lines") or []
        skins = parsed.get("skins") or []
        if not skins and not groups:
            return {"status": "error", "error": "no skins/line groups parsed"}
        save_ship_lines(
            wiki_db,
            wiki_title=page,
            display_name=ch.get("name_zh") or page,
            groups=groups,
            lines=lines,
            skins=skins,
        )
        return {
            "status": "ok",
            "skins": len(skins),
            "groups": len(groups),
            "lines": len(lines),
            "id": ch.get("id"),
        }
    except Exception as exc:  # noqa: BLE001
        return {"status": "error", "error": str(exc), "id": ch.get("id")}


def fetch_many(
    targets: list[dict[str, str]],
    *,
    wiki_db: Path | None = None,
    concurrency: int | None = None,
    on_progress: Callable[[int, int, dict[str, Any], dict[str, str]], None]
    | None = None,
) -> dict[str, Any]:
    """Fetch missing wiki pages with bounded concurrency + shared throttle.

    Returns {ok, fail, skip, fetched_ids, fail_samples}.
    """
    wiki_db = wiki_db or default_wiki_db()
    workers = concurrency if concurrency is not None else fetch_concurrency()
    workers = max(1, min(4, int(workers)))
    total = len(targets)
    if total == 0:
        return {
            "ok": 0,
            "fail": 0,
            "skip": 0,
            "fetched_ids": [],
            "fail_samples": [],
            "concurrency": workers,
        }

    ok = fail = skip = 0
    fetched_ids: list[str] = []
    fail_samples: list[str] = []
    lock = threading.Lock()
    done = 0

    def _run(ch: dict[str, str]) -> tuple[dict[str, str], dict[str, Any]]:
        return ch, fetch_one(ch, wiki_db=wiki_db)

    with ThreadPoolExecutor(max_workers=workers) as pool:
        futs = [pool.submit(_run, ch) for ch in targets]
        for fut in as_completed(futs):
            ch, result = fut.result()
            st = result.get("status")
            with lock:
                done += 1
                if st == "ok":
                    ok += 1
                    cid = ch.get("id")
                    if cid:
                        fetched_ids.append(cid)
                elif st == "skipped":
                    skip += 1
                else:
                    fail += 1
                    if len(fail_samples) < 5:
                        fail_samples.append(
                            f"{ch.get('name_zh') or ch.get('id')}: "
                            f"{result.get('error') or st}"
                        )
                if on_progress:
                    on_progress(done, total, result, ch)

    return {
        "ok": ok,
        "fail": fail,
        "skip": skip,
        "fetched_ids": fetched_ids,
        "fail_samples": fail_samples,
        "concurrency": workers,
    }
