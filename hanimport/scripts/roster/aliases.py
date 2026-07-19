"""Live2D / painting-folder aliases and roster enrich."""
from __future__ import annotations

import re
import sqlite3

from roster.folder_rules import (
    ADULT_CN_FOR_XIAO_NICK,
    IDOL_SLUG_BY_ADULT_CN,
    MIDDLE_DOT_RE,
    MU_IDOL_NAME_RE,
    is_meta_display_name,
    strip_meta_display_name,
    strip_skin,
    u_boat_folder_slug,
    yi_num_folder_slug,
    _pinyin_folder_slug,
)

LATIN_RE = re.compile(
    r"[A-Za-zÄÖÜäöüßÁÉÍÓÚáéíóúÀÈÌÒÙàèìòùÂÊÎÔÛâêîôûÃÑÕãñõÅåÆæØø]"
)

CJK_RE = re.compile(r"[\u3040-\u30ff\u3400-\u9fff\uf900-\ufaff]")

LIVE2D_ALIASES = {
    "aijiang": "埃吉尔",
    "aijier": "埃吉尔",
    "abeikelongbi": "阿贝克隆比",
    "adaerbote": "阿达尔伯特亲王",
    "aerbien": "阿尔比恩",
    "aersasi": "阿尔萨斯",
    "aidang": "爱宕",
    "aierdeliqi": "埃尔德里奇",
    "kubo": "可怖",
    # Game folder uses Latin I404; Wiki / pinyin slug would be yi404
    "i404": "伊404",
}

ALIAS_PRIMARY_BY_CN: dict[str, str] = {
    "埃吉尔": "aijiang",
    "可怖": "kubo",
    "大凤": "dafeng",
}

SHIP_CODE_ID = re.compile(r"^[A-Za-z][A-Za-z0-9_-]*\d[A-Za-z0-9_-]*$")

HASH_PERSONA_ID = re.compile(r"^p[0-9a-f]{8}$", re.I)

def _adult_painting_slug(
    cn: str, alias_map: dict[str, str] | None = None
) -> str | None:
    """Folder token for a non-variant ship name (no μ / 小 / META wrapper)."""
    cn = (cn or "").strip()
    if not cn:
        return None
    yi = yi_num_folder_slug(cn)
    if yi:
        return yi
    ub = u_boat_folder_slug(cn)
    if ub:
        return ub
    alias_map = alias_map or LIVE2D_ALIASES
    if _matches_ship_code_pattern(cn):
        return cn.upper().replace("-", "").lower()
    primary = ALIAS_PRIMARY_BY_CN.get(cn)
    if primary:
        return primary.lower()
    slugs = [s for s, c in alias_map.items() if (c or "").strip() == cn and s]
    if slugs:
        return slugs[0].lower()
    return _pinyin_folder_slug(MIDDLE_DOT_RE.sub("", cn)) or None

def mu_idol_folder_slug(
    name: str, alias_map: dict[str, str] | None = None
) -> str | None:
    """光辉(μ兵装) → guanghui_idol."""
    m = MU_IDOL_NAME_RE.fullmatch((name or "").strip())
    if not m:
        return None
    adult = m.group(1).strip()
    short = IDOL_SLUG_BY_ADULT_CN.get(adult)
    base = short or _adult_painting_slug(adult, alias_map)
    return f"{base}_idol" if base else None

def xiao_younv_folder_slug(
    name: str, alias_map: dict[str, str] | None = None
) -> str | None:
    """小企业 → qiye_younv; 小贝法 → beierfasite_younv."""
    zh = (name or "").strip()
    if not zh.startswith("小") or len(zh) < 2:
        return None
    nick = zh[1:]
    adult = ADULT_CN_FOR_XIAO_NICK.get(nick, nick)
    base = _adult_painting_slug(adult, alias_map)
    # 埃吉尔 younv folder is aijier_younv (not primary aijiang)
    if adult == "埃吉尔":
        base = "aijier"
    return f"{base}_younv" if base else None

def meta_alter_slug(
    name: str, alias_map: dict[str, str] | None = None
) -> str | None:
    """Folder id for a META ship: ``{base}_alter`` (伊丽莎白女王·META → yilishabai_alter).

    Prefers an existing painting-folder alias for the base ship (yilishabai),
    else pinyin of the stripped Chinese name.
    """
    zh = (name or "").strip()
    if not zh or not is_meta_display_name(zh):
        return None
    base = strip_meta_display_name(zh)
    if not base:
        return None
    alias_map = alias_map or LIVE2D_ALIASES
    pref = preferred_slug_for_cn(base, alias_map)
    if pref and pref.isascii() and not HASH_PERSONA_ID.match(pref):
        # Hull codes like U-556 → folder token u556 (no hyphen)
        base_slug = pref.lower().replace("-", "")
    else:
        # Middle dots are not part of game folder tokens
        base_slug = _pinyin_folder_slug(MIDDLE_DOT_RE.sub("", base))
    if not base_slug:
        return None
    return f"{base_slug}_alter"

