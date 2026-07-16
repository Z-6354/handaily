"""Local roster avatar resolve + download from BLHX wiki catalog."""
from __future__ import annotations

import re
import sqlite3
import urllib.error
import urllib.request
from functools import lru_cache
from pathlib import Path
from typing import Any
from urllib.parse import urlparse

from roster_db import repo_root, roster_dir

_SAFE_ID = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]{0,120}$")
_USER_AGENT = "hanimport-avatar/0.1 (+local; handaily)"
_LATIN_TOKEN = re.compile(r"^[A-Za-z]+$")


def avatars_dir() -> Path:
    d = roster_dir() / "avatars"
    d.mkdir(parents=True, exist_ok=True)
    return d


def is_safe_character_id(cid: str) -> bool:
    return bool(cid and _SAFE_ID.match(cid))


def to_pinyin_slug(text: str) -> str:
    """Chinese (or mixed) display name → live2d-style slug (no tones, latin only)."""
    from pypinyin import lazy_pinyin

    parts = lazy_pinyin(text or "")
    return "".join(p for p in parts if _LATIN_TOKEN.match(p)).lower()


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


@lru_cache(maxsize=4)
def _catalog_slug_index(wiki_db_str: str) -> dict[str, dict[str, str]]:
    """slug → {wiki_title, display_name, avatar_url} from catalog (pinyin of CN names)."""
    path = Path(wiki_db_str)
    if not path.is_file():
        return {}
    out: dict[str, dict[str, str]] = {}
    conn = sqlite3.connect(str(path))
    try:
        rows = conn.execute(
            """
            SELECT wiki_title, display_name, avatar_url FROM catalog
            WHERE avatar_url IS NOT NULL AND length(avatar_url) > 0
            """
        ).fetchall()
    finally:
        conn.close()
    for wiki_title, display_name, avatar_url in rows:
        title = (wiki_title or "").strip()
        display = (display_name or "").strip() or title
        url = (avatar_url or "").strip()
        if not url:
            continue
        for label in (display, title):
            if not label:
                continue
            slug = to_pinyin_slug(label)
            if slug and slug not in out:
                out[slug] = {
                    "wiki_title": title or display,
                    "display_name": display or title,
                    "avatar_url": url,
                }
    return out


def lookup_avatar_url(
    wiki_db: Path,
    *,
    wiki_title: str = "",
    name_zh: str = "",
    character_id: str = "",
) -> str | None:
    if not wiki_db.is_file():
        return None
    keys: list[str] = []
    for k in (wiki_title, name_zh):
        s = (k or "").strip()
        if s and s not in keys:
            keys.append(s)
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

    # Unpacked stubs: name_zh == pinyin id; resolve via catalog pinyin slug
    slug = (character_id or "").strip().lower()
    if not slug:
        for k in keys:
            if k.isascii() and k.replace("_", "").isalnum():
                slug = k.lower()
                break
    if slug:
        hit = resolve_catalog_by_slug(wiki_db, slug)
        if hit and hit.get("avatar_url"):
            return hit["avatar_url"]
    return None


def _load_slug_aliases() -> dict[str, str]:
    from roster_db import LIVE2D_ALIASES, load_json, repo_root

    return {
        **LIVE2D_ALIASES,
        **load_json(repo_root() / "mcp" / "blhx-wiki" / "data" / "live2d-aliases.json", {}),
    }


def _slug_lookup_candidates(slug: str) -> list[str]:
    """Exact slug plus soft variants (class suffix / ding↔ting)."""
    s = (slug or "").strip().lower()
    if not s:
        return []
    out: list[str] = []

    def add(x: str) -> None:
        if x and x not in out:
            out.append(x)

    add(s)
    for suf in ("_cv", "_bb", "_dd", "_cl", "_ca", "_ss", "_bc", "_cvl", "_doa"):
        if s.endswith(suf):
            add(s[: -len(suf)])
    if s.endswith("ding"):
        add(s[:-4] + "ting")
    elif s.endswith("ting"):
        add(s[:-4] + "ding")
    return out


def _catalog_row_by_cn(wiki_db: Path, name: str) -> dict[str, str] | None:
    name = (name or "").strip()
    if not name or not wiki_db.is_file():
        return None
    conn = sqlite3.connect(str(wiki_db))
    try:
        row = conn.execute(
            """
            SELECT wiki_title, display_name, avatar_url FROM catalog
            WHERE (wiki_title = ? OR display_name = ?)
              AND avatar_url IS NOT NULL AND length(avatar_url) > 0
            LIMIT 1
            """,
            (name, name),
        ).fetchone()
    finally:
        conn.close()
    if not row:
        return None
    title = (row[0] or "").strip()
    display = (row[1] or "").strip() or title
    url = (row[2] or "").strip()
    if not url:
        return None
    return {
        "wiki_title": title or display,
        "display_name": display or title,
        "avatar_url": url,
    }


def resolve_catalog_by_slug(wiki_db: Path, slug: str) -> dict[str, str] | None:
    """Map live2d folder / character id → catalog row via pinyin / aliases.

    Game folders sometimes use nonstandard pinyin (aimierbeierding vs
    埃米尔·贝尔汀 → aimierbeierting). Aliases + ding↔ting soft match cover that.
    """
    slug = (slug or "").strip().lower()
    if not slug or not wiki_db.is_file():
        return None

    from roster_db import strip_skin

    base, _suffix = strip_skin(slug)
    index = _catalog_slug_index(str(wiki_db.resolve()))
    aliases = _load_slug_aliases()
    tried: set[str] = set()
    for cand in _slug_lookup_candidates(slug) + _slug_lookup_candidates(base):
        if cand in tried:
            continue
        tried.add(cand)
        cn = (aliases.get(cand) or "").strip()
        if cn:
            hit = _catalog_row_by_cn(wiki_db, cn)
            if hit:
                return hit
        hit = index.get(cand)
        if hit:
            return hit
        # Ship codes: z23 → Z23, u2501 → U-2501 (alias preferred; also try uppercase)
        if cand[:1].isalpha() and any(ch.isdigit() for ch in cand):
            for form in (cand.upper(), cand.upper().replace("U", "U-", 1) if cand.startswith("u") and cand[1:].isdigit() else ""):
                if not form:
                    continue
                hit = _catalog_row_by_cn(wiki_db, form)
                if hit:
                    return hit
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
        character_id=cid,
    )
    if not url:
        return {"id": cid, "status": "skipped", "reason": "no_url"}
    try:
        path = download_avatar(url, cid)
        return {"id": cid, "status": "ok", "path": str(path)}
    except (urllib.error.URLError, urllib.error.HTTPError, ValueError, OSError) as exc:
        return {"id": cid, "status": "error", "error": str(exc)}
