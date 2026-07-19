"""Local handaily-skin-slot pack/unpack (phase ①; no upload)."""
from __future__ import annotations

import json
import re
import sqlite3
import zipfile
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

from roster.ids import is_oath_skin_id
from roster.skin_probe import _has_cubism_assets, _has_spine_assets

FORMAT = "handaily-skin-slot"
FORMAT_VERSION = 1

_SAFE_ID = re.compile(r"^[A-Za-z0-9._-]+$")


@dataclass(frozen=True)
class SkipReason:
    code: str
    message: str


@dataclass
class PackResult:
    path: Path | None
    skipped: SkipReason | None = None


def slot_zip_name(character_id: str, skin_id: str) -> str:
    cid = (character_id or "").strip()
    sid = (skin_id or "").strip()
    if not cid or not sid:
        raise ValueError("character_id and skin_id required")
    if not _SAFE_ID.match(cid) or not _SAFE_ID.match(sid):
        raise ValueError(f"unsafe id in zip name: {cid!r} / {sid!r}")
    if "/" in cid or "\\" in cid or "/" in sid or "\\" in sid:
        raise ValueError("path separators not allowed")
    return f"{cid}__{sid}.slot.zip"


def lines_from_db(conn: sqlite3.Connection, skin_id: str) -> list[dict[str, Any]]:
    rows = conn.execute(
        """
        SELECT wiki_key, label, lang, text, animation, sort_order
        FROM skin_lines
        WHERE skin_id = ?
        ORDER BY sort_order, id
        """,
        (skin_id,),
    ).fetchall()
    out: list[dict[str, Any]] = []
    for r in rows:
        out.append(
            {
                "wiki_key": r["wiki_key"] if isinstance(r, sqlite3.Row) else r[0],
                "label": r["label"] if isinstance(r, sqlite3.Row) else r[1],
                "lang": r["lang"] if isinstance(r, sqlite3.Row) else r[2],
                "text": r["text"] if isinstance(r, sqlite3.Row) else r[3],
                "animation": r["animation"] if isinstance(r, sqlite3.Row) else r[4],
                "sort_order": int(
                    r["sort_order"] if isinstance(r, sqlite3.Row) else r[5] or 0
                ),
            }
        )
    return out


def build_manifest(
    character: dict[str, Any],
    skin: dict[str, Any],
    *,
    has_pet: bool,
    has_kanmusu: bool,
    packed_at: str | None = None,
) -> dict[str, Any]:
    sid = str(skin.get("id") or "")
    return {
        "format": FORMAT,
        "format_version": FORMAT_VERSION,
        "character": {
            "id": str(character.get("id") or ""),
            "name_zh": str(character.get("name_zh") or ""),
            "name_en": str(character.get("name_en") or ""),
            "faction": str(character.get("faction") or ""),
            "wiki_title": str(character.get("wiki_title") or ""),
        },
        "skin": {
            "id": sid,
            "name_zh": str(skin.get("name_zh") or ""),
            "is_default": bool(int(skin.get("is_default") or 0)),
            "is_oath": is_oath_skin_id(sid),
            "pet_model_id": str(skin.get("pet_model_id") or ""),
            "kanmusu_dir": str(skin.get("kanmusu_dir") or ""),
            "has_pet": bool(has_pet),
            "has_kanmusu": bool(has_kanmusu),
        },
        "lines": {"path": "lines.json"},
        "packed_at": packed_at
        or datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
    }


def check_slot_eligible(
    skin: dict[str, Any],
    *,
    pet_root: Path,
    skin_root: Path,
) -> tuple[SkipReason | None, Path | None, Path | None]:
    """Return (skip, pet_dir, skin_dir). pet_dir set only when eligible."""
    pet_id = str(skin.get("pet_model_id") or "").strip()
    km = str(skin.get("kanmusu_dir") or "").strip()

    if not pet_id:
        if km:
            return (
                SkipReason("skin_only", "仅有舰娘目录、无桌宠绑定，已跳过"),
                None,
                None,
            )
        return (
            SkipReason("unbound", "未绑定桌宠（pet_model_id 为空），已跳过"),
            None,
            None,
        )

    pet_dir = pet_root / pet_id
    if not _has_spine_assets(pet_dir):
        return (
            SkipReason("no_pet", f"桌宠目录缺失或不完整: {pet_id}"),
            None,
            None,
        )

    skin_dir: Path | None = None
    if km:
        cand = skin_root / km
        if _has_cubism_assets(cand):
            skin_dir = cand
        # kanmusu listed but missing → still allow pet-only pack
    return None, pet_dir, skin_dir


def _find_avatar(avatar_dir: Path | None, character_id: str) -> Path | None:
    if not avatar_dir or not avatar_dir.is_dir():
        return None
    for ext in (".webp", ".png", ".jpg", ".jpeg"):
        p = avatar_dir / f"{character_id}{ext}"
        if p.is_file():
            return p
    return None


def _add_tree(zf: zipfile.ZipFile, folder: Path, arc_prefix: str) -> None:
    folder = folder.resolve()
    for path in sorted(folder.rglob("*")):
        if not path.is_file():
            continue
        rel = path.relative_to(folder).as_posix()
        zf.write(path, f"{arc_prefix}/{rel}")