def _matches_ship_code_pattern(cid: str) -> bool:
    """Raw hull-code shape (Z46 / u-2501), ignoring aliases."""
    s = (cid or "").strip()
    if not s or not s.isascii():
        return False
    if HASH_PERSONA_ID.match(s):
        return False
    return bool(SHIP_CODE_ID.match(s))

def _alias_is_cjk_display(alias_val: str) -> bool:
    """True when alias target is a real display name (CJK), not hull-code spelling."""
    v = (alias_val or "").strip()
    return bool(v and CJK_RE.search(v))

def is_ship_code_id(cid: str, alias_map: dict[str, str] | None = None) -> bool:
    """True for hull codes like Z46 / z23 — not CJK-aliased painting folders.

    ``z13→Z13`` (case-only / hull spelling) still counts as a ship code.
    ``i404→伊404`` (CJK display) does not.
    """
    s = (cid or "").strip()
    if not _matches_ship_code_pattern(s):
        return False
    aliased = _alias_cn_for_slug(s, alias_map or LIVE2D_ALIASES)
    if not aliased:
        return True
    if _alias_is_cjk_display(aliased):
        return False
    # Alias to another hull spelling (z13→Z13) → still a ship code
    if _matches_ship_code_pattern(aliased) and aliased.casefold() == s.casefold():
        return True
    return False

def normalize_character_id(
    cid: str, alias_map: dict[str, str] | None = None
) -> str:
    """Canonical roster id: CJK alias first, else hull-code UPPERCASE (z46 → Z46)."""
    s = (cid or "").strip()
    if not s:
        return s
    alias_map = alias_map or LIVE2D_ALIASES
    cn = _alias_cn_for_slug(s, alias_map)
    if cn:
        # z13→Z13: normalize to uppercase hull id, do not reverse-map to folder slug
        if _matches_ship_code_pattern(cn) and cn.casefold() == s.casefold():
            return cn.upper()
        if _alias_is_cjk_display(cn):
            pref = preferred_slug_for_cn(cn, alias_map)
            return pref or s
        if _matches_ship_code_pattern(cn):
            return cn.upper()
        pref = preferred_slug_for_cn(cn, alias_map)
        return pref or s
    if _matches_ship_code_pattern(s):
        return s.upper()
    return s

def preferred_slug_for_cn(cn: str, alias_map: dict[str, str] | None = None) -> str | None:
    """同一中文名下多个拼音别名 → 唯一权威 id。"""
    cn = (cn or "").strip()
    if not cn:
        return None
    # 伊+digits always use Latin i{N} folders (never pinyin yi{N})
    yi = yi_num_folder_slug(cn)
    if yi:
        return yi
    ub = u_boat_folder_slug(cn)
    if ub:
        return ub
    mu = mu_idol_folder_slug(cn, alias_map or LIVE2D_ALIASES)
    if mu:
        return mu
    xiao = xiao_younv_folder_slug(cn, alias_map or LIVE2D_ALIASES)
    if xiao:
        return xiao
    alias_map = alias_map or LIVE2D_ALIASES
    # Hull codes always canonicalize to UPPERCASE id (ignore reverse alias z13←Z13)
    if _matches_ship_code_pattern(cn):
        return cn.upper()
    slugs = [s for s, c in alias_map.items() if (c or "").strip() == cn and s]
    if not slugs:
        for s, c in alias_map.items():
            cv = (c or "").strip()
            if cv.casefold() == cn.casefold() and _matches_ship_code_pattern(cv):
                if _alias_is_cjk_display(_alias_cn_for_slug(cv, alias_map)):
                    continue
                return cv.upper()
        return None
    primary = ALIAS_PRIMARY_BY_CN.get(cn)
    if primary and primary in slugs:
        if _matches_ship_code_pattern(primary) and not _alias_is_cjk_display(
            _alias_cn_for_slug(primary, alias_map)
        ):
            return primary.upper()
        return primary
    for s, c in alias_map.items():
        if (c or "").strip() == cn and s:
            if _matches_ship_code_pattern(c) and not _alias_is_cjk_display(
                _alias_cn_for_slug(c, alias_map)
            ):
                return c.upper()
            return s
    return slugs[0]

