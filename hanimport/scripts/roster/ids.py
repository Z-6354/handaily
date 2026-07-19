"""Roster id helpers + re-export of folder_rules / aliases."""
from __future__ import annotations

import hashlib
import re
import sqlite3
from pathlib import Path

from roster.folder_rules import *  # noqa: F403
from roster.aliases import *  # noqa: F403

# Re-export private helpers used by bind_pipeline / tests
from roster.folder_rules import (  # noqa: F401
    _pinyin_folder_slug,
    _suffix_variant_kind,
)
from roster.aliases import (  # noqa: F401
    _adult_painting_slug,
    _alias_cn_for_slug,
    _alias_is_cjk_display,
    _matches_ship_code_pattern,
)

def dedupe_characters_by_name(characters: list) -> list:
    """Collapse BWIKI-hash + pinyin duplicates (same zh name) into pinyin id."""
    from collections import defaultdict

    # 与 merge_duplicate_characters 共用序号合并，避免导入后 Spine/舰娘分两行
    try:
        from merge_duplicate_characters import coalesce_skins
    except ImportError:
        coalesce_skins = None  # type: ignore

    by_name: dict[str, list] = defaultdict(list)
    orphans = []
    for c in characters:
        if not isinstance(c, dict):
            continue
        name = (c.get("name") or "").strip()
        if not name:
            orphans.append(c)
            continue
        by_name[name].append(c)

    out: list = []
    for name, group in by_name.items():
        if len(group) == 1:
            c = dict(group[0])
            cid = str(c.get("id") or "")
            skins = list(c.get("skins") or [])
            if coalesce_skins and skins:
                c["skins"] = coalesce_skins(skins, cid)
            out.append(c)
            continue
        non_hash = [c for c in group if not HASH_PERSONA_ID.match(str(c.get("id") or ""))]
        canon = dict(non_hash[0] if non_hash else group[0])
        donors = [c for c in group if c.get("id") != canon.get("id")]
        skins_by_id = {
            s.get("id"): dict(s)
            for s in (canon.get("skins") or [])
            if isinstance(s, dict) and s.get("id")
        }
        for d in donors:
            for s in d.get("skins") or []:
                if not isinstance(s, dict) or not s.get("id"):
                    continue
                sid = s["id"]
                if sid not in skins_by_id:
                    skins_by_id[sid] = dict(s)
                    continue
                cur = skins_by_id[sid]
                if not (cur.get("model_id") or "").strip() and (s.get("model_id") or "").strip():
                    cur["model_id"] = s["model_id"]
                if not (cur.get("kanmusu_dir") or "").strip() and (s.get("kanmusu_dir") or "").strip():
                    cur["kanmusu_dir"] = s["kanmusu_dir"]
                if cur.get("skin_index") is None and s.get("skin_index") is not None:
                    cur["skin_index"] = s["skin_index"]
            if not (canon.get("english_name") or "").strip() and (d.get("english_name") or "").strip():
                canon["english_name"] = d["english_name"]
            if not (canon.get("wiki_title") or "").strip() and (d.get("wiki_title") or "").strip():
                canon["wiki_title"] = d["wiki_title"]
        skins = list(skins_by_id.values())
        cid = str(canon.get("id") or "")
        canon["skins"] = coalesce_skins(skins, cid) if coalesce_skins else skins
        canon["persona_id"] = canon.get("id")
        out.append(canon)
    out.extend(orphans)
    return out

def clean_cv(raw: str) -> str:
    """Wiki CV field is often polluted by scrape noise; keep short readable text only."""
    s = (raw or "").strip()
    if not s:
        return ""
    if any(tok in s for tok in (".jpg", ".png", ".webp", "decoding=", "<img", "Skillicon", "%E")):
        return ""
    # Prefer segment before "画师" if present
    if "画师" in s:
        s = s.split("画师", 1)[0].strip(" /|")
    if len(s) > 160:
        s = s[:160].rstrip() + "…"
    return s