def pack_slot(
    conn: sqlite3.Connection,
    skin_id: str,
    *,
    pet_root: Path,
    skin_root: Path,
    out_dir: Path,
    avatar_dir: Path | None = None,
) -> PackResult:
    row = conn.execute(
        """
        SELECT s.*, c.name_zh AS c_name_zh, c.name_en AS c_name_en,
               c.faction AS c_faction, c.wiki_title AS c_wiki_title, c.id AS c_id
        FROM skins s
        JOIN characters c ON c.id = s.character_id
        WHERE s.id = ?
        """,
        (skin_id,),
    ).fetchone()
    if row is None:
        return PackResult(
            None, SkipReason("missing", f"皮肤不存在: {skin_id}")
        )

    def g(key: str, default: Any = "") -> Any:
        try:
            return row[key]
        except (KeyError, IndexError):
            return default

    skin = {
        "id": g("id"),
        "character_id": g("character_id"),
        "name_zh": g("name_zh"),
        "is_default": g("is_default"),
        "pet_model_id": g("pet_model_id"),
        "kanmusu_dir": g("kanmusu_dir"),
    }
    character = {
        "id": g("c_id") or g("character_id"),
        "name_zh": g("c_name_zh"),
        "name_en": g("c_name_en"),
        "faction": g("c_faction"),
        "wiki_title": g("c_wiki_title"),
    }

    skip, pet_dir, skin_dir = check_slot_eligible(
        skin, pet_root=pet_root, skin_root=skin_root
    )
    if skip or pet_dir is None:
        return PackResult(None, skip)

    has_km = skin_dir is not None
    manifest = build_manifest(
        character, skin, has_pet=True, has_kanmusu=has_km
    )
    lines = lines_from_db(conn, skin_id)

    out_dir.mkdir(parents=True, exist_ok=True)
    out_path = out_dir / slot_zip_name(str(character["id"]), str(skin["id"]))

    with zipfile.ZipFile(out_path, "w", compression=zipfile.ZIP_DEFLATED) as zf:
        zf.writestr(
            "manifest.json",
            json.dumps(manifest, ensure_ascii=False, indent=2) + "\n",
        )
        zf.writestr(
            "lines.json",
            json.dumps(lines, ensure_ascii=False, indent=2) + "\n",
        )
        avatar = _find_avatar(avatar_dir, str(character["id"]))
        if avatar is not None:
            # normalize to avatar.webp|png… matching source ext
            zf.write(avatar, f"avatar{avatar.suffix.lower()}")
        pet_slug = str(skin["pet_model_id"]).strip()
        _add_tree(zf, pet_dir, f"pet/{pet_slug}")
        if skin_dir is not None:
            km_slug = str(skin["kanmusu_dir"]).strip()
            _add_tree(zf, skin_dir, f"skin/{km_slug}")

    return PackResult(out_path, None)


def pack_many(
    conn: sqlite3.Connection,
    skin_ids: list[str],
    *,
    pet_root: Path,
    skin_root: Path,
    out_dir: Path,
    avatar_dir: Path | None = None,
) -> list[PackResult]:
    return [
        pack_slot(
            conn,
            sid,
            pet_root=pet_root,
            skin_root=skin_root,
            out_dir=out_dir,
            avatar_dir=avatar_dir,
        )
        for sid in skin_ids
    ]


def _safe_zip_members(zf: zipfile.ZipFile) -> list[zipfile.ZipInfo]:
    out: list[zipfile.ZipInfo] = []
    for info in zf.infolist():
        name = info.filename.replace("\\", "/")
        if name.startswith("/") or ".." in name.split("/"):
            raise ValueError(f"unsafe zip member: {info.filename}")
        out.append(info)
    return out


def unpack_slot(zip_path: Path, *, dest_root: Path) -> dict[str, Any]:
    """Extract slot zip into dest_root/{pet,skin,avatars}; return manifest."""
    dest_root.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(zip_path, "r") as zf:
        _safe_zip_members(zf)
        raw = zf.read("manifest.json")
        manifest = json.loads(raw.decode("utf-8"))
        if manifest.get("format") != FORMAT:
            raise ValueError(f"unsupported format: {manifest.get('format')}")

        for info in zf.infolist():
            name = info.filename.replace("\\", "/")
            if name.endswith("/"):
                continue
            if name in ("manifest.json", "lines.json") or name.startswith(
                "avatar."
            ):
                continue
            if name.startswith("pet/") or name.startswith("skin/"):
                target = dest_root / name
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_bytes(zf.read(info))

        # avatar
        cid = str((manifest.get("character") or {}).get("id") or "")
        for info in zf.infolist():
            name = info.filename.replace("\\", "/")
            if name.startswith("avatar.") and not name.endswith("/"):
                ext = Path(name).suffix.lower() or ".webp"
                av_dir = dest_root / "avatars"
                av_dir.mkdir(parents=True, exist_ok=True)
                if cid:
                    (av_dir / f"{cid}{ext}").write_bytes(zf.read(info))
                break

        # lines sidecar next to unpack root for local verify
        if "lines.json" in zf.namelist():
            lines_path = dest_root / "lines" / f"{manifest['skin']['id']}.json"
            lines_path.parent.mkdir(parents=True, exist_ok=True)
            lines_path.write_bytes(zf.read("lines.json"))

        meta_path = dest_root / "manifests" / f"{manifest['skin']['id']}.json"
        meta_path.parent.mkdir(parents=True, exist_ok=True)
        meta_path.write_text(
            json.dumps(manifest, ensure_ascii=False, indent=2) + "\n",
            encoding="utf-8",
        )
    return manifest