def enrich_alias_map_from_roster(
    conn: sqlite3.Connection, alias_map: dict[str, str]
) -> dict[str, str]:
    """Fill painting-folder slugs (pinyin) → 中文名 from local roster rows.

    Game AssetBundle folders use pinyin (changmen_6); wiki roster ids are often
    hashed (p8ae802d6). Without this bridge, bind only hits the few rows whose
    id already equals the folder base.

    META ships use ``{base}_alter`` (声望·META → shengwang_alter). Base slug
    prefers an existing non-META painting alias (伊丽莎白女王 → yilishabai_alter).

    伊+digits ships use Latin ``i{N}`` (伊56 → i56), never pinyin ``yi56``.
    """
    out = dict(alias_map)

    def _zh_slug(name: str) -> str:
        return _pinyin_folder_slug(name)

    # wiki first so duplicate 中文名 prefer wiki character names in alias
    rows = list(
        conn.execute(
            """
            SELECT name_zh, id, source FROM characters
            ORDER BY CASE source WHEN 'wiki' THEN 0 ELSE 1 END, id
            """
        )
    )

    # Pass 1: non-META ships → pinyin / ascii id aliases
    meta_rows: list[tuple[str, str]] = []
    for name_zh, cid, _src in rows:
        zh = (name_zh or "").strip()
        if not zh:
            continue
        if is_meta_display_name(zh):
            meta_rows.append((zh, str(cid or "").strip()))
            continue
        # U-101 → u101 (ascii hull names skipped by the CJK path below)
        ub = u_boat_folder_slug(zh)
        if ub:
            out[ub] = zh
            continue
        if zh.isascii():
            continue
        # 光辉(μ兵装) → guanghui_idol; 小企业 → qiye_younv
        mu = mu_idol_folder_slug(zh, out)
        if mu:
            out[mu] = zh
            # still map adult base for normal skins; do not use μ name as pinyin base
            continue
        xiao = xiao_younv_folder_slug(zh, out)
        if xiao:
            out[xiao] = zh
            continue
        # 伊56 → i56 (skip pinyin yi56 which would miss game folders)
        yi_slug = yi_num_folder_slug(zh)
        if yi_slug:
            out[yi_slug] = zh
            ascii_id = str(cid or "").strip().lower()
            if (
                ascii_id
                and ascii_id.isascii()
                and not re.fullmatch(r"p[0-9a-f]{8,}", ascii_id)
                and re.fullmatch(r"[a-z][a-z0-9_]*", ascii_id)
            ):
                out.setdefault(ascii_id, zh)
            continue
        slug = _zh_slug(zh)
        if slug and slug not in out:
            out[slug] = zh
        # also map existing ascii id when it looks like a painting folder
        ascii_id = str(cid or "").strip().lower()
        # Skip hashed wiki ids like p8ae802d6; keep real pinyin ids (aijiang)
        if (
            ascii_id
            and ascii_id.isascii()
            and not re.fullmatch(r"p[0-9a-f]{8,}", ascii_id)
            and re.fullmatch(r"[a-z][a-z0-9_]*", ascii_id)
        ):
            out.setdefault(ascii_id, zh)

    # Pass 2: META → {base_slug}_alter (needs pass-1 aliases for base ships)
    for zh, _cid in meta_rows:
        alter = meta_alter_slug(zh, out)
        if alter and alter not in out:
            out[alter] = zh
        # Also keep raw pinyin(full)·meta garbage out of the map (never added above)

    # Pass 3: already-bound default folders (game romanization ≠ pinyin, e.g. danfo≠danfu)
    for folder, name_zh in conn.execute(
        """
        SELECT COALESCE(NULLIF(s.pet_model_id,''), NULLIF(s.kanmusu_dir,'')),
               c.name_zh
        FROM skins s
        JOIN characters c ON c.id = s.character_id
        WHERE s.is_default = 1
        """
    ):
        raw = (folder or "").strip()
        zh = (name_zh or "").strip()
        if not raw or not zh:
            continue
        base, suf = strip_skin(raw)
        if base and not suf:
            out.setdefault(base.lower(), zh)
    return out

def _alias_cn_for_slug(slug: str, alias_map: dict[str, str] | None = None) -> str:
    """Resolve painting-folder / stub slug → Chinese display name via aliases."""
    s = (slug or "").strip()
    if not s:
        return ""
    alias_map = alias_map or LIVE2D_ALIASES
    return (alias_map.get(s) or alias_map.get(s.lower()) or "").strip()

def alias_redirect_id(cid: str, alias_map: dict[str, str] | None = None) -> str:
    """若 cid 是次要别名，改写成权威 id。优先别名，无别名再大写规范化。"""
    cid = (cid or "").strip()
    if not cid:
        return cid
    return normalize_character_id(cid, alias_map)
