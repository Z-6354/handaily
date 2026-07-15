"""Local roster avatar resolve + download from BLHX wiki catalog."""
from __future__ import annotations

import re
import sqlite3
import urllib.error
import urllib.request
from pathlib import Path
from typing import Any
from urllib.parse import urlparse

from roster_db import repo_root, roster_dir

_SAFE_ID = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]{0,120}$")
_USER_AGENT = "hanimport-avatar/0.1 (+local; handaily)"


def avatars_dir() -> Path:
    d = roster_dir() / "avatars"
    d.mkdir(parents=True, exist_ok=True)
    return d


def is_safe_character_id(cid: str) -> bool:
    return bool(cid and _SAFE_ID.match(cid))


def resolve_avatar_file(cid: str) -> Path | None:
    if not is_safe_character_id(cid):
        return None
    base = avatars_dir()
    for ext in ("webp", "jpg", "jpeg", "png"):
        p = base / f"{cid}.{ext}"
        if p.is_file() and p.stat().st_size > 0:
            return p
    return None


def avatar_public_url(cid: str) -> str | None:
    path = resolve_avatar_file(cid)
    if not path:
        return None
    mtime = int(path.stat().st_mtime)
    return f"/avatars/{cid}?t={mtime}"


def default_wiki_db() -> Path:
    return repo_root() / "mcp" / "blhx-wiki" / "data" / "blhx.sqlite"


def lookup_avatar_url(
    wiki_db: Path,
    *,
    wiki_title: str = "",
    name_zh: str = "",
) -> str | None:
    if not wiki_db.is_file():
        return None
    keys: list[str] = []
    for k in (wiki_title, name_zh):
        s = (k or "").strip()
        if s and s not in keys:
            keys.append(s)
    if not keys:
        return None
    conn = sqlite3.connect(str(wiki_db))
    try:
        for key in keys:
            row = conn.execute(
                """
                SELECT avatar_url FROM catalog
                WHERE (wiki_title = ? OR display_name = ?)
                  AND avatar_url IS NOT NULL AND length(avatar_url) > 0
                LIMIT 1
                """,
                (key, key),
            ).fetchone()
            if row and row[0]:
                return str(row[0]).strip()
    finally:
        conn.close()
    return None


def _ext_from_url(url: str, content_type: str | None) -> str:
    path = urlparse(url).path.lower()
    for ext in (".webp", ".png", ".jpeg", ".jpg"):
        if path.endswith(ext):
            return ext.lstrip(".")
    ct = (content_type or "").lower()
    if "webp" in ct:
        return "webp"
    if "png" in ct:
        return "png"
    if "jpeg" in ct or "jpg" in ct:
        return "jpg"
    return "jpg"


def download_avatar(url: str, character_id: str, timeout: float = 20.0) -> Path:
    if not is_safe_character_id(character_id):
        raise ValueError(f"unsafe character id: {character_id}")
    if not url.startswith("https://"):
        raise ValueError("avatar url must be https")
    host = urlparse(url).hostname or ""
    if "biligame.com" not in host and "hdslb.com" not in host:
        raise ValueError(f"avatar host not allowed: {host}")

    req = urllib.request.Request(url, headers={"User-Agent": _USER_AGENT})
    with urllib.request.urlopen(req, timeout=timeout) as resp:
        data = resp.read()
        ctype = resp.headers.get("Content-Type")
        final_url = resp.geturl() or url
    if not data:
        raise ValueError("empty avatar body")
    ext = _ext_from_url(final_url, ctype)
    dest = avatars_dir() / f"{character_id}.{ext}"
    # remove other ext variants
    for old in avatars_dir().glob(f"{character_id}.*"):
        if old != dest and old.is_file():
            try:
                old.unlink()
            except OSError:
                pass
    dest.write_bytes(data)
    return dest


def list_missing_character_ids(conn: sqlite3.Connection) -> list[dict[str, str]]:
    rows = conn.execute(
        "SELECT id, name_zh, wiki_title FROM characters ORDER BY id"
    ).fetchall()
    missing: list[dict[str, str]] = []
    for row in rows:
        cid = row["id"] if isinstance(row, sqlite3.Row) else row[0]
        if resolve_avatar_file(str(cid)):
            continue
        if isinstance(row, sqlite3.Row):
            missing.append(
                {
                    "id": str(row["id"]),
                    "name_zh": str(row["name_zh"] or ""),
                    "wiki_title": str(row["wiki_title"] or ""),
                }
            )
        else:
            missing.append(
                {
                    "id": str(row[0]),
                    "name_zh": str(row[1] or ""),
                    "wiki_title": str(row[2] or ""),
                }
            )
    return missing


def fetch_one(
    character: dict[str, str],
    wiki_db: Path | None = None,
) -> dict[str, Any]:
    cid = character["id"]
    if resolve_avatar_file(cid):
        return {"id": cid, "status": "skipped", "reason": "exists"}
    wiki_db = wiki_db or default_wiki_db()
    url = lookup_avatar_url(
        wiki_db,
        wiki_title=character.get("wiki_title") or "",
        name_zh=character.get("name_zh") or "",
    )
    if not url:
        return {"id": cid, "status": "skipped", "reason": "no_url"}
    try:
        path = download_avatar(url, cid)
        return {"id": cid, "status": "ok", "path": str(path)}
    except (urllib.error.URLError, urllib.error.HTTPError, ValueError, OSError) as exc:
        return {"id": cid, "status": "error", "error": str(exc)}