def pick_english(aliases: list, fallback: str) -> str:
    best = ""
    for a in aliases or []:
        if not isinstance(a, str):
            continue
        s = a.strip()
        if not s or not LATIN_RE.search(s):
            continue
        cjk = len(CJK_RE.findall(s))
        if cjk > len(s) / 2:
            continue
        if len(s) > len(best):
            best = s
    return best or fallback

def pick_skin_title(assets: list, skin_index: int | None, fallback: str) -> str:
    skins = [
        a
        for a in (assets or [])
        if isinstance(a, dict) and a.get("kind") == "skin" and isinstance(a.get("name"), str)
    ]
    labeled = [s for s in skins if "换装" in str(s.get("name"))]
    pool = labeled or skins
    if not pool:
        return fallback
    if skin_index is None or skin_index <= 0:
        return Path(str(pool[0]["name"])).stem.replace(".jpg", "")
    for s in pool:
        stem = Path(str(s["name"])).stem
        m = re.search(r"换装\s*(\d+)", stem)
        if m and int(m.group(1)) + 1 == skin_index:
            return stem
        if m and int(m.group(1)) == skin_index:
            return stem
    if skin_index == 2 and pool:
        for s in pool:
            stem = Path(str(s["name"])).stem
            if re.search(r"换装\s*$", stem) or stem.endswith("换装"):
                return stem
    return fallback

def is_hx_skin(*, skin_id: str = "", kanmusu_dir: str = "", name_zh: str = "") -> bool:
    """True for censored/harmonized skins (folder or L2D id ending in _hx / -hx)."""
    from common.unpack_complete import is_hx_slug

    kd = (kanmusu_dir or "").strip()
    if kd and is_hx_slug(kd):
        return True
    sid = (skin_id or "").strip().lower()
    if sid.endswith("_hx") or sid.endswith("-hx"):
        return True
    if "-l2d-" in sid:
        suf = sid.rsplit("-l2d-", 1)[-1]
        if suf == "hx" or suf.endswith("_hx"):
            return True
    # name like 皮肤3_hx / 默认_hx
    nz = (name_zh or "").strip().lower()
    if nz.endswith("_hx") or nz.endswith("-hx"):
        return True
    return False

def skin_db_id(character_id: str, skin_id: str) -> str:
    """SQLite skins.id is global PK — map JSON 'default' → '{char}-default'."""
    sid = (skin_id or "").strip() or "default"
    if sid == "default":
        return f"{character_id}-default"
    return sid

def skin_manifest_id(character_id: str, db_skin_id: str) -> str:
    """Export '{char}-default' back to 'default' for AppData / bundled JSON."""
    if db_skin_id == f"{character_id}-default":
        return "default"
    return db_skin_id

def normalize_name_en(name_en: str, id_: str) -> str:
    s = (name_en or "").strip()
    return s if s else id_

def _parse_id_filter(only_ids: list[str] | str | None) -> set[str] | None:
    if only_ids is None or only_ids == "":
        return None
    if isinstance(only_ids, str):
        parts = [x.strip() for x in only_ids.replace(";", ",").split(",") if x.strip()]
    else:
        parts = [str(x).strip() for x in only_ids if str(x).strip()]
    return set(parts) if parts else None

def _stable_wiki_id(cn: str) -> str:
    """无拼音 slug 时用稳定 hash id（与历史 AppData pXXXXXXXX 风格一致）。"""
    digest = hashlib.md5((cn or "").encode("utf-8")).hexdigest()[:8]
    return f"p{digest}"

def _parse_import_phases(phases: set[str] | list[str] | str | None) -> set[str]:
    """characters | skins | lines | bind；默认全开。"""
    all_phases = {"characters", "skins", "lines", "bind"}
    if phases is None or phases == "" or phases == "all":
        return set(all_phases)
    if isinstance(phases, str):
        parts = {p.strip() for p in phases.replace(";", ",").split(",") if p.strip()}
    else:
        parts = {str(p).strip() for p in phases if str(p).strip()}
    parts = parts & all_phases
    return parts if parts else set(all_phases)
