#!/usr/bin/env python3
"""Handaily roster DB: local private SQLite + allowlisted bundled preview export.

Commands:
  init | import-wiki | import-bundled-seed | sync-appdata | publish-bundled | export-pack | verify | repair-l2d-binds
"""
from __future__ import annotations

import argparse
import hashlib
import json
import logging
import os
import re
import shutil
import sqlite3
import sys
import zipfile
from pathlib import Path

from roster.line_skin_match import apply_lines_by_skin, merge_meta_json

# --- ids ---
SKIN_SUFFIX = re.compile(
    r"_(?:\d+|h|g|hx|doa|painting|idol|younv|summer|school|winter|swimsuit|wedding|newyear|cn|jp|en|super|asmr)$",
    re.I,
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

# META ships: game folders are ``{base_slug}_alter`` (声望·META → shengwang_alter).
META_SUFFIX_RE = re.compile(
    r"(?:[·・‧\.．]\s*)?META\s*$",
    re.IGNORECASE,
)
MIDDLE_DOT_RE = re.compile(r"[·・‧]")

# 伊+digits submarines: game folders are Latin ``i{digits}`` (伊56 → i56, not yi56).
YI_NUM_NAME_RE = re.compile(r"^伊(\d+)$")

# U-boats: Wiki ``U-101`` → folder ``u101`` (no hyphen).
U_BOAT_NAME_RE = re.compile(r"^U-(\d+)$", re.I)

# μ兵装 / 小舰娘 → ``{adult}_idol`` / ``{adult}_younv``
MU_IDOL_NAME_RE = re.compile(r"^(.+)\(μ兵装\)$")
# Short idol painting id when full pinyin folder is unused
IDOL_SLUG_BY_ADULT_CN: dict[str, str] = {
    "希佩尔海军上将": "xipeier",
}
# 小X display nick → adult wiki name (folder uses adult slug)
ADULT_CN_FOR_XIAO_NICK: dict[str, str] = {
    "贝法": "贝尔法斯特",
    "斯佩": "斯佩伯爵海军上将",
    "腓特烈": "腓特烈大帝",
    "欧根": "欧根亲王",
    "齐柏林": "齐柏林伯爵",
}
XIAO_DISPLAY_BY_ADULT_CN: dict[str, str] = {
    "贝尔法斯特": "小贝法",
    "斯佩伯爵海军上将": "小斯佩",
    "腓特烈大帝": "小腓特烈",
    "欧根亲王": "小欧根",
    "齐柏林伯爵": "小齐柏林",
}


def is_yi_num_display_name(name: str) -> bool:
    """True for 伊56 / 伊404 style names (not 伊势 / 伊丽莎白)."""
    return bool(YI_NUM_NAME_RE.fullmatch((name or "").strip()))


def yi_num_folder_slug(name: str) -> str | None:
    """伊56 → i56; non-matching names → None."""
    m = YI_NUM_NAME_RE.fullmatch((name or "").strip())
    if not m:
        return None
    return f"i{m.group(1)}"


def u_boat_folder_slug(name: str) -> str | None:
    """U-101 → u101."""
    m = U_BOAT_NAME_RE.fullmatch((name or "").strip())
    if not m:
        return None
    return f"u{m.group(1)}"


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


def variant_character_name_for_suffix(adult_cn: str, suffix: str) -> str | None:
    """Map adult display name + folder suffix → variant character name."""
    cn = (adult_cn or "").strip()
    if not cn:
        return None
    kind = _suffix_variant_kind(suffix)
    if kind == "idol":
        return f"{cn}(μ兵装)"
    if kind == "younv":
        return XIAO_DISPLAY_BY_ADULT_CN.get(cn) or f"小{cn}"
    return None


def _suffix_variant_kind(suffix: str) -> str | None:
    s = (suffix or "").strip().lower()
    if s == "idol" or re.fullmatch(r"\d+_idol", s):
        return "idol"
    if s == "younv" or re.fullmatch(r"\d+_younv", s):
        return "younv"
    return None


def is_meta_display_name(name: str) -> bool:
    """True when display name is a META variant (声望·META / 声望META)."""
    return bool(META_SUFFIX_RE.search((name or "").strip()))


def strip_meta_display_name(name: str) -> str:
    """Strip trailing META marker → base ship display name (keep internal ·)."""
    s = (name or "").strip()
    if not s:
        return ""
    s = META_SUFFIX_RE.sub("", s).strip()
    return s.rstrip("·・‧.．").strip()


def _pinyin_folder_slug(name: str) -> str:
    """CJK display name → lowercase pinyin folder token (no skin/META suffix)."""
    try:
        from pypinyin import Style, lazy_pinyin
    except ImportError:
        return ""
    raw = "".join(lazy_pinyin(name or "", style=Style.NORMAL)).lower()
    return re.sub(r"[^a-z0-9_]+", "", raw)


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

def strip_skin(folder: str) -> tuple[str, str]:
    """Split unpack folder into (character_base, skin_suffix).

    Peels trailing tokens until the base ship id remains:
      qiye_9 → (qiye, 9)
      abeikelongbi_3_1 → (abeikelongbi, 3_1)
      abeikelongbi_3_hx → (abeikelongbi, 3_hx)
      z23_hx → (z23, hx)
    """
    name = folder
    parts: list[str] = []
    while True:
        m = SKIN_SUFFIX.search(name)
        if not m or m.start() == 0:
            break
        # Only peel from the end of the string
        if m.end() != len(name):
            break
        parts.append(m.group(0)[1:].lower())
        name = name[: m.start()]
    parts.reverse()
    return name, "_".join(parts)

def is_folder_like_character_id(cid: str) -> bool:
    """True when id looks like an unpack folder (base + skin suffix), not a character."""
    _base, suffix = strip_skin((cid or "").strip())
    return bool(suffix)

def skin_label(suffix: str) -> str:
    if not suffix:
        return "默认皮肤"
    if suffix.isdigit():
        return f"皮肤{suffix}"
    return f"变体_{suffix}"

def is_generic_wiki_skin_label(label: str) -> bool:
    t = (label or "").strip()
    if not t:
        return True
    if t in ("通常", "默认", "默认皮肤", "default", "换装"):
        return True
    if re.match(r"^换装\s*\d*$", t) or re.match(r"^皮肤\s*\d*$", t):
        return True
    return False

def is_hidden_wiki_skin(*, kind: str = "", label: str = "") -> bool:
    """改造 / 改装：不入库、不展示。"""
    if (kind or "").strip() == "retrofit":
        return True
    lab = (label or "").strip()
    if "改造" in lab or "改装" in lab:
        return True
    if lab.endswith(".改"):
        return True
    return False

def is_oath_skin_id(skin_id: str) -> bool:
    return (skin_id or "").strip().endswith("-oath")

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

# --- schema ---
def repo_root() -> Path:
    return Path(__file__).resolve().parents[3]

def roster_dir() -> Path:
    return repo_root() / "data" / "roster"

def schema_path() -> Path:
    return roster_dir() / "schema.sql"

def default_local_db() -> Path:
    override = os.environ.get("HANDAILY_ROSTER_DB", "").strip()
    if override:
        return Path(override)
    return roster_dir() / "handaily-roster.sqlite"

def appdata_data_dir() -> Path:
    override = os.environ.get("HANDAILY_DATA_DIR", "").strip()
    if override:
        return Path(override)
    appdata = os.environ.get("APPDATA") or ""
    return Path(appdata) / "xiaohan-daily" / "data"

def bundled_roster_dir() -> Path:
    return repo_root() / "hanpet" / "bundled" / "roster"

def allowlist_path() -> Path:
    return roster_dir() / "bundled-allowlist.json"

def emit(obj) -> None:
    sys.stdout.buffer.write((json.dumps(obj, ensure_ascii=False, indent=2) + "\n").encode("utf-8"))

def load_json(path: Path, default):
    if not path.is_file():
        return default
    return json.loads(path.read_text(encoding="utf-8-sig"))

def connect(db_path: Path) -> sqlite3.Connection:
    db_path.parent.mkdir(parents=True, exist_ok=True)
    conn = sqlite3.connect(str(db_path), timeout=60.0)
    conn.row_factory = sqlite3.Row
    conn.execute("PRAGMA foreign_keys = ON")
    # Concurrent web reads + wiki pipeline writes (default DELETE journal locks hard)
    conn.execute("PRAGMA journal_mode=WAL")
    conn.execute("PRAGMA busy_timeout=60000")
    return conn

def apply_schema(conn: sqlite3.Connection) -> None:
    sql = schema_path().read_text(encoding="utf-8")
    conn.executescript(sql)
    conn.execute(
        "INSERT INTO meta(key, value) VALUES(?, ?) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        ("schema_version", "1"),
    )
    conn.commit()

def cmd_init(args: argparse.Namespace) -> int:
    db = Path(args.db) if args.db else default_local_db()
    if db.is_file() and not args.force:
        emit({"ok": True, "db": str(db), "note": "already exists (use --force to recreate)"})
        return 0
    if db.is_file() and args.force:
        db.unlink()
    conn = connect(db)
    apply_schema(conn)
    conn.close()
    emit({"ok": True, "db": str(db), "action": "init"})
    return 0

# --- crud ---
def _skin_row_dicts(conn: sqlite3.Connection, cid: str) -> list[dict]:
    rows = conn.execute(
        "SELECT * FROM skins WHERE character_id=? ORDER BY sort_order, id", (cid,)
    ).fetchall()
    return [{k: r[k] for k in r.keys()} for r in rows]

def _skin_is_manual_or_bound(conn: sqlite3.Connection, sid: str) -> bool:
    """True for L2D / bound / unpacked skins that Wiki sync must not purge."""
    row = conn.execute(
        "SELECT id, pet_model_id, kanmusu_dir, meta_json FROM skins WHERE id=?",
        (sid,),
    ).fetchone()
    if not row:
        return False
    sid_s = str(row[0])
    if "-l2d-" in sid_s.lower():
        return True
    if str(row[1] or "").strip() or str(row[2] or "").strip():
        return True
    try:
        meta = json.loads(row[3] or "{}")
    except json.JSONDecodeError:
        return False
    if not isinstance(meta, dict):
        return False
    if str(meta.get("source") or "").lower() == "unpacked":
        return True
    if meta.get("binds") or meta.get("bind"):
        return True
    return False

def _delete_skins_not_in(
    conn: sqlite3.Connection,
    cid: str,
    keep_ids: set[str],
    *,
    purge_orphans: bool = False,
) -> int:
    """删除该角色不在 keep 集合内的皮肤行（台词经 FK CASCADE）。

    默认保留 L2D / 已绑定 / meta.source=unpacked 的皮；仅 purge_orphans=True 时全删。
    """
    rows = conn.execute(
        "SELECT id FROM skins WHERE character_id = ?", (cid,)
    ).fetchall()
    deleted = 0
    for (sid,) in rows:
        if sid in keep_ids:
            continue
        if not purge_orphans and _skin_is_manual_or_bound(conn, sid):
            continue
        conn.execute("DELETE FROM skins WHERE id = ?", (sid,))
        deleted += 1
    return deleted

def expected_skin_ids_from_slots(cid: str, slots: list[dict]) -> set[str]:
    """Wiki skins_json → 权威 skin id 集合（不写库）。"""
    keep: set[str] = set()
    for slot in slots:
        if not isinstance(slot, dict):
            continue
        key = str(slot.get("key") or "").strip()
        label = str(slot.get("label") or key).strip()
        kind = str(slot.get("kind") or "skin")
        if not key or is_hidden_wiki_skin(kind=kind, label=label):
            continue
        if key == "default" or kind == "default":
            keep.add(skin_db_id(cid, "default"))
        elif key == "oath" or kind == "oath":
            keep.add(f"{cid}-oath")
        else:
            keep.add(f"{cid}-{key}")
    return keep

def local_skin_ids(conn: sqlite3.Connection, cid: str) -> set[str]:
    return {
        str(r[0])
        for r in conn.execute(
            "SELECT id FROM skins WHERE character_id=?", (cid,)
        )
    }

def character_skins_in_sync(
    conn: sqlite3.Connection, cid: str, slots: list[dict]
) -> bool:
    """Wiki 权威皮均已存在即可（本地可额外保留 L2D/手工皮）。"""
    keep = expected_skin_ids_from_slots(cid, slots)
    if not keep:
        return False
    return keep.issubset(local_skin_ids(conn, cid))

def purge_folder_like_skins(conn: sqlite3.Connection) -> int:
    """删除 id 不以 {character_id}- 开头的解包脏皮（如 aijier_3）。"""
    rows = conn.execute("SELECT character_id, id FROM skins").fetchall()
    deleted = 0
    for cid, sid in rows:
        cid_s, sid_s = str(cid), str(sid)
        if sid_s == cid_s or sid_s.startswith(cid_s + "-"):
            continue
        conn.execute("DELETE FROM skins WHERE id=?", (sid_s,))
        deleted += 1
    return deleted

def purge_hx_skins(conn: sqlite3.Connection) -> int:
    """Delete harmonized (*_hx) skins and their lines. Returns deleted skin count."""
    rows = conn.execute(
        "SELECT id, kanmusu_dir, name_zh FROM skins"
    ).fetchall()
    deleted = 0
    for row in rows:
        sid = str(row[0] if not isinstance(row, sqlite3.Row) else row["id"])
        kdir = str(
            (row[1] if not isinstance(row, sqlite3.Row) else row["kanmusu_dir"]) or ""
        )
        name = str(
            (row[2] if not isinstance(row, sqlite3.Row) else row["name_zh"]) or ""
        )
        if not is_hx_skin(skin_id=sid, kanmusu_dir=kdir, name_zh=name):
            continue
        conn.execute("DELETE FROM skin_lines WHERE skin_id=?", (sid,))
        conn.execute("DELETE FROM skins WHERE id=?", (sid,))
        deleted += 1
    return deleted

def list_character_ids_needing_lines(conn: sqlite3.Connection) -> list[str]:
    """本地仍有空/未匹配台词的角色（用于增量写入）。"""
    need: list[str] = []
    for (cid,) in conn.execute("SELECT id FROM characters ORDER BY id"):
        skins = conn.execute(
            "SELECT id, meta_json FROM skins WHERE character_id=?",
            (cid,),
        ).fetchall()
        if not skins:
            continue
        for sid, meta_raw in skins:
            n = conn.execute(
                "SELECT count(*) FROM skin_lines WHERE skin_id=?", (sid,)
            ).fetchone()[0]
            status = ""
            try:
                meta = json.loads(meta_raw or "{}")
                status = str((meta.get("lines_import") or {}).get("status") or "")
            except json.JSONDecodeError:
                status = ""
            if n == 0 or status in ("empty", "unmatched", "stale_flat"):
                need.append(str(cid))
                break
    return need

def lines_rows_from_wiki(raw_lines: list) -> list[dict]:
    out = []
    for i, item in enumerate(raw_lines or []):
        if not isinstance(item, dict):
            continue
        text = (item.get("text") or "").strip()
        if not text:
            continue
        key = item.get("key") if isinstance(item.get("key"), str) else ""
        label = item.get("label") if isinstance(item.get("label"), str) else ""
        lang = item.get("lang") if isinstance(item.get("lang"), str) else ""
        audio = item.get("audioUrl") if isinstance(item.get("audioUrl"), str) else ""
        anim = ""
        if key:
            k = key.lower()
            if "touch2" in k or "touch_special" in k:
                anim = "touch_special"
            elif "head" in k:
                anim = "touch_head"
            elif "touch" in k:
                anim = "touch_body"
            elif "idle" in k or "main" in k:
                anim = "idle"
        out.append(
            {
                "wiki_key": key or "",
                "label": label or "",
                "lang": lang or "",
                "text": text,
                "animation": anim,
                "audio_url": audio or "",
                "audio_relpath": "",
                "sort_order": i,
            }
        )
    return out

def fill_english_names(conn: sqlite3.Connection) -> dict:
    cur = conn.execute("SELECT id, name_en FROM characters")
    c_n = 0
    for cid, en in cur.fetchall():
        if not (en or "").strip():
            conn.execute("UPDATE characters SET name_en=? WHERE id=?", (cid, cid))
            c_n += 1
    cur = conn.execute("SELECT id, name_en FROM skins")
    s_n = 0
    for sid, en in cur.fetchall():
        if not (en or "").strip():
            conn.execute("UPDATE skins SET name_en=? WHERE id=?", (sid, sid))
            s_n += 1
    conn.commit()
    return {"characters": c_n, "skins": s_n}

def upsert_character(conn: sqlite3.Connection, row: dict) -> None:
    cid = (row.get("id") or "").strip()
    if is_folder_like_character_id(cid):
        raise ValueError(
            f"folder-like character id rejected (skin suffix): {cid!r}; "
            "use base id (e.g. abeikelongbi) and attach skins separately"
        )
    name_en = normalize_name_en(row.get("name_en") or "", cid)
    conn.execute(
        """
        INSERT INTO characters(
          id, name_zh, name_en, wiki_title, cv, faction, ship_type, rarity,
          persona_id, source, description, meta_json, updated_at
        ) VALUES (?,?,?,?,?,?,?,?,?,?,?,?,datetime('now'))
        ON CONFLICT(id) DO UPDATE SET
          name_zh=excluded.name_zh,
          name_en=CASE WHEN excluded.name_en!='' THEN excluded.name_en ELSE characters.name_en END,
          wiki_title=excluded.wiki_title,
          cv=excluded.cv,
          faction=excluded.faction,
          ship_type=excluded.ship_type,
          rarity=excluded.rarity,
          persona_id=CASE WHEN excluded.persona_id!='' THEN excluded.persona_id ELSE characters.persona_id END,
          source=excluded.source,
          description=CASE WHEN excluded.description!='' THEN excluded.description ELSE characters.description END,
          updated_at=datetime('now')
        """,
        (
            row["id"],
            row["name_zh"],
            name_en,
            row.get("wiki_title") or "",
            row.get("cv") or "",
            row.get("faction") or "",
            row.get("ship_type") or "",
            row.get("rarity") or "",
            row.get("persona_id") or row["id"],
            row.get("source") or "",
            row.get("description") or "",
            row.get("meta_json") or "{}",
        ),
    )

def upsert_skin(conn: sqlite3.Connection, row: dict, replace_lines: bool) -> None:
    name_en = normalize_name_en(row.get("name_en") or "", row["id"])
    conn.execute(
        """
        INSERT INTO skins(
          id, character_id, name_zh, name_en, skin_index, pet_model_id, kanmusu_dir,
          sort_order, is_default, meta_json, updated_at
        ) VALUES (?,?,?,?,?,?,?,?,?,?,datetime('now'))
        ON CONFLICT(id) DO UPDATE SET
          character_id=excluded.character_id,
          name_zh=excluded.name_zh,
          name_en=CASE WHEN excluded.name_en!='' THEN excluded.name_en ELSE skins.name_en END,
          skin_index=excluded.skin_index,
          pet_model_id=CASE WHEN excluded.pet_model_id!='' THEN excluded.pet_model_id ELSE skins.pet_model_id END,
          kanmusu_dir=CASE WHEN excluded.kanmusu_dir!='' THEN excluded.kanmusu_dir ELSE skins.kanmusu_dir END,
          sort_order=excluded.sort_order,
          is_default=excluded.is_default,
          meta_json=excluded.meta_json,
          updated_at=datetime('now')
        """,
        (
            row["id"],
            row["character_id"],
            row["name_zh"],
            name_en,
            row.get("skin_index"),
            row.get("pet_model_id") or "",
            row.get("kanmusu_dir") or "",
            row.get("sort_order") or 0,
            1 if row.get("is_default") else 0,
            row.get("meta_json") or "{}",
        ),
    )
    if replace_lines:
        conn.execute("DELETE FROM skin_lines WHERE skin_id=?", (row["id"],))
        for ln in row.get("lines") or []:
            conn.execute(
                """
                INSERT INTO skin_lines(
                  skin_id, wiki_key, label, lang, text, animation, audio_url, audio_relpath, sort_order
                ) VALUES (?,?,?,?,?,?,?,?,?)
                """,
                (
                    row["id"],
                    ln.get("wiki_key") or "",
                    ln.get("label") or "",
                    ln.get("lang") or "",
                    ln["text"],
                    ln.get("animation") or "",
                    ln.get("audio_url") or "",
                    ln.get("audio_relpath") or "",
                    int(ln.get("sort_order") or 0),
                ),
            )

def cmd_import_bundled_seed(args: argparse.Namespace) -> int:
    """Seed local DB from existing hanpet/bundled characters manifest (builtins)."""
    db = Path(args.db) if args.db else default_local_db()
    conn = connect(db)
    apply_schema(conn)
    manifest = load_json(
        bundled_roster_dir() / "characters" / "manifest.json",
        {"characters": []},
    )
    n_char = n_skin = 0
    for c in manifest.get("characters") or []:
        if not isinstance(c, dict) or not c.get("id"):
            continue
        upsert_character(
            conn,
            {
                "id": c["id"],
                "name_zh": c.get("name") or c["id"],
                "name_en": c.get("english_name") or "",
                "wiki_title": c.get("wiki_title") or "",
                "cv": c.get("cv") or "",
                "faction": c.get("faction") or "",
                "ship_type": c.get("ship_type") or "",
                "rarity": c.get("rarity") or "",
                "persona_id": c.get("persona_id") or c["id"],
                "source": c.get("source") or "bundled",
                "description": c.get("description") or "",
            },
        )
        n_char += 1
        conn.execute(
            "DELETE FROM skin_lines WHERE skin_id IN (SELECT id FROM skins WHERE character_id=?)",
            (c["id"],),
        )
        conn.execute("DELETE FROM skins WHERE character_id=?", (c["id"],))
        for i, s in enumerate(c.get("skins") or []):
            if not isinstance(s, dict) or not s.get("id"):
                continue
            raw_id = str(s["id"])
            db_id = skin_db_id(c["id"], raw_id)
            upsert_skin(
                conn,
                {
                    "id": db_id,
                    "character_id": c["id"],
                    "name_zh": s.get("name") or "默认",
                    "name_en": s.get("english_name") or s.get("name_en") or "",
                    "skin_index": s.get("skin_index"),
                    "pet_model_id": s.get("model_id") or "",
                    "kanmusu_dir": (s.get("kanmusu_dir") or ""),
                    "sort_order": i,
                    "is_default": bool(s.get("default")) or raw_id == "default",
                    "lines": [],
                },
                replace_lines=False,
            )
            n_skin += 1
    conn.commit()
    conn.close()
    emit({"ok": True, "db": str(db), "characters": n_char, "skins": n_skin})
    return 0

# --- merge ---
def purge_folder_like_characters(
    conn: sqlite3.Connection, alias_map: dict[str, str] | None = None
) -> int:
    """Delete character rows whose ids are skin folders; merge into base first.

    Example: abeikelongbi_3 → merge/delete into abeikelongbi.
    Returns number of folder-like character rows removed.
    """
    alias_map = alias_map or LIVE2D_ALIASES
    donors = [
        str(r[0])
        for r in conn.execute("SELECT id FROM characters ORDER BY id")
        if is_folder_like_character_id(str(r[0]))
    ]
    removed = 0
    for donor in donors:
        base, _suffix = strip_skin(donor)
        base = alias_redirect_id(base, alias_map)
        if not base or base == donor:
            conn.execute("DELETE FROM skin_lines WHERE skin_id IN (SELECT id FROM skins WHERE character_id=?)", (donor,))
            conn.execute("DELETE FROM skins WHERE character_id=?", (donor,))
            conn.execute("DELETE FROM characters WHERE id=?", (donor,))
            removed += 1
            continue
        if not conn.execute("SELECT 1 FROM characters WHERE id=?", (base,)).fetchone():
            cn = (alias_map.get(base) or "").strip() or base
            upsert_character(
                conn,
                {
                    "id": base,
                    "name_zh": cn,
                    "name_en": "",
                    "wiki_title": cn,
                    "persona_id": base,
                    "source": "unpacked",
                },
            )
        _merge_character_into(conn, donor, base)
        _repoint_avatar_file(donor, base)
        removed += 1
    return removed

def _remap_skin_id(old_id: str, donor_cid: str, canon_cid: str) -> str:
    if old_id == donor_cid:
        return canon_cid
    prefix = donor_cid + "-"
    if old_id.startswith(prefix):
        return canon_cid + "-" + old_id[len(prefix) :]
    return f"{canon_cid}-{old_id}"

def _merge_character_into(
    conn: sqlite3.Connection, donor_cid: str, canon_cid: str
) -> None:
    """Move skins/lines from donor → canon, then delete donor character."""
    if donor_cid == canon_cid:
        return
    donor_skins = conn.execute(
        "SELECT * FROM skins WHERE character_id=?", (donor_cid,)
    ).fetchall()
    for sk in donor_skins:
        old_id = str(sk["id"])
        new_id = _remap_skin_id(old_id, donor_cid, canon_cid)
        existing = conn.execute(
            "SELECT * FROM skins WHERE id=?", (new_id,)
        ).fetchone()
        if existing:
            # fill empty bind fields on canon
            pet = (existing["pet_model_id"] or "") or (sk["pet_model_id"] or "")
            kdir = (existing["kanmusu_dir"] or "") or (sk["kanmusu_dir"] or "")
            conn.execute(
                "UPDATE skins SET pet_model_id=?, kanmusu_dir=?, updated_at=datetime('now') WHERE id=?",
                (pet, kdir, new_id),
            )
            tgt_n = conn.execute(
                "SELECT count(*) FROM skin_lines WHERE skin_id=?", (new_id,)
            ).fetchone()[0]
            if tgt_n == 0:
                conn.execute(
                    "UPDATE skin_lines SET skin_id=? WHERE skin_id=?",
                    (new_id, old_id),
                )
            else:
                conn.execute("DELETE FROM skin_lines WHERE skin_id=?", (old_id,))
            conn.execute("DELETE FROM skins WHERE id=?", (old_id,))
        else:
            # rename via insert+line move+delete (PK change)
            cols = [
                str(d[1])
                for d in conn.execute("PRAGMA table_info(skins)").fetchall()
            ]
            row = {k: sk[k] for k in sk.keys()}
            row["id"] = new_id
            row["character_id"] = canon_cid
            placeholders = ",".join("?" for _ in cols)
            conn.execute(
                f"INSERT INTO skins({','.join(cols)}) VALUES ({placeholders})",
                tuple(row.get(c) for c in cols),
            )
            conn.execute(
                "UPDATE skin_lines SET skin_id=? WHERE skin_id=?",
                (new_id, old_id),
            )
            conn.execute("DELETE FROM skins WHERE id=?", (old_id,))
    # any leftover
    conn.execute("DELETE FROM skins WHERE character_id=?", (donor_cid,))
    conn.execute("DELETE FROM characters WHERE id=?", (donor_cid,))

def repair_uppercased_hash_character_ids(conn: sqlite3.Connection) -> int:
    """Undo mistaken uppercase of BWIKI hash ids (P00e01b29 → p00e01b29)."""
    changed = 0
    ids = [str(r[0]) for r in conn.execute("SELECT id FROM characters").fetchall()]
    for cid in ids:
        if not conn.execute("SELECT 1 FROM characters WHERE id=?", (cid,)).fetchone():
            continue
        if not HASH_PERSONA_ID.match(cid):
            continue
        canon = cid.lower()
        if canon == cid:
            continue
        if conn.execute("SELECT 1 FROM characters WHERE id=?", (canon,)).fetchone():
            _merge_character_into(conn, cid, canon)
            _repoint_avatar_file(cid, canon)
        else:
            row = conn.execute("SELECT * FROM characters WHERE id=?", (cid,)).fetchone()
            if not row:
                continue
            payload = {k: row[k] for k in row.keys()}
            payload["id"] = canon
            if str(payload.get("persona_id") or "").casefold() == cid.casefold():
                payload["persona_id"] = canon
            upsert_character(conn, payload)
            _merge_character_into(conn, cid, canon)
            _repoint_avatar_file(cid, canon)
        changed += 1
    return changed

def canonicalize_ship_code_character_ids(conn: sqlite3.Connection) -> int:
    """Rename z46 → Z46 (etc.) and merge into existing uppercase row if present."""
    changed = 0
    ids = [str(r[0]) for r in conn.execute("SELECT id FROM characters").fetchall()]
    for cid in ids:
        if not conn.execute("SELECT 1 FROM characters WHERE id=?", (cid,)).fetchone():
            continue
        if not is_ship_code_id(cid):
            continue
        canon = normalize_character_id(cid)
        if canon == cid:
            row = conn.execute(
                "SELECT name_zh, wiki_title FROM characters WHERE id=?", (cid,)
            ).fetchone()
            if not row:
                continue
            nz = (row["name_zh"] or "").strip()
            wt = (row["wiki_title"] or "").strip()
            new_nz = normalize_character_id(nz) if is_ship_code_id(nz) else nz
            new_wt = normalize_character_id(wt) if is_ship_code_id(wt) else wt
            if new_nz != nz or new_wt != wt:
                conn.execute(
                    "UPDATE characters SET name_zh=?, wiki_title=?, updated_at=datetime('now') WHERE id=?",
                    (new_nz, new_wt, cid),
                )
                changed += 1
            continue
        if conn.execute("SELECT 1 FROM characters WHERE id=?", (canon,)).fetchone():
            _merge_character_into(conn, cid, canon)
            _repoint_avatar_file(cid, canon)
        else:
            row = conn.execute("SELECT * FROM characters WHERE id=?", (cid,)).fetchone()
            if not row:
                continue
            payload = {k: row[k] for k in row.keys()}
            payload["id"] = canon
            payload["persona_id"] = canon
            nz = str(payload.get("name_zh") or "").strip()
            wt = str(payload.get("wiki_title") or "").strip()
            if is_ship_code_id(nz) or nz.casefold() == canon.casefold():
                payload["name_zh"] = (
                    canon
                    if nz.casefold() == canon.casefold()
                    else normalize_character_id(nz)
                )
            if is_ship_code_id(wt) or wt.casefold() == canon.casefold():
                payload["wiki_title"] = (
                    canon
                    if wt.casefold() == canon.casefold()
                    else normalize_character_id(wt)
                )
            upsert_character(conn, payload)
            _merge_character_into(conn, cid, canon)
            _repoint_avatar_file(cid, canon)
        changed += 1
    return changed

def merge_roster_duplicates_by_name(
    conn: sqlite3.Connection, alias_map: dict[str, str] | None = None
) -> int:
    """合并同中文名多角色行（如 aijiang+aijier）。返回合并掉的 donor 数。

    名称按 casefold 归组，避免 wiki stub `z46` 与正式 `Z46` 双开。
    随后将舰船代号 id 规范为大写（z46 → Z46）。
    """
    from collections import defaultdict

    alias_map = alias_map or LIVE2D_ALIASES
    by_name: dict[str, list[str]] = defaultdict(list)
    name_display: dict[str, str] = {}
    source_by_id: dict[str, str] = {}

    def _merge_name_key(name: str, cid: str) -> tuple[str, str]:
        """Group key + display: ASCII stubs via alias (i404 → 伊404)."""
        raw = (name or "").strip()
        if not raw:
            return "", ""
        # Already has a real CJK display name — trust it
        if CJK_RE.search(raw):
            return raw.casefold(), raw
        # Stub still using folder slug / hull-looking ASCII as name_zh
        for cand in (raw, cid):
            cn = _alias_cn_for_slug(cand, alias_map)
            if cn and CJK_RE.search(cn):
                return cn.casefold(), cn
        return raw.casefold(), raw

    for row in conn.execute("SELECT id, name_zh, source FROM characters"):
        name = (row["name_zh"] or "").strip()
        cid = str(row["id"])
        source_by_id[cid] = (row["source"] or "").strip()
        if not name:
            continue
        key, display = _merge_name_key(name, cid)
        if not key:
            continue
        by_name[key].append(cid)
        prev = name_display.get(key)
        if prev is None or (prev.isascii() and not display.isascii()) or (
            prev.islower() and display != display.lower()
        ):
            name_display[key] = display
    merged = 0
    for key, ids in by_name.items():
        if len(ids) < 2:
            continue
        name = name_display.get(key) or key

        def _skin_count(cid: str) -> int:
            return int(
                conn.execute(
                    "SELECT count(*) FROM skins WHERE character_id=?", (cid,)
                ).fetchone()[0]
            )

        # Wiki rows win over unpacked folder stubs (i404 stub vs 伊404 wiki hash)
        wiki_ids = [i for i in ids if source_by_id.get(i) == "wiki"]
        if wiki_ids:
            canon = max(wiki_ids, key=_skin_count)
        else:
            pref = preferred_slug_for_cn(name, alias_map)
            if pref and pref not in ids:
                for i in ids:
                    if i.casefold() == pref.casefold():
                        pref = i
                        break
                else:
                    pref = None
            if not pref:
                for i in ids:
                    if i.casefold() == key and not HASH_PERSONA_ID.match(i):
                        pref = i
                        break
            non_hash = [i for i in ids if not HASH_PERSONA_ID.match(i)]
            if pref and pref in ids:
                canon = pref
            elif non_hash:
                # Prefer uppercase ship-code id when present
                ship_ids = [i for i in non_hash if is_ship_code_id(i)]
                pool = ship_ids or non_hash
                canon = max(
                    pool,
                    key=lambda i: (i == normalize_character_id(i), _skin_count(i)),
                )
            else:
                canon = max(ids, key=_skin_count)
        for donor in ids:
            if donor == canon:
                continue
            _merge_character_into(conn, donor, canon)
            _repoint_avatar_file(donor, canon)
            merged += 1
        disp = name_display.get(key) or name
        if is_ship_code_id(disp):
            disp = normalize_character_id(disp)
        if disp and disp != key:
            conn.execute(
                "UPDATE characters SET name_zh=?, wiki_title=COALESCE(NULLIF(wiki_title,''), ?) "
                "WHERE id=?",
                (disp, disp, canon),
            )
    merged += repair_uppercased_hash_character_ids(conn)
    merged += canonicalize_ship_code_character_ids(conn)
    return merged

def enrich_unpacked_character_names(
    conn: sqlite3.Connection,
    wiki_db: Path | None = None,
    alias_map: dict[str, str] | None = None,
) -> int:
    """Fill Chinese name/wiki_title for stubs where name_zh == id (pinyin folder).

    Uses catalog pinyin slug index + live2d aliases (kubo→可怖 when pinyin is kebu).
    Returns number of characters updated.
    """
    from roster.avatar_fetch import default_wiki_db, resolve_catalog_by_slug

    wiki_db = Path(wiki_db) if wiki_db else default_wiki_db()
    if not wiki_db.is_file():
        return 0
    alias_map = alias_map or {
        **LIVE2D_ALIASES,
        **load_json(repo_root() / "data/wiki/live2d-aliases.json", {}),
    }
    updated = 0
    rows = conn.execute(
        "SELECT id, name_zh, wiki_title, name_en FROM characters ORDER BY id"
    ).fetchall()
    for row in rows:
        cid = str(row["id"])
        name_zh = (row["name_zh"] or "").strip()
        wiki_title = (row["wiki_title"] or "").strip()
        name_en = (row["name_en"] or "").strip()
        # Only touch obvious stubs (name still equals slug / empty CN)
        if name_zh and name_zh != cid and not name_zh.isascii():
            continue
        if wiki_title and wiki_title != cid and not wiki_title.isascii():
            continue
        base, _ = strip_skin(cid)
        # Prefer explicit live2d aliases (i404→伊404; pinyin of 伊 is yi404)
        aliased = (
            _alias_cn_for_slug(cid, alias_map)
            or _alias_cn_for_slug(base, alias_map)
            or _alias_cn_for_slug(name_zh, alias_map)
        )
        hit = resolve_catalog_by_slug(wiki_db, cid) or resolve_catalog_by_slug(
            wiki_db, base
        )
        if aliased and CJK_RE.search(aliased):
            display = aliased
            title = aliased
        elif hit:
            display = (hit.get("display_name") or hit.get("wiki_title") or "").strip()
            title = (hit.get("wiki_title") or display).strip()
        else:
            continue
        if not display:
            continue
        # Keep English code as name_en when stub was folder code (kubo)
        new_en = name_en
        if (not new_en or new_en == cid or HASH_PERSONA_ID.match(new_en)) and cid.isascii():
            if not HASH_PERSONA_ID.match(cid):
                new_en = cid
        conn.execute(
            """
            UPDATE characters SET
              name_zh=?,
              wiki_title=?,
              name_en=?,
              source=CASE WHEN source='unpacked' THEN 'wiki' ELSE source END,
              updated_at=datetime('now')
            WHERE id=?
            """,
            (display, title, new_en, cid),
        )
        updated += 1
    return updated

def _repoint_avatar_file(donor_cid: str, canon_cid: str) -> None:
    """If donor has an avatar file and canon does not, rename/copy to canon id."""
    if donor_cid == canon_cid:
        return
    try:
        from roster.avatar_fetch import (
            avatars_dir,
            invalidate_avatar_index,
            resolve_avatar_file,
        )
    except ImportError:
        return
    if resolve_avatar_file(canon_cid):
        return
    src = resolve_avatar_file(donor_cid)
    if not src:
        return
    dest = avatars_dir() / f"{canon_cid}{src.suffix}"
    try:
        if not dest.exists():
            src.replace(dest)
        else:
            src.unlink(missing_ok=True)
        invalidate_avatar_index()
    except OSError:
        try:
            import shutil as _shutil

            _shutil.copy2(src, dest)
            invalidate_avatar_index()
        except OSError as exc:
            logging.warning(
                "avatar repoint failed donor=%s canon=%s: %s",
                donor_cid,
                canon_cid,
                exc,
            )

# --- bind ---
def _build_cn_to_slug(
    conn: sqlite3.Connection,
    wiki: sqlite3.Connection,
    alias_map: dict[str, str],
) -> dict[str, str]:
    """中文名 → 权威拼音 / 已有 id。"""
    rev: dict[str, str] = {}
    # 先按中文聚合别名，写入权威 slug（避免 aijiang/aijier 谁先谁后）
    seen_cn: set[str] = set()
    for _slug, cn in alias_map.items():
        cn = (cn or "").strip()
        if not cn or cn in seen_cn:
            continue
        seen_cn.add(cn)
        pref = preferred_slug_for_cn(cn, alias_map)
        if pref:
            rev[cn] = pref
    try:
        for row in wiki.execute(
            "SELECT folder, wiki_title, display_name FROM live2d_mappings"
        ):
            base, _ = strip_skin(row["folder"] or "")
            base = alias_redirect_id(base, alias_map)
            for key in ((row["display_name"] or "").strip(), (row["wiki_title"] or "").strip()):
                if key and base and key not in rev:
                    rev[key] = base
    except sqlite3.Error:
        pass
    for row in conn.execute("SELECT id, name_zh, wiki_title, source FROM characters"):
        cid = alias_redirect_id(str(row["id"] or ""), alias_map)
        src = (row["source"] or "").strip()
        for key in ((row["name_zh"] or "").strip(), (row["wiki_title"] or "").strip()):
            if not key or not cid:
                continue
            existing = rev.get(key)
            if not existing:
                rev[key] = cid
                continue
            # Wiki / hash persona ids beat pinyin alias stubs (伊404 → p85… not i404)
            if src == "wiki" and HASH_PERSONA_ID.match(cid) and not HASH_PERSONA_ID.match(
                existing
            ):
                rev[key] = cid
    return rev

def _resolve_character_id(cn: str, cn_to_slug: dict[str, str]) -> str:
    cn = (cn or "").strip()
    if not cn:
        return _stable_wiki_id("unknown")
    # Hull codes always use uppercase id — never hash (z46 / Z46 → Z46)
    if is_ship_code_id(cn):
        return normalize_character_id(cn)
    if cn in cn_to_slug:
        return alias_redirect_id(cn_to_slug[cn])
    for k, v in cn_to_slug.items():
        if k.casefold() == cn.casefold():
            return alias_redirect_id(v)
    return _stable_wiki_id(cn)

def resolve_bind_skin_id(
    conn: sqlite3.Connection, cid: str, suffix: str
) -> str | None:
    """Map unpacked folder suffix to an existing roster skin id (no create).

    Azur Lane folder convention (common):
      slug       → default
      slug_2     → first alternate (Wiki skin1)
      slug_3     → second alternate (Wiki skin2)
    So digit N>=2 prefers Wiki skin{N-1} before skin{N}.
    """
    if not suffix or suffix == "0":
        row = conn.execute(
            "SELECT id FROM skins WHERE character_id=? AND is_default=1 LIMIT 1",
            (cid,),
        ).fetchone()
        if row:
            return str(row[0])
        sid = skin_db_id(cid, "default")
        exists = conn.execute("SELECT 1 FROM skins WHERE id=?", (sid,)).fetchone()
        return sid if exists else None

    # DOA collab: slug_doa → default; slug_2_doa → same digit mapping as slug_2
    if suffix.lower() == "doa":
        return resolve_bind_skin_id(conn, cid, "")
    m_doa = re.fullmatch(r"(\d+)_doa", suffix, re.I)
    if m_doa:
        return resolve_bind_skin_id(conn, cid, m_doa.group(1))

    # μ兵装 / 小舰娘: slug_idol / slug_younv → that variant's default skin
    if suffix.lower() in ("idol", "younv"):
        return resolve_bind_skin_id(conn, cid, "")
    m_var = re.fullmatch(r"(\d+)_(idol|younv)", suffix, re.I)
    if m_var:
        return resolve_bind_skin_id(conn, cid, m_var.group(1))

    # 誓约：桌宠文件夹 ``{slug}_h`` → ``{cid}-oath``（无舰娘）
    if suffix.lower() == "h":
        oath_sid = f"{cid}-oath"
        if conn.execute("SELECT 1 FROM skins WHERE id=?", (oath_sid,)).fetchone():
            return oath_sid
        row = conn.execute(
            "SELECT id FROM skins WHERE character_id=? AND id LIKE '%-oath' LIMIT 1",
            (cid,),
        ).fetchone()
        return str(row[0]) if row else None

    # Pure digit → Wiki skin slots. Compound (3_1 / 3_hx) intentionally
    # does not fall back to the parent skin — those get dedicated L2D rows.
    #
    # BLHX folders: bare ``slug`` = default (原皮也算皮肤); first costume is
    # ``slug_2`` → Wiki skin1. There is no ``slug_1``.
    if suffix.isdigit():
        n = int(suffix)
        if n == 1:
            return None
        # Prefer BLHX _N → skin{N-1}; fall back to skin{N} / skin_index.
        wiki_nums: list[int] = []
        if n >= 2:
            wiki_nums.append(n - 1)
        wiki_nums.append(n)
        seen: set[int] = set()
        ordered: list[int] = []
        for w in wiki_nums:
            if w not in seen and w >= 1:
                seen.add(w)
                ordered.append(w)
        for w in ordered:
            sid = f"{cid}-skin{w}"
            if conn.execute("SELECT 1 FROM skins WHERE id=?", (sid,)).fetchone():
                return sid
        for w in ordered:
            row = conn.execute(
                "SELECT id FROM skins WHERE character_id=? AND skin_index=? LIMIT 1",
                (cid, w),
            ).fetchone()
            if row:
                return str(row[0])
        return None

    # non-digit / compound: match existing bind by folder-shaped kanmusu_dir or id
    row = conn.execute(
        "SELECT id FROM skins WHERE character_id=? AND "
        "(kanmusu_dir=? OR id=? OR name_zh LIKE ?) LIMIT 1",
        (cid, f"{cid}_{suffix}", f"{cid}-{suffix}", f"%{suffix}%"),
    ).fetchone()
    return str(row[0]) if row else None

def reclaim_l2d_orphan_into_skin(
    conn: sqlite3.Connection,
    cid: str,
    target_sid: str,
    *,
    folder: str,
    suffix: str,
) -> bool:
    """Merge a leftover {cid}-L2D-{suffix} row into the Wiki/target skin.

    Typical bug residue: lines on skin1, assets on L2D-2. After reclaim, assets
    (and any orphan-only lines) live on target; orphan row is deleted.
    Returns True if an orphan was removed.
    """
    if not suffix:
        orphan_id = f"{cid}-L2D-default"
    else:
        orphan_id = f"{cid}-L2D-{suffix}"
    orphan_id = re.sub(r"[^a-zA-Z0-9._-]+", "-", orphan_id)
    if orphan_id == target_sid:
        return False
    row = conn.execute(
        "SELECT id, kanmusu_dir, pet_model_id, meta_json FROM skins WHERE id=?",
        (orphan_id,),
    ).fetchone()
    if not row:
        # Same folder bound on a differently named L2D leftover
        row = conn.execute(
            "SELECT id, kanmusu_dir, pet_model_id, meta_json FROM skins "
            "WHERE character_id=? AND id LIKE ? AND id != ? AND kanmusu_dir=? LIMIT 1",
            (cid, f"{cid}-L2D-%", target_sid, folder),
        ).fetchone()
    if not row:
        return False
    orphan_id = str(row[0])
    if orphan_id == target_sid:
        return False

    o_kanmusu = (row[1] or "").strip()
    o_pet = (row[2] or "").strip()
    tgt = conn.execute(
        "SELECT kanmusu_dir, pet_model_id, meta_json FROM skins WHERE id=?",
        (target_sid,),
    ).fetchone()
    if not tgt:
        return False
    t_kanmusu = (tgt[0] or "").strip()
    t_pet = (tgt[1] or "").strip()
    # 誓约：只合桌宠，不写舰娘目录
    if is_oath_skin_id(target_sid):
        new_kanmusu = ""
        new_pet = t_pet or o_pet
    else:
        new_kanmusu = t_kanmusu or o_kanmusu or folder
        new_pet = t_pet or o_pet

    # Move lines if target empty and orphan has some
    t_n = conn.execute(
        "SELECT COUNT(*) FROM skin_lines WHERE skin_id=?", (target_sid,)
    ).fetchone()[0]
    o_n = conn.execute(
        "SELECT COUNT(*) FROM skin_lines WHERE skin_id=?", (orphan_id,)
    ).fetchone()[0]
    if t_n == 0 and o_n > 0:
        conn.execute(
            "UPDATE skin_lines SET skin_id=? WHERE skin_id=?",
            (target_sid, orphan_id),
        )

    # Merge meta: keep target lines_import; copy slot_key if missing
    try:
        t_meta = json.loads(tgt[2] or "{}")
        if not isinstance(t_meta, dict):
            t_meta = {}
    except json.JSONDecodeError:
        t_meta = {}
    try:
        o_meta = json.loads(row[3] or "{}")
        if not isinstance(o_meta, dict):
            o_meta = {}
    except json.JSONDecodeError:
        o_meta = {}
    if "slot_key" not in t_meta and o_meta.get("slot_key"):
        t_meta["slot_key"] = o_meta["slot_key"]
    t_meta["reclaimed_from"] = orphan_id

    conn.execute(
        """
        UPDATE skins SET
          kanmusu_dir=?,
          pet_model_id=?,
          meta_json=?,
          updated_at=datetime('now')
        WHERE id=?
        """,
        (
            new_kanmusu,
            new_pet,
            json.dumps(t_meta, ensure_ascii=False),
            target_sid,
        ),
    )
    conn.execute("DELETE FROM skins WHERE id=?", (orphan_id,))
    return True

def _character_has_wiki_skins(conn: sqlite3.Connection, cid: str) -> bool:
    """True if character already has canonical Wiki slots (not only L2D orphans)."""
    row = conn.execute(
        """
        SELECT 1 FROM skins
        WHERE character_id=? AND id NOT LIKE '%-L2D-%'
        LIMIT 1
        """,
        (cid,),
    ).fetchone()
    return row is not None


def prune_unmapped_l2d_orphan_skins(conn: sqlite3.Connection) -> int:
    """Delete L2D-* rows that cannot map onto any existing Wiki skin slot.

    Extra game folders (e.g. i404_3 when Wiki only has default+skin1) must not
    remain as a third empty/orphan skin on a Wiki character.
    """
    deleted = 0
    rows = conn.execute(
        "SELECT id, character_id FROM skins WHERE id LIKE '%-L2D-%'"
    ).fetchall()
    pat = re.compile(r"^(.+)-L2D-(.+)$")
    for sid, cid in rows:
        sid_s = str(sid)
        cid_s = str(cid)
        if not _character_has_wiki_skins(conn, cid_s):
            continue
        m = pat.match(sid_s)
        if not m:
            continue
        suffix = m.group(2)
        bind_suffix = "" if suffix == "default" else suffix
        target = resolve_bind_skin_id(conn, cid_s, bind_suffix)
        if target and target != sid_s and "-L2D-" not in target:
            # Reclaimable onto a Wiki slot — leave for reclaim pass
            continue
        conn.execute("DELETE FROM skin_lines WHERE skin_id=?", (sid_s,))
        conn.execute("DELETE FROM skins WHERE id=?", (sid_s,))
        deleted += 1
    return deleted


def repair_l2d_folder_orphans(conn: sqlite3.Connection) -> dict[str, int]:
    """Merge leftover {cid}-L2D-{N} rows into Wiki skin{N-1} (BLHX folder map).

    Folder slug_N (N>=2) ↔ Wiki skin{N-1}. Historical binds created L2D-N while
    lines stayed on empty skin{N-1}; this heals the whole roster in one pass.
    """
    stats = {"scanned": 0, "reclaimed": 0, "skipped": 0, "pruned": 0}
    rows = conn.execute(
        "SELECT id, character_id, kanmusu_dir, pet_model_id FROM skins "
        "WHERE id LIKE '%-L2D-%'"
    ).fetchall()
    pat = re.compile(r"^(.+)-L2D-(\d+)$")
    for sid, cid, kanmusu, pet in rows:
        stats["scanned"] += 1
        m = pat.match(str(sid))
        if not m:
            stats["skipped"] += 1
            continue
        cid_s = str(m.group(1))
        n = int(m.group(2))
        if n < 2:
            stats["skipped"] += 1
            continue
        wiki_sid = f"{cid_s}-skin{n - 1}"
        wiki = conn.execute(
            "SELECT id, kanmusu_dir, pet_model_id FROM skins WHERE id=?",
            (wiki_sid,),
        ).fetchone()
        if not wiki:
            stats["skipped"] += 1
            continue
        o_kanmusu = (kanmusu or "").strip()
        o_pet = (pet or "").strip()
        w_kanmusu = (wiki[1] or "").strip()
        w_pet = (wiki[2] or "").strip()
        orphan_has = bool(o_kanmusu or o_pet)
        wiki_empty = not w_kanmusu and not w_pet
        same_folder = bool(o_kanmusu) and o_kanmusu == w_kanmusu
        if not orphan_has:
            stats["skipped"] += 1
            continue
        if not (wiki_empty or same_folder or not w_kanmusu):
            # Wiki already bound to a different folder — leave alone
            stats["skipped"] += 1
            continue
        folder = o_kanmusu or f"{cid_s}_{n}"
        if reclaim_l2d_orphan_into_skin(
            conn, cid_s, wiki_sid, folder=folder, suffix=str(n)
        ):
            # Ensure folder lands on wiki skin even if reclaim kept prior empty
            if o_kanmusu:
                conn.execute(
                    "UPDATE skins SET kanmusu_dir=?, "
                    "pet_model_id=CASE WHEN ?!='' THEN ? ELSE pet_model_id END, "
                    "updated_at=datetime('now') WHERE id=?",
                    (o_kanmusu, o_pet, o_pet, wiki_sid),
                )
            stats["reclaimed"] += 1
        else:
            stats["skipped"] += 1
    stats["pruned"] = prune_unmapped_l2d_orphan_skins(conn)
    return stats

def repair_misindexed_wiki_folder_binds(conn: sqlite3.Connection) -> dict[str, int]:
    """Move assets when Wiki skinK was wrongly bound to folder _{K} instead of _{K+1}.

    Old resolve used skin{N} for folder _N, so with both skin1+skin2 present,
    slug_2 landed on skin2 while skin1 (lines) stayed empty. Move assets to
    skin{N-1}; do not move lines (they already sit on the correct Wiki slot).
    """
    stats = {"scanned": 0, "moved": 0, "skipped": 0}
    skin_pat = re.compile(r"^(.+)-skin(\d+)$")
    folder_pat = re.compile(r"_(\d+)$")
    rows = conn.execute(
        "SELECT id, character_id, kanmusu_dir, pet_model_id FROM skins "
        "WHERE id LIKE '%-skin%' AND IFNULL(kanmusu_dir,'') != ''"
    ).fetchall()
    moves: list[tuple[int, str, str, str, str]] = []
    for sid, cid, kanmusu, pet in rows:
        stats["scanned"] += 1
        sm = skin_pat.match(str(sid))
        if not sm:
            stats["skipped"] += 1
            continue
        k = int(sm.group(2))
        kd = (kanmusu or "").strip()
        fm = folder_pat.search(kd)
        if not fm:
            stats["skipped"] += 1
            continue
        n = int(fm.group(1))
        # Correct BLHX map: folder _N → skin{N-1}. Wrong classic bug: skinK has _K.
        if n != k:
            stats["skipped"] += 1
            continue
        if n < 2:
            stats["skipped"] += 1
            continue
        target = f"{sm.group(1)}-skin{n - 1}"
        if target == sid:
            stats["skipped"] += 1
            continue
        tgt = conn.execute(
            "SELECT kanmusu_dir, pet_model_id FROM skins WHERE id=?", (target,)
        ).fetchone()
        if not tgt:
            stats["skipped"] += 1
            continue
        if (tgt[0] or "").strip() or (tgt[1] or "").strip():
            stats["skipped"] += 1
            continue
        moves.append((n, str(sid), target, kd, (pet or "").strip()))

    for _n, src, target, kd, pet in sorted(moves, key=lambda x: x[0]):
        tgt = conn.execute(
            "SELECT kanmusu_dir, pet_model_id FROM skins WHERE id=?", (target,)
        ).fetchone()
        if not tgt or (tgt[0] or "").strip() or (tgt[1] or "").strip():
            stats["skipped"] += 1
            continue
        conn.execute(
            """
            UPDATE skins SET
              kanmusu_dir=?,
              pet_model_id=CASE WHEN ?!='' THEN ? ELSE pet_model_id END,
              updated_at=datetime('now')
            WHERE id=?
            """,
            (kd, pet, pet, target),
        )
        conn.execute(
            """
            UPDATE skins SET
              kanmusu_dir='',
              pet_model_id='',
              updated_at=datetime('now')
            WHERE id=?
            """,
            (src,),
        )
        stats["moved"] += 1
    return stats

def repair_blhx_skin_folder_binds(conn: sqlite3.Connection) -> dict[str, int]:
    """Full heal: fix misindexed Wiki binds, then reclaim L2D orphans.

    Runs until a pass makes no further changes (chains like skin3←_3 then
    L2D-4→skin3 need multiple passes).
    """
    total = {
        "passes": 0,
        "misindexed_scanned": 0,
        "misindexed_moved": 0,
        "misindexed_skipped": 0,
        "l2d_scanned": 0,
        "l2d_reclaimed": 0,
        "l2d_skipped": 0,
        "l2d_pruned": 0,
    }
    for _ in range(12):
        a = repair_misindexed_wiki_folder_binds(conn)
        b = repair_l2d_folder_orphans(conn)
        total["passes"] += 1
        total["misindexed_scanned"] = a["scanned"]
        total["misindexed_moved"] += a["moved"]
        total["misindexed_skipped"] = a["skipped"]
        total["l2d_scanned"] = b["scanned"]
        total["l2d_reclaimed"] += b["reclaimed"]
        total["l2d_skipped"] = b["skipped"]
        total["l2d_pruned"] += int(b.get("pruned") or 0)
        if not a["moved"] and not b["reclaimed"] and not b.get("pruned"):
            break
    return total

def ensure_skin_for_unpacked(
    conn: sqlite3.Connection,
    cid: str,
    folder: str,
    suffix: str,
) -> str:
    """Create a roster skin row for an unpacked folder when Wiki has no match."""
    from common.unpack_complete import is_hx_slug

    if is_hx_slug(folder) or (suffix or "").lower() == "hx" or (suffix or "").lower().endswith(
        "_hx"
    ):
        raise ValueError(f"refusing hx skin folder: {folder!r}")
    sid = f"{cid}-L2D-{suffix}" if suffix else f"{cid}-L2D-default"
    # Keep id filesystem-safe / stable
    sid = re.sub(r"[^a-zA-Z0-9._-]+", "-", sid)
    exists = conn.execute("SELECT 1 FROM skins WHERE id=?", (sid,)).fetchone()
    skin_index = None
    primary = suffix.split("_", 1)[0] if suffix else ""
    if primary.isdigit():
        skin_index = int(primary)
    name_zh = skin_label(suffix) if suffix else "默认"
    if suffix and not suffix.isdigit() and "_" in suffix:
        name_zh = f"皮肤{suffix}"
    if exists:
        conn.execute(
            """
            UPDATE skins SET
              kanmusu_dir=?,
              name_zh=COALESCE(NULLIF(name_zh,''), ?),
              updated_at=datetime('now')
            WHERE id=?
            """,
            (folder, name_zh, sid),
        )
        return sid
    upsert_skin(
        conn,
        {
            "id": sid,
            "character_id": cid,
            "name_zh": name_zh,
            "name_en": "",
            "is_default": 1 if not suffix else 0,
            "skin_index": skin_index,
            "kanmusu_dir": folder,
            "source": "unpacked",
            "lines": [],
        },
        replace_lines=False,
    )
    return sid

def _bind_paths_for_folder(
    unpacked_dir: Path, pet_models: Path, folder: str
) -> tuple[str, str]:
    """Return (kanmusu_dir, pet_model_id) for a non-oath unpacked folder.

    Cubism folders remain 舰娘; optional Spine mirror under live2d is 桌宠.
    Spine-only unpacked (no Cubism): still 舰娘; if the same slug exists under
    live2d with Spine assets, also bind 桌宠 so the UI shows local skin files.
    """
    from roster.skin_probe import _has_cubism_assets, _has_spine_assets

    kanmusu_dir = folder
    pet_model_id = ""
    has_cubism = _has_cubism_assets(unpacked_dir)
    has_spine = _has_spine_assets(unpacked_dir)

    def _pet_mirror() -> str:
        if (pet_models / folder).is_dir() and _has_spine_assets(pet_models / folder):
            return folder
        if (pet_models / f"skin-{folder}").is_dir() and _has_spine_assets(
            pet_models / f"skin-{folder}"
        ):
            return f"skin-{folder}"
        return ""

    if has_cubism:
        pet_model_id = _pet_mirror()
    elif has_spine:
        # Spine-only painting: kanmusu = unpacked; pet = live2d mirror when present
        pet_model_id = _pet_mirror()
    else:
        # Unknown / incomplete folder: keep prior dual-bind when live2d mirror exists
        if (pet_models / folder).is_dir():
            pet_model_id = folder
        elif (pet_models / f"skin-{folder}").is_dir():
            pet_model_id = f"skin-{folder}"
    return kanmusu_dir, pet_model_id

def _lookup_character_id_by_cn(
    conn: sqlite3.Connection,
    cn: str,
    *,
    cn_to_slug: dict[str, str],
) -> str | None:
    row = conn.execute(
        """
        SELECT id FROM characters
        WHERE name_zh=? OR wiki_title=?
        ORDER BY CASE
          WHEN source='wiki' THEN 0
          WHEN id LIKE 'p%' THEN 1
          ELSE 2
        END
        LIMIT 1
        """,
        (cn, cn),
    ).fetchone()
    if row:
        return str(row[0])
    return cn_to_slug.get(cn) or _resolve_character_id(cn, cn_to_slug)


def _resolve_character_id_for_folder(
    conn: sqlite3.Connection,
    base: str,
    *,
    alias_map: dict[str, str],
    cn_to_slug: dict[str, str],
    suffix: str = "",
) -> str | None:
    """Map unpack folder base(+suffix) → roster character id.

    ``*_younv`` → 小XX；``*_idol`` → XX(μ兵装)；勿绑回成人设。
    """
    cn = (alias_map.get(base) or alias_map.get(base.lower()) or "").strip() or None
    cid: str | None = None
    if cn:
        variant_cn = variant_character_name_for_suffix(cn, suffix)
        if variant_cn:
            cid = _lookup_character_id_by_cn(conn, variant_cn, cn_to_slug=cn_to_slug)
            if cid:
                redirected = alias_redirect_id(cid, alias_map)
                if conn.execute(
                    "SELECT 1 FROM characters WHERE id=?", (redirected,)
                ).fetchone():
                    return redirected
                return cid
            # Variant folder but no 小XX / μ row — do not fall back to adult
            if _suffix_variant_kind(suffix):
                return None
        cid = _lookup_character_id_by_cn(conn, cn, cn_to_slug=cn_to_slug)
    if not cid and not _suffix_variant_kind(suffix):
        # Default skin already bound to this bare folder (danfo vs pinyin danfu)
        row = conn.execute(
            """
            SELECT character_id FROM skins
            WHERE is_default=1
              AND (pet_model_id=? OR kanmusu_dir=?)
            LIMIT 1
            """,
            (base, base),
        ).fetchone()
        if row:
            cid = str(row[0])
    if not cid:
        redirected = alias_redirect_id(base, alias_map)
        if conn.execute(
            "SELECT 1 FROM characters WHERE id=?", (redirected,)
        ).fetchone():
            return redirected
        if conn.execute("SELECT 1 FROM characters WHERE id=?", (base,)).fetchone():
            return base
        return None
    redirected = alias_redirect_id(cid, alias_map)
    if conn.execute(
        "SELECT 1 FROM characters WHERE id=?", (redirected,)
    ).fetchone():
        return redirected
    return cid


def repair_misbound_variant_pets(conn: sqlite3.Connection) -> int:
    """Clear ``*_younv`` / ``*_idol`` pets wrongly set on adult (non-variant) characters."""
    cleared = 0
    rows = conn.execute(
        """
        SELECT s.id, s.pet_model_id, c.name_zh
        FROM skins s
        JOIN characters c ON c.id = s.character_id
        WHERE IFNULL(s.pet_model_id,'') != ''
          AND (s.pet_model_id LIKE '%younv' OR s.pet_model_id LIKE '%idol')
        """
    ).fetchall()
    for sid, pet_id, name_zh in rows:
        pet = (pet_id or "").strip()
        zh = (name_zh or "").strip()
        _base, suf = strip_skin(pet)
        kind = _suffix_variant_kind(suf)
        if not kind:
            continue
        if kind == "younv":
            # Adult 安克雷奇 must not keep ankeleiqi_younv; 小安克雷奇 may.
            if zh.startswith("小"):
                continue
        elif kind == "idol":
            if "(μ兵装)" in zh or "μ兵装" in zh:
                continue
        conn.execute(
            """
            UPDATE skins SET pet_model_id='', updated_at=datetime('now')
            WHERE id=?
            """,
            (sid,),
        )
        cleared += 1
    return cleared


def bind_pet_folder_models(
    conn: sqlite3.Connection,
    *,
    pet_models: Path,
    alias_map: dict[str, str] | None = None,
    cn_to_slug: dict[str, str] | None = None,
    only_set: set[str] | None = None,
) -> int:
    """Scan ``data/pet`` and set ``pet_model_id`` (BLHX: ``slug``, ``slug_2``, …).

    Does not write ``kanmusu_dir`` (舰娘仍由 skin 树扫描). Skips ``*_h`` (oath)
    and non-existent ``*_1`` folders.
    """
    if not pet_models.is_dir():
        return 0
    alias_map = alias_map or {}
    cn_to_slug = cn_to_slug or {}
    from common.unpack_complete import is_hx_slug
    from common.path_policy import is_special_pet_folder
    from roster.skin_probe import _has_spine_assets

    bound = 0
    folders = sorted(
        p.name for p in pet_models.iterdir() if p.is_dir() and not p.name.startswith(".")
    )
    for folder in folders:
        if is_hx_slug(folder) or is_special_pet_folder(folder):
            continue
        base, suffix = strip_skin(folder)
        if (suffix or "").lower() == "h":
            continue
        if suffix.isdigit() and int(suffix) == 1:
            continue
        if only_set and base not in only_set and folder not in only_set:
            continue
        if not _has_spine_assets(pet_models / folder):
            continue
        cid = _resolve_character_id_for_folder(
            conn,
            base,
            alias_map=alias_map,
            cn_to_slug=cn_to_slug,
            suffix=suffix,
        )
        if not cid:
            continue
        # younv/idol folders bind to the variant character's default skin
        bind_suffix = "" if _suffix_variant_kind(suffix) else suffix
        sid = resolve_bind_skin_id(conn, cid, bind_suffix)
        if not sid or is_oath_skin_id(sid):
            continue
        conn.execute(
            """
            UPDATE skins SET
              pet_model_id=?,
              updated_at=datetime('now')
            WHERE id=?
            """,
            (folder, sid),
        )
        bound += 1
    return bound


def bind_oath_h_pets(
    conn: sqlite3.Connection,
    *,
    pet_models: Path,
    alias_map: dict[str, str] | None = None,
    cn_to_slug: dict[str, str] | None = None,
    only_set: set[str] | None = None,
) -> int:
    """Scan ``pet`` for ``{slug}_h`` folders and bind to ``{cid}-oath`` (pet only).

    誓约皮肤没有舰娘资源；只读桌宠目录，写入 ``pet_model_id``，清空 ``kanmusu_dir``。
    """
    if not pet_models.is_dir():
        return 0
    alias_map = alias_map or {}
    cn_to_slug = cn_to_slug or {}
    from common.unpack_complete import is_hx_slug
    from common.path_policy import is_special_pet_folder
    from roster.skin_probe import _has_spine_assets

    bound = 0
    folders = sorted(
        p.name for p in pet_models.iterdir() if p.is_dir() and not p.name.startswith(".")
    )
    for folder in folders:
        if is_hx_slug(folder) or is_special_pet_folder(folder):
            continue
        base, suffix = strip_skin(folder)
        if (suffix or "").lower() != "h":
            continue
        if only_set and base not in only_set and folder not in only_set:
            continue
        pet_dir = pet_models / folder
        if not _has_spine_assets(pet_dir):
            continue

        cid = _resolve_character_id_for_folder(
            conn,
            base,
            alias_map=alias_map,
            cn_to_slug=cn_to_slug,
            suffix=suffix,
        )
        if not cid:
            continue

        sid = resolve_bind_skin_id(conn, cid, "h")
        if not sid or not is_oath_skin_id(sid):
            continue
        conn.execute(
            """
            UPDATE skins SET
              kanmusu_dir='',
              pet_model_id=?,
              updated_at=datetime('now')
            WHERE id=?
            """,
            (folder, sid),
        )
        bound += 1
    return bound


def bind_unpacked_models(
    conn: sqlite3.Connection,
    unpacked: Path,
    *,
    pet_models: Path | None = None,
    alias_map: dict[str, str] | None = None,
    cn_to_slug: dict[str, str] | None = None,
    only_set: set[str] | None = None,
    create_missing: bool = True,
) -> int:
    """Attach pet/kanmusu paths onto Wiki skins; optionally create skins for orphans."""
    alias_map = alias_map or {}
    cn_to_slug = cn_to_slug or {}
    pet_models = pet_models or (appdata_data_dir() / "pet-models")
    if not unpacked.is_dir():
        # Still bind pet folders when skin root is missing
        n = bind_pet_folder_models(
            conn,
            pet_models=pet_models,
            alias_map=alias_map,
            cn_to_slug=cn_to_slug,
            only_set=only_set,
        )
        n += bind_oath_h_pets(
            conn,
            pet_models=pet_models,
            alias_map=alias_map,
            cn_to_slug=cn_to_slug,
            only_set=only_set,
        )
        return n
    from common.unpack_complete import is_hx_slug
    from common.path_policy import is_special_pet_folder

    bound = 0
    folders = sorted(
        p.name for p in unpacked.iterdir() if p.is_dir() and not p.name.startswith(".")
    )
    for folder in folders:
        if is_hx_slug(folder):
            continue
        if is_special_pet_folder(folder):
            continue
        base, suffix = strip_skin(folder)
        # 誓约 ``*_h`` 只走 pet 扫描，不在 skin/unpacked 树里绑舰娘
        if (suffix or "").lower() == "h":
            continue
        # BLHX: 无 ``slug_1``；第一套换装是 ``slug_2``
        if suffix.isdigit() and int(suffix) == 1:
            continue
        if only_set and base not in only_set and folder not in only_set:
            continue
        cn = (alias_map.get(base) or alias_map.get(base.lower()) or "").strip() or None
        cid: str | None = None
        if cn:
            # guanghui_idol → 光辉(μ兵装); beierfasite_younv → 小贝法
            variant_cn = variant_character_name_for_suffix(cn, suffix)
            if variant_cn:
                row = conn.execute(
                    """
                    SELECT id FROM characters
                    WHERE name_zh=? OR wiki_title=?
                    ORDER BY CASE
                      WHEN source='wiki' THEN 0
                      WHEN id LIKE 'p%' THEN 1
                      ELSE 2
                    END
                    LIMIT 1
                    """,
                    (variant_cn, variant_cn),
                ).fetchone()
                if row:
                    cid = str(row[0])
                    cn = variant_cn
                elif _suffix_variant_kind(suffix):
                    # Do not bind idol/younv assets onto the adult ship
                    continue
            if not cid:
                # Prefer existing Wiki roster row by Chinese name over alias pinyin stub
                row = conn.execute(
                    """
                    SELECT id FROM characters
                    WHERE name_zh=? OR wiki_title=?
                    ORDER BY CASE
                      WHEN source='wiki' THEN 0
                      WHEN id LIKE 'p%' THEN 1
                      ELSE 2
                    END
                    LIMIT 1
                    """,
                    (cn, cn),
                ).fetchone()
                if row:
                    cid = str(row[0])
                else:
                    cid = cn_to_slug.get(cn) or _resolve_character_id(cn, cn_to_slug)
                    # Prefer an existing roster row by Chinese name over a fresh hash id
                    if cid and not conn.execute(
                        "SELECT 1 FROM characters WHERE id=?", (cid,)
                    ).fetchone():
                        row = conn.execute(
                            "SELECT id FROM characters WHERE name_zh=? OR wiki_title=? LIMIT 1",
                            (cn, cn),
                        ).fetchone()
                        if row:
                            cid = str(row[0])
        if not cid:
            redirected = alias_redirect_id(base, alias_map)
            if conn.execute(
                "SELECT 1 FROM characters WHERE id=?", (redirected,)
            ).fetchone():
                cid = redirected
            elif conn.execute(
                "SELECT 1 FROM characters WHERE id=?", (base,)
            ).fetchone():
                cid = base
            else:
                # Painting / pet folder slug equals Wiki display name (e.g. 「22」)
                row = conn.execute(
                    """
                    SELECT id FROM characters
                    WHERE name_zh=? OR wiki_title=?
                    ORDER BY CASE
                      WHEN source='wiki' THEN 0
                      WHEN id LIKE 'p%' THEN 1
                      ELSE 2
                    END
                    LIMIT 1
                    """,
                    (base, base),
                ).fetchone()
                if row:
                    cid = str(row[0])
                elif create_missing:
                    # Orphan painting folder (collab / unmapped) → stub character in 自用库
                    cid = redirected or base
                    upsert_character(
                        conn,
                        {
                            "id": cid,
                            "name_zh": cn or base,
                            "name_en": "",
                            "wiki_title": cn or base,
                            "persona_id": cid,
                            "source": "unpacked",
                        },
                    )
                else:
                    continue
        else:
            original_cid = cid
            redirected = alias_redirect_id(cid, alias_map)
            if conn.execute(
                "SELECT 1 FROM characters WHERE id=?", (redirected,)
            ).fetchone():
                cid = redirected
            elif conn.execute(
                "SELECT 1 FROM characters WHERE id=?", (original_cid,)
            ).fetchone():
                cid = original_cid
            elif cn:
                # Case-insensitive display-name match (wiki name_zh may be z13)
                row = conn.execute(
                    """
                    SELECT id FROM characters
                    WHERE lower(name_zh)=lower(?) OR lower(wiki_title)=lower(?)
                    ORDER BY CASE
                      WHEN source='wiki' THEN 0
                      WHEN id LIKE 'p%' THEN 1
                      ELSE 2
                    END
                    LIMIT 1
                    """,
                    (cn, cn),
                ).fetchone()
                if row:
                    cid = str(row[0])
                elif not create_missing:
                    continue
                else:
                    cid = redirected
            elif not create_missing:
                continue
            else:
                cid = redirected
        if only_set and cid not in only_set and base not in only_set:
            continue
        sid = resolve_bind_skin_id(conn, cid, suffix)
        if not sid:
            # Also match a prior bind that already points at this exact folder
            row = conn.execute(
                "SELECT id FROM skins WHERE character_id=? AND kanmusu_dir=? LIMIT 1",
                (cid, folder),
            ).fetchone()
            sid = str(row[0]) if row else None
        if not sid:
            if not create_missing:
                continue
            # Wiki characters already have fixed slots — do not invent L2D-N for
            # extra game folders (i404_3 when Wiki only lists default+skin1).
            if _character_has_wiki_skins(conn, cid):
                continue
            # Ensure character row exists for orphan folders
            if not conn.execute(
                "SELECT 1 FROM characters WHERE id=?", (cid,)
            ).fetchone():
                upsert_character(
                    conn,
                    {
                        "id": cid,
                        "name_zh": cn or base,
                        "name_en": "",
                        "wiki_title": cn or base,
                        "persona_id": cid,
                        "source": "unpacked",
                    },
                )
            sid = ensure_skin_for_unpacked(conn, cid, folder, suffix)
        else:
            # Fold leftover L2D-{suffix} into the Wiki/target skin when present
            reclaim_l2d_orphan_into_skin(
                conn, cid, sid, folder=folder, suffix=suffix
            )
        kanmusu_dir, pet_model_id = _bind_paths_for_folder(
            unpacked / folder, pet_models, folder
        )
        # 誓约：只绑桌宠，不绑舰娘
        if is_oath_skin_id(str(sid)):
            oath_pet = pet_model_id
            if not oath_pet and (pet_models / folder).is_dir():
                oath_pet = folder
            elif not oath_pet and (pet_models / f"skin-{folder}").is_dir():
                oath_pet = f"skin-{folder}"
            conn.execute(
                """
                UPDATE skins SET
                  kanmusu_dir='',
                  pet_model_id=CASE WHEN ?!='' THEN ? ELSE pet_model_id END,
                  updated_at=datetime('now')
                WHERE id=?
                """,
                (oath_pet, oath_pet, sid),
            )
        else:
            conn.execute(
                """
                UPDATE skins SET
                  kanmusu_dir=?,
                  pet_model_id=?,
                  updated_at=datetime('now')
                WHERE id=?
                """,
                (kanmusu_dir, pet_model_id, sid),
            )
        bound += 1
    # 桌宠：扫 pet（``slug`` / ``slug_2`` / …）；誓约 ``*_h`` 另处理
    bound += bind_pet_folder_models(
        conn,
        pet_models=pet_models,
        alias_map=alias_map,
        cn_to_slug=cn_to_slug,
        only_set=only_set,
    )
    bound += bind_oath_h_pets(
        conn,
        pet_models=pet_models,
        alias_map=alias_map,
        cn_to_slug=cn_to_slug,
        only_set=only_set,
    )
    # Drop younv/idol pets left on adult characters, then heal Wiki slot mismatches
    bound += repair_misbound_variant_pets(conn)
    repair_stats = repair_blhx_skin_folder_binds(conn)
    bound += int(repair_stats.get("misindexed_moved") or 0)
    bound += int(repair_stats.get("l2d_reclaimed") or 0)
    bound += int(repair_stats.get("l2d_pruned") or 0)
    return bound

def cmd_repair_l2d_binds(args: argparse.Namespace) -> int:
    """Heal BLHX folder↔Wiki skin mismatches (misindexed binds + L2D orphans)."""
    db = Path(args.db) if args.db else default_local_db()
    if not db.is_file():
        emit({"ok": False, "error": f"db not found: {db}"})
        return 1
    conn = connect(db)
    stats = repair_blhx_skin_folder_binds(conn)
    from common.path_policy import default_pet

    alias_map = enrich_alias_map_from_roster(
        conn,
        {
            **LIVE2D_ALIASES,
            **load_json(repo_root() / "data/wiki/live2d-aliases.json", {}),
        },
    )
    pet_root = default_pet()
    variant_cleared = repair_misbound_variant_pets(conn)
    pet_bound = bind_pet_folder_models(
        conn,
        pet_models=pet_root,
        alias_map=alias_map,
    )
    oath_bound = bind_oath_h_pets(
        conn,
        pet_models=pet_root,
        alias_map=alias_map,
    )
    conn.commit()
    left = conn.execute(
        "SELECT count(*) FROM skins WHERE id LIKE '%-L2D-%'"
    ).fetchone()[0]
    conn.close()
    emit(
        {
            "ok": True,
            **stats,
            "variant_pet_cleared": variant_cleared,
            "pet_folder_bound": pet_bound,
            "oath_h_bound": oath_bound,
            "l2d_skins_remaining": left,
            "db": str(db),
        }
    )
    return 0

# --- import_wiki ---
def _wiki_line_groups(row: sqlite3.Row, ship_cols: set[str]) -> list[dict]:
    if "lines_by_skin_json" not in ship_cols:
        return []
    try:
        raw = row["lines_by_skin_json"]
    except (IndexError, KeyError):
        return []
    try:
        data = json.loads(raw or "[]")
    except json.JSONDecodeError:
        return []
    return data if isinstance(data, list) else []

def _wiki_skin_slots(row: sqlite3.Row, ship_cols: set[str]) -> list[dict]:
    if "skins_json" not in ship_cols:
        return []
    try:
        raw = row["skins_json"]
    except (IndexError, KeyError):
        return []
    try:
        data = json.loads(raw or "[]")
    except json.JSONDecodeError:
        return []
    return data if isinstance(data, list) else []

def _upsert_skins_from_slots(
    conn: sqlite3.Connection,
    cid: str,
    slots: list[dict],
    *,
    purge_orphans: bool = False,
) -> set[str]:
    """按 Wiki TabContainer 槽位 upsert 皮肤，并删除旧错误项。返回 Wiki keep + 保留手工皮 id。"""
    keep = expected_skin_ids_from_slots(cid, slots)
    # re-walk slots for upsert metadata (keep already computed)
    for slot in slots:
        if not isinstance(slot, dict):
            continue
        key = str(slot.get("key") or "").strip()
        label = str(slot.get("label") or key).strip()
        kind = str(slot.get("kind") or "skin")
        if not key or is_hidden_wiki_skin(kind=kind, label=label):
            continue
        sort_order = int(slot.get("sort_order") or 0)
        if key == "default" or kind == "default":
            sid = skin_db_id(cid, "default")
            is_default = True
            name_zh = "默认皮肤"
            skin_index = 0
        elif key == "oath" or kind == "oath":
            sid = f"{cid}-oath"
            is_default = False
            name_zh = label or "誓约"
            if is_generic_wiki_skin_label(name_zh):
                name_zh = "誓约"
            skin_index = 100 + sort_order
        else:
            sid = f"{cid}-{key}"
            is_default = False
            name_zh = label or key
            skin_index = sort_order
        keep.add(sid)
        meta_obj: dict = {
            "slot_key": key,
            "lines_import": {
                "status": "empty",
                "wiki_skin": label,
                "matched_by": "slot",
            },
            "bind_policy": "pet_only" if (key == "oath" or kind == "oath") else "full",
        }
        if slot.get("image_url"):
            meta_obj["wiki_image_url"] = slot.get("image_url")
        # Preserve non-empty lines_import / extra meta on existing rows
        prev = conn.execute(
            "SELECT meta_json FROM skins WHERE id=?", (sid,)
        ).fetchone()
        if prev:
            try:
                old_meta = json.loads(prev[0] or "{}")
            except json.JSONDecodeError:
                old_meta = {}
            if isinstance(old_meta, dict):
                old_li = old_meta.get("lines_import")
                if isinstance(old_li, dict) and str(old_li.get("status") or "") not in (
                    "",
                    "empty",
                ):
                    meta_obj["lines_import"] = old_li
                for k, v in old_meta.items():
                    if k not in meta_obj:
                        meta_obj[k] = v
        upsert_skin(
            conn,
            {
                "id": sid,
                "character_id": cid,
                "name_zh": name_zh,
                "name_en": "",
                "skin_index": skin_index,
                "pet_model_id": "",
                "kanmusu_dir": "",
                "sort_order": sort_order,
                "is_default": is_default,
                "meta_json": json.dumps(meta_obj, ensure_ascii=False),
                "lines": [],
            },
            replace_lines=False,
        )
        # upsert_skin 的 kanmusu CASE 不会用空串覆盖；誓约强制不绑舰娘
        if key == "oath" or kind == "oath":
            conn.execute(
                "UPDATE skins SET kanmusu_dir='' WHERE id=?", (sid,)
            )
    # 仅在解析出至少一个权威皮肤时清理 Wiki 孤儿；默认保留 L2D/手工皮
    if keep:
        _delete_skins_not_in(conn, cid, keep, purge_orphans=purge_orphans)
    return local_skin_ids(conn, cid)

def _apply_lines_import(
    conn: sqlite3.Connection,
    cid: str,
    groups: list[dict],
    flat_raw: list,
    stats: dict,
) -> None:
    """Write per-skin lines + meta; update stats counters / report lists."""
    skins = _skin_row_dicts(conn, cid)
    if not skins:
        return

    def _bump(key: str, n: int = 1) -> None:
        stats[key] = int(stats.get(key) or 0) + n

    report_cap = 40

    def _note(bucket: str, item: dict) -> None:
        lst = stats.setdefault(bucket, [])
        if len(lst) < report_cap:
            lst.append(item)

    if groups:
        report = apply_lines_by_skin(groups, skins, lines_rows_from_wiki)
        by_id = {s["id"]: s for s in skins}
        for a in report["assignments"]:
            sk = by_id.get(a["skin_id"]) or {"id": a["skin_id"], "character_id": cid}
            sid = str(a["skin_id"])
            wiki_skin = str(a.get("wiki_skin") or "").strip()
            is_def = bool(sk.get("is_default")) or sid.endswith("-default")
            if is_def:
                name_zh = "默认皮肤"
            elif wiki_skin and not is_generic_wiki_skin_label(wiki_skin):
                name_zh = wiki_skin
            else:
                name_zh = sk.get("name_zh") or sid
            if is_oath_skin_id(sid) and is_generic_wiki_skin_label(name_zh):
                name_zh = "誓约"
            meta = merge_meta_json(
                sk.get("meta_json"),
                {
                    "status": a["status"],
                    "wiki_skin": a.get("wiki_skin"),
                    "matched_by": a.get("matched_by"),
                },
            )
            kanmusu = "" if is_oath_skin_id(sid) else (sk.get("kanmusu_dir") or "")
            upsert_skin(
                conn,
                {
                    **{k: sk.get(k) for k in (
                        "id", "character_id", "name_zh", "name_en", "skin_index",
                        "pet_model_id", "kanmusu_dir", "sort_order", "is_default",
                    )},
                    "id": sid,
                    "character_id": cid,
                    "name_zh": name_zh,
                    "kanmusu_dir": kanmusu,
                    "meta_json": meta,
                    "lines": a["lines"],
                },
                replace_lines=True,
            )
            if is_oath_skin_id(sid):
                conn.execute(
                    "UPDATE skins SET kanmusu_dir='' WHERE id=?", (sid,)
                )
            if a["status"] == "ready":
                _bump("skins_lines_ok")
            else:
                _bump("skins_lines_empty")
        for u in report["wiki_unmatched"]:
            _bump("wiki_skins_unmatched")
            _note(
                "lines_report",
                {"type": "wiki_unmatched", "character_id": cid, **u},
            )
        for sid in report["roster_unmatched_ids"]:
            sk = by_id.get(sid)
            if not sk:
                continue
            _bump("roster_skins_unmatched")
            meta = merge_meta_json(
                sk.get("meta_json"),
                {
                    "status": "unmatched",
                    "wiki_skin": None,
                    "matched_by": None,
                },
            )
            upsert_skin(
                conn,
                {
                    "id": sid,
                    "character_id": cid,
                    "name_zh": sk.get("name_zh") or sid,
                    "name_en": sk.get("name_en") or "",
                    "skin_index": sk.get("skin_index"),
                    "pet_model_id": sk.get("pet_model_id") or "",
                    "kanmusu_dir": sk.get("kanmusu_dir") or "",
                    "sort_order": sk.get("sort_order") or 0,
                    "is_default": sk.get("is_default"),
                    "meta_json": meta,
                    "lines": [],
                },
                replace_lines=False,
            )
            _note(
                "lines_report",
                {
                    "type": "roster_unmatched",
                    "character_id": cid,
                    "skin_id": sid,
                    "name_zh": sk.get("name_zh") or "",
                },
            )
        return

    # Flat-only / legacy wiki rows
    lines = lines_rows_from_wiki(flat_raw)
    for sk in skins:
        is_def = bool(sk.get("is_default")) or str(sk.get("id") or "").endswith(
            "-default"
        )
        if is_def and lines:
            meta = merge_meta_json(
                sk.get("meta_json"),
                {"status": "stale_flat", "wiki_skin": "default", "matched_by": "default"},
            )
            upsert_skin(
                conn,
                {
                    "id": sk["id"],
                    "character_id": cid,
                    "name_zh": sk.get("name_zh") or sk["id"],
                    "name_en": sk.get("name_en") or "",
                    "skin_index": sk.get("skin_index"),
                    "pet_model_id": sk.get("pet_model_id") or "",
                    "kanmusu_dir": sk.get("kanmusu_dir") or "",
                    "sort_order": sk.get("sort_order") or 0,
                    "is_default": True,
                    "meta_json": meta,
                    "lines": lines,
                },
                replace_lines=True,
            )
            _bump("skins_lines_ok")
        else:
            meta = merge_meta_json(
                sk.get("meta_json"),
                {
                    "status": "stale_flat" if is_def else "empty",
                    "wiki_skin": None,
                    "matched_by": None,
                },
            )
            upsert_skin(
                conn,
                {
                    "id": sk["id"],
                    "character_id": cid,
                    "name_zh": sk.get("name_zh") or sk["id"],
                    "name_en": sk.get("name_en") or "",
                    "skin_index": sk.get("skin_index"),
                    "pet_model_id": sk.get("pet_model_id") or "",
                    "kanmusu_dir": sk.get("kanmusu_dir") or "",
                    "sort_order": sk.get("sort_order") or 0,
                    "is_default": sk.get("is_default"),
                    "meta_json": meta,
                    "lines": [],
                },
                replace_lines=False,
            )
            if not is_def:
                _bump("skins_lines_empty")

def run_import_wiki(
    db: Path | None = None,
    wiki_db: Path | None = None,
    unpacked: Path | None = None,
    en_map: Path | None = None,
    only_ids: list[str] | str | None = None,
    scope: str = "all",
    phases: set[str] | list[str] | str | None = None,
    incremental: bool = False,
) -> dict:
    """从 BWIKI sqlite 写入自用库。

    scope:
      - all（默认）：导入 ships 表全部舰船（阵营/CV/稀有度/台词等）
      - unpacked：旧行为，仅扫 data/skin 文件夹

    phases: characters | skins | lines | bind（默认全部）
    incremental: 皮肤已与 skins_json 对齐则跳过该角色
    """
    db = Path(db) if db else default_local_db()
    if wiki_db is None:
        from common.path_policy import default_wiki_db as _default_wiki_db

        wiki_db = _default_wiki_db()
    else:
        wiki_db = Path(wiki_db)
    unpacked = Path(unpacked) if unpacked else (repo_root() / "data/skin")
    en_map_path = Path(en_map) if en_map else (repo_root() / "data/wiki/ship-en-names.json")
    en_map_data = load_json(en_map_path, {})
    alias_map = {
        **LIVE2D_ALIASES,
        **load_json(repo_root() / "data/wiki/live2d-aliases.json", {}),
    }
    only_set = _parse_id_filter(only_ids)
    phase_set = _parse_import_phases(phases)
    do_chars = "characters" in phase_set
    do_skins = "skins" in phase_set
    do_lines = "lines" in phase_set
    do_bind = "bind" in phase_set
    scope = (scope or "all").strip().lower()
    if scope not in ("all", "unpacked"):
        scope = "all"

    if not wiki_db.is_file():
        return {"ok": False, "error": f"wiki db missing: {wiki_db}"}

    conn = connect(db)
    apply_schema(conn)
    wiki: sqlite3.Connection | None = None
    try:
        # 先合并同名别名双开（aijiang/aijier），再清 folder-like 假角色，再继续导入
        merge_roster_duplicates_by_name(conn, alias_map)
        purge_folder_like_characters(conn, alias_map)
        wiki = sqlite3.connect(str(wiki_db), timeout=60.0)
        wiki.row_factory = sqlite3.Row
        wiki.execute("PRAGMA busy_timeout=60000")
        ship_cols = {r[1] for r in wiki.execute("PRAGMA table_info(ships)")}
        select_cols = [
            "wiki_title",
            "display_name",
            "aliases_json",
            "lines_json",
            "assets_json",
        ]
        for opt in (
            "cv",
            "faction",
            "ship_type",
            "rarity",
            "persona_reference",
            "lines_by_skin_json",
            "skins_json",
        ):
            if opt in ship_cols:
                select_cols.append(opt)
        select_sql = ", ".join(select_cols)

        cn_to_slug = _build_cn_to_slug(conn, wiki, alias_map)
        pet_models = appdata_data_dir() / "pet-models"
        upserted: list[dict] = []
        chars_seen: set[str] = set()
        lines_stats: dict = {
            "skins_lines_ok": 0,
            "skins_lines_empty": 0,
            "wiki_skins_unmatched": 0,
            "roster_skins_unmatched": 0,
            "lines_report": [],
            "skins_skipped": 0,
            "skins_updated": 0,
        }

        def import_ship_row(row: sqlite3.Row) -> str | None:
            display = (row["display_name"] or row["wiki_title"] or "").strip()
            wiki_title = (row["wiki_title"] or display).strip()
            if not display:
                return None
            cid = _resolve_character_id(display, cn_to_slug)
            if only_set:
                matched = False
                for oid in only_set:
                    if oid == cid or oid == display or alias_map.get(oid) == display:
                        if oid in alias_map:
                            cid = alias_redirect_id(oid, alias_map)
                        matched = True
                        break
                if not matched:
                    return None

            aliases = json.loads(row["aliases_json"] or "[]")
            english = pick_english(aliases, en_map_data.get(cid, "") or en_map_data.get(display, ""))
            keys = set(row.keys())
            cv = clean_cv(row["cv"] or "") if "cv" in keys else ""
            faction = (row["faction"] or "").strip() if "faction" in keys else ""
            ship_type = (row["ship_type"] or "").strip() if "ship_type" in keys else ""
            rarity = (row["rarity"] or "").strip() if "rarity" in keys else ""
            desc = ""
            if "persona_reference" in keys and row["persona_reference"]:
                desc = str(row["persona_reference"]).strip()
                if len(desc) > 4000:
                    desc = desc[:4000].rstrip() + "…"

            if do_chars or do_skins or do_lines:
                # skins/lines 阶段也确保角色行存在
                upsert_character(
                    conn,
                    {
                        "id": cid,
                        "name_zh": display,
                        "name_en": english,
                        "wiki_title": wiki_title,
                        "cv": cv,
                        "faction": faction,
                        "ship_type": ship_type,
                        "rarity": rarity,
                        "persona_id": cid,
                        "source": "wiki",
                        "description": desc,
                    },
                )
            else:
                return None

            if do_skins:
                slots = _wiki_skin_slots(row, ship_cols)
                if (
                    incremental
                    and slots
                    and character_skins_in_sync(conn, cid, slots)
                ):
                    lines_stats["skins_skipped"] = int(lines_stats["skins_skipped"]) + 1
                elif slots:
                    # 权威皮肤清单：整角色替换（删除以往错误/残留皮肤），非纯增量
                    _upsert_skins_from_slots(conn, cid, slots)
                    lines_stats["skins_updated"] = int(lines_stats["skins_updated"]) + 1
                else:
                    keep_legacy: set[str] = set()
                    default_skin_id = skin_db_id(cid, "default")
                    keep_legacy.add(default_skin_id)
                    upsert_skin(
                        conn,
                        {
                            "id": default_skin_id,
                            "character_id": cid,
                            "name_zh": "默认皮肤",
                            "name_en": "",
                            "skin_index": 0,
                            "pet_model_id": "",
                            "kanmusu_dir": "",
                            "sort_order": 0,
                            "is_default": True,
                            "lines": [],
                        },
                        replace_lines=False,
                    )
                    # legacy: assets only when no TabContainer skins_json
                    assets = json.loads(row["assets_json"] or "[]")
                    if isinstance(assets, list):
                        for i, asset in enumerate(assets):
                            if not isinstance(asset, dict):
                                continue
                            title = pick_skin_title([asset], None, "") or ""
                            if not title or title in ("默认",) or "改造" in title:
                                continue
                            sid = f"{cid}-skin{i + 1}"
                            keep_legacy.add(sid)
                            upsert_skin(
                                conn,
                                {
                                    "id": sid,
                                    "character_id": cid,
                                    "name_zh": title,
                                    "name_en": "",
                                    "skin_index": i + 1,
                                    "pet_model_id": "",
                                    "kanmusu_dir": "",
                                    "sort_order": i + 1,
                                    "is_default": False,
                                    "lines": [],
                                },
                                replace_lines=False,
                            )
                    if keep_legacy:
                        _delete_skins_not_in(conn, cid, keep_legacy)

            line_n = 0
            if do_lines:
                lines_raw = json.loads(row["lines_json"] or "[]")
                if not isinstance(lines_raw, list):
                    lines_raw = []
                groups = _wiki_line_groups(row, ship_cols)
                _apply_lines_import(conn, cid, groups, lines_raw, lines_stats)
                line_n = conn.execute(
                    "SELECT count(*) FROM skin_lines WHERE skin_id IN (SELECT id FROM skins WHERE character_id=?)",
                    (cid,),
                ).fetchone()[0]

            cn_to_slug[display] = cid
            chars_seen.add(cid)
            upserted.append(
                {
                    "character_id": cid,
                    "name_zh": display,
                    "name_en": english,
                    "faction": faction,
                    "cv": cv,
                    "lines": line_n,
                }
            )
            return cid

        # —— 全量：wiki ships ——
        if scope == "all" and (do_chars or do_skins or do_lines):
            for row in wiki.execute(f"SELECT {select_sql} FROM ships ORDER BY display_name"):
                import_ship_row(row)

        # —— 解包目录：unpacked 模式下补角色；绑定永不按 folder 新建皮肤 ——
        if not unpacked.is_dir():
            folders: list[str] = []
            if scope == "unpacked":
                return {"ok": False, "error": f"unpacked missing: {unpacked}"}
        else:
            folders = sorted(
                p.name for p in unpacked.iterdir() if p.is_dir() and not p.name.startswith(".")
            )

        if scope == "unpacked" and (do_chars or do_skins or do_lines):
            for folder in folders:
                base, _suffix = strip_skin(folder)
                if only_set and base not in only_set and folder not in only_set:
                    continue
                cn = alias_map.get(base)
                row = None
                if cn:
                    row = wiki.execute(
                        f"SELECT {select_sql} FROM ships WHERE display_name=? OR wiki_title=?",
                        (cn, cn),
                    ).fetchone()
                if row is not None:
                    import_ship_row(row)
                elif do_chars:
                    display = cn or base
                    cid = base
                    if only_set and cid not in only_set:
                        continue
                    upsert_character(
                        conn,
                        {
                            "id": cid,
                            "name_zh": display,
                            "name_en": en_map_data.get(cid, ""),
                            "wiki_title": display,
                            "persona_id": cid,
                            "source": "unpacked",
                        },
                    )
                    chars_seen.add(cid)

        if only_set and scope == "all" and (do_chars or do_skins or do_lines):
            # 保证点名 id 即使 wiki 无匹配也至少尝试一次（拼音别名）
            for oid in sorted(only_set):
                if oid in chars_seen:
                    continue
                cn = alias_map.get(oid)
                if not cn:
                    continue
                row = wiki.execute(
                    f"SELECT {select_sql} FROM ships WHERE display_name=? OR wiki_title=?",
                    (cn, cn),
                ).fetchone()
                if row:
                    import_ship_row(row)

        bind_n = 0
        if do_bind:
            cn_to_slug = _build_cn_to_slug(conn, wiki, alias_map)
            bind_n = bind_unpacked_models(
                conn,
                unpacked,
                pet_models=pet_models,
                alias_map=alias_map,
                cn_to_slug=cn_to_slug,
                only_set=only_set,
            )

        if only_set is None and scope == "all" and do_chars:
            seed_chars = load_json(roster_dir() / "seed" / "characters.json", [])
            for c in seed_chars if isinstance(seed_chars, list) else []:
                if isinstance(c, dict) and c.get("id"):
                    upsert_character(
                        conn, {**c, "name_zh": c.get("name_zh") or c.get("name") or c["id"]}
                    )
        if only_set is None and scope == "all" and do_skins:
            seed_skins = load_json(roster_dir() / "seed" / "skins.json", [])
            for s in seed_skins if isinstance(seed_skins, list) else []:
                if isinstance(s, dict) and s.get("id") and s.get("character_id"):
                    upsert_skin(
                        conn,
                        {
                            **s,
                            "name_zh": s.get("name_zh") or s.get("name") or s["id"],
                            "lines": s.get("lines") or [],
                        },
                        replace_lines=bool(s.get("lines")),
                    )

        if do_skins:
            # 原皮统一显示名（不依赖 Wiki 重抓）
            conn.execute(
                """
                UPDATE skins SET name_zh='默认皮肤', updated_at=datetime('now')
                WHERE is_default=1
                  AND (name_zh IS NULL OR trim(name_zh)='' OR name_zh IN ('默认','通常','default'))
                """
            )
            conn.execute(
                """
                UPDATE skins SET kanmusu_dir='', updated_at=datetime('now')
                WHERE id LIKE '%-oath' AND IFNULL(kanmusu_dir,'') != ''
                """
            )

        conn.commit()
        char_count = conn.execute("SELECT count(*) FROM characters").fetchone()[0]
        return {
            "ok": True,
            "db": str(db),
            "scope": scope,
            "phases": sorted(phase_set),
            "bound_models": bind_n,
            "skins_skipped": lines_stats.get("skins_skipped", 0),
            "skins_updated": lines_stats.get("skins_updated", 0),
            "upserted": len(upserted),
            "character_total": char_count,
            "sample": upserted[:8],
            "only_ids": sorted(only_set) if only_set else None,
            "skins_lines_ok": lines_stats["skins_lines_ok"],
            "skins_lines_empty": lines_stats["skins_lines_empty"],
            "wiki_skins_unmatched": lines_stats["wiki_skins_unmatched"],
            "roster_skins_unmatched": lines_stats["roster_skins_unmatched"],
            "lines_report": lines_stats["lines_report"],
        }

    finally:
        try:
            conn.close()
        except Exception as exc:  # noqa: BLE001
            logging.debug("roster conn close: %s", exc)
        if wiki is not None:
            try:
                wiki.close()
            except Exception as exc:  # noqa: BLE001
                logging.debug("wiki conn close: %s", exc)

def cmd_import_wiki(args: argparse.Namespace) -> int:
    result = run_import_wiki(
        db=Path(args.db) if args.db else None,
        wiki_db=Path(args.wiki_db) if args.wiki_db else None,
        unpacked=Path(args.unpacked),
        en_map=Path(args.en_map),
        only_ids=getattr(args, "ids", None) or None,
        scope=getattr(args, "scope", None) or "all",
    )
    emit(result)
    return 0 if result.get("ok") else 1

# --- sync ---
def character_to_manifest(conn: sqlite3.Connection, cid: str) -> dict | None:
    c = conn.execute("SELECT * FROM characters WHERE id=?", (cid,)).fetchone()
    if not c:
        return None
    skins_out = []
    for s in conn.execute(
        "SELECT * FROM skins WHERE character_id=? ORDER BY sort_order, id", (cid,)
    ):
        lines = []
        for ln in conn.execute(
            "SELECT * FROM skin_lines WHERE skin_id=? ORDER BY sort_order, id", (s["id"],)
        ):
            item = {"text": ln["text"]}
            if ln["animation"]:
                item["animation"] = ln["animation"]
            if ln["wiki_key"]:
                item["wiki_key"] = ln["wiki_key"]
            if ln["audio_url"]:
                item["audio_url"] = ln["audio_url"]
            if ln["audio_relpath"]:
                item["audio_relpath"] = ln["audio_relpath"]
            lines.append(item)
        skin_id = skin_manifest_id(cid, s["id"])
        skins_out.append(
            {
                "id": skin_id,
                "name": s["name_zh"],
                "english_name": s["name_en"] or "",
                "model_id": s["pet_model_id"] or "",
                "default": bool(s["is_default"]),
                "skin_index": s["skin_index"],
                "kanmusu_dir": s["kanmusu_dir"] or None,
                "lines": lines,
            }
        )
    preferred = next((s["id"] for s in skins_out if s.get("default")), None)
    if preferred is None and skins_out:
        preferred = skins_out[0]["id"]
    return {
        "id": c["id"],
        "name": c["name_zh"],
        "english_name": c["name_en"] or "",
        "wiki_title": c["wiki_title"] or "",
        "cv": c["cv"] or "",
        "source": c["source"] or "roster",
        "description": c["description"] or "",
        "persona_id": c["persona_id"] or c["id"],
        "faction": c["faction"] or "",
        "ship_type": c["ship_type"] or "",
        "rarity": c["rarity"] or "",
        "skins": skins_out,
        "preferred_skin_id": preferred,
    }

def run_sync_appdata(
    db: Path | None = None,
    data_dir: Path | None = None,
    ids: str = "",
    force_lines: bool = False,
    replace: bool = True,
) -> dict:
    """同步自用库 → AppData characters/manifest.json。

    replace=True（默认）：AppData 角色列表改成「本次同步的自用库角色」
    （不再把旧的八百多个 wiki 角色粘在一起）。
    replace=False：仅 upsert，保留 AppData 里其它角色。
    """
    db = Path(db) if db else default_local_db()
    data_dir = Path(data_dir) if data_dir else appdata_data_dir()
    if not db.is_file():
        return {"ok": False, "error": f"local db missing: {db}"}
    conn = connect(db)
    manifest_path = data_dir / "characters" / "manifest.json"
    manifest_path.parent.mkdir(parents=True, exist_ok=True)
    existing = load_json(manifest_path, {"version": 1, "default_id": "", "characters": []})
    if replace:
        by_id: dict = {}
    else:
        by_id = {c["id"]: c for c in existing.get("characters", []) if isinstance(c, dict)}

    char_ids = [r["id"] for r in conn.execute("SELECT id FROM characters ORDER BY id")]
    if ids:
        want = {x.strip() for x in ids.split(",") if x.strip()}
        char_ids = [i for i in char_ids if i in want]

    synced = []
    for cid in char_ids:
        char = character_to_manifest(conn, cid)
        if not char:
            continue
        prev = None if replace else by_id.get(cid)
        if prev and not force_lines:
            # keep user-edited lines if present
            prev_skins = {s.get("id"): s for s in (prev.get("skins") or []) if isinstance(s, dict)}
            for s in char["skins"]:
                old = prev_skins.get(s["id"])
                if old and old.get("lines") and not force_lines:
                    s["lines"] = old["lines"]
        by_id[cid] = char
        synced.append(cid)

        # copy cubism if present under unpacked
        for s in char["skins"]:
            kd = (s.get("kanmusu_dir") or "").strip()
            if not kd:
                continue
            src = repo_root() / "data" / "model" / "unpacked" / kd
            dst = data_dir / "kanmusu-models" / kd
            if src.is_dir():
                if dst.exists():
                    shutil.rmtree(dst)
                shutil.copytree(src, dst)

    before = len(existing.get("characters") or [])
    existing["characters"] = dedupe_characters_by_name(list(by_id.values()))
    # 全量覆盖时 default 落到自用库第一个；若仍在名单里则保留原 default
    old_default = existing.get("default_id") or ""
    if old_default and any(c.get("id") == old_default for c in existing["characters"]):
        existing["default_id"] = old_default
    elif existing["characters"]:
        existing["default_id"] = existing["characters"][0]["id"]
    else:
        existing["default_id"] = ""
    manifest_path.write_text(
        json.dumps(existing, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )

    # mirror kanmusu manifest for desktop_open
    km_chars = []
    for c in existing["characters"]:
        km_skins = []
        for s in c.get("skins") or []:
            kd = (s.get("kanmusu_dir") or "").strip()
            if not kd:
                continue
            km_skins.append(
                {
                    "id": s["id"],
                    "name": s.get("name") or kd,
                    "model_dir": kd,
                    "lines": [
                        {
                            "text": ln.get("text", ""),
                            **({"animation": ln["animation"]} if ln.get("animation") else {}),
                        }
                        for ln in (s.get("lines") or [])
                        if isinstance(ln, dict) and (ln.get("text") or "").strip()
                    ],
                }
            )
        if km_skins:
            km_chars.append(
                {
                    "id": c["id"],
                    "name": c.get("name") or c["id"],
                    "description": c.get("description") or "",
                    "skins": km_skins,
                }
            )
    km_path = data_dir / "kanmusu" / "manifest.json"
    km_path.parent.mkdir(parents=True, exist_ok=True)
    km_path.write_text(
        json.dumps({"version": 1, "characters": km_chars}, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )
    conn.close()
    return {
        "ok": True,
        "manifest": str(manifest_path),
        "synced": synced,
        "replace": replace,
        "before_count": before,
        "after_count": len(existing["characters"]),
    }

def cmd_sync_appdata(args: argparse.Namespace) -> int:
    result = run_sync_appdata(
        db=Path(args.db) if args.db else None,
        data_dir=Path(args.data_dir) if args.data_dir else None,
        ids=args.ids or "",
        force_lines=bool(args.force_lines),
        replace=not bool(getattr(args, "merge", False)),
    )
    emit(result)
    return 0 if result.get("ok") else 1

def copy_subset_db(src: Path, dst: Path, character_ids: list[str]) -> dict:
    if dst.exists():
        dst.unlink()
    dst.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(src, dst)
    conn = connect(dst)
    apply_schema(conn)
    placeholders = ",".join("?" * len(character_ids)) if character_ids else "''"
    if character_ids:
        conn.execute(f"DELETE FROM characters WHERE id NOT IN ({placeholders})", character_ids)
        # cascades may not fire for skins if we only delete characters with FK — ensure
        conn.execute(
            f"DELETE FROM skins WHERE character_id NOT IN ({placeholders})",
            character_ids,
        )
        conn.execute(
            """
            DELETE FROM skin_lines WHERE skin_id NOT IN (SELECT id FROM skins)
            """
        )
    else:
        conn.execute("DELETE FROM skin_lines")
        conn.execute("DELETE FROM skins")
        conn.execute("DELETE FROM characters")
    conn.execute(
        "INSERT INTO meta(key, value) VALUES(?, ?) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        ("bundled", "1"),
    )
    conn.commit()
    counts = {
        "characters": conn.execute("SELECT count(*) FROM characters").fetchone()[0],
        "skins": conn.execute("SELECT count(*) FROM skins").fetchone()[0],
        "lines": conn.execute("SELECT count(*) FROM skin_lines").fetchone()[0],
    }
    conn.close()
    return counts

def run_publish_bundled(db: Path | None = None, ids: str = "") -> dict:
    db = Path(db) if db else default_local_db()
    if not db.is_file():
        return {"ok": False, "error": f"local db missing: {db} — run import first"}
    allow = load_json(allowlist_path(), {"character_ids": []})
    id_list = list(allow.get("character_ids") or [])
    if ids:
        id_list = [x.strip() for x in ids.split(",") if x.strip()]
    if not id_list:
        return {"ok": False, "error": "empty allowlist — refuse publishing entire local DB"}

    out_db = bundled_roster_dir() / "handaily-roster.sqlite"
    counts = copy_subset_db(db, out_db, id_list)

    # refresh characters/manifest.json for allowlisted ids only (merge keep others if any)
    conn = connect(db)
    bundled_manifest_path = bundled_roster_dir() / "characters" / "manifest.json"
    bundled_manifest = load_json(
        bundled_manifest_path, {"version": 1, "default_id": "cheshire", "characters": []}
    )
    by_id = {
        c["id"]: c for c in bundled_manifest.get("characters", []) if isinstance(c, dict)
    }
    for cid in id_list:
        char = character_to_manifest(conn, cid)
        if char:
            by_id[cid] = char
    # Prefer allowlist order first
    ordered = [by_id[i] for i in id_list if i in by_id]
    rest = [c for i, c in by_id.items() if i not in id_list]
    bundled_manifest["characters"] = ordered + rest
    if ordered and not bundled_manifest.get("default_id"):
        bundled_manifest["default_id"] = ordered[0]["id"]
    # Keep stable default_id if still present
    if bundled_manifest.get("default_id") not in {c["id"] for c in bundled_manifest["characters"]}:
        if ordered:
            bundled_manifest["default_id"] = ordered[0]["id"]
    bundled_manifest_path.parent.mkdir(parents=True, exist_ok=True)
    bundled_manifest_path.write_text(
        json.dumps(bundled_manifest, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )
    conn.close()
    return {
        "ok": True,
        "bundled_db": str(out_db),
        "allowlist": id_list,
        "counts": counts,
        "manifest": str(bundled_manifest_path),
        "note": "models under bundled/roster/pet-models are not copied by this command",
    }

def cmd_publish_bundled(args: argparse.Namespace) -> int:
    result = run_publish_bundled(
        db=Path(args.db) if args.db else None,
        ids=args.ids or "",
    )
    emit(result)
    return 0 if result.get("ok") else 1

def cmd_export_pack(args: argparse.Namespace) -> int:
    """Export a user data pack (subset) — not the full private local DB."""
    db = Path(args.db) if args.db else default_local_db()
    if not db.is_file():
        emit({"ok": False, "error": f"local db missing: {db}"})
        return 1
    ids = [x.strip() for x in (args.ids or "").split(",") if x.strip()]
    if not ids:
        emit({"ok": False, "error": "--ids required (never export entire private DB)"})
        return 1
    out = Path(args.output)
    out.parent.mkdir(parents=True, exist_ok=True)
    tmp = out.with_suffix(".sqlite.tmp")
    counts = copy_subset_db(db, tmp, ids)
    with zipfile.ZipFile(out, "w", compression=zipfile.ZIP_DEFLATED) as zf:
        zf.write(tmp, arcname="handaily-roster.sqlite")
        zf.writestr(
            "README.txt",
            "Handaily roster pack\n"
            f"characters: {', '.join(ids)}\n"
            "Import via future pack import; do not overwrite other users' private data.\n",
        )
    tmp.unlink(missing_ok=True)
    emit({"ok": True, "pack": str(out), "ids": ids, "counts": counts})
    return 0

def cmd_verify(args: argparse.Namespace) -> int:
    db = Path(args.db) if args.db else default_local_db()
    allow = load_json(allowlist_path(), {"character_ids": []})
    allow_ids = set(allow.get("character_ids") or [])
    bundled_db = bundled_roster_dir() / "handaily-roster.sqlite"
    checks = []

    # private db must not live under hanpet/bundled except subset
    private_in_tree = list(repo_root().glob("**/data/roster/handaily-roster.sqlite"))
    checks.append(
        {
            "name": "private_db_path",
            "ok": all("bundled" not in str(p).replace("\\", "/") for p in private_in_tree),
            "paths": [str(p) for p in private_in_tree],
        }
    )

    if db.is_file():
        conn = connect(db)
        n = conn.execute("SELECT count(*) FROM characters").fetchone()[0]
        checks.append({"name": "local_characters", "ok": n > 0, "count": n})
        conn.close()
    else:
        checks.append({"name": "local_db_exists", "ok": False, "path": str(db)})

    if bundled_db.is_file():
        conn = connect(bundled_db)
        rows = [r[0] for r in conn.execute("SELECT id FROM characters").fetchall()]
        extra = [i for i in rows if i not in allow_ids]
        checks.append(
            {
                "name": "bundled_subset_of_allowlist",
                "ok": len(extra) == 0 and len(rows) > 0,
                "bundled_ids": rows,
                "extra": extra,
                "allowlist": sorted(allow_ids),
            }
        )
        conn.close()
    else:
        checks.append({"name": "bundled_db_exists", "ok": False, "path": str(bundled_db)})

    # gitignore presence
    gi = (repo_root() / "data" / ".gitignore").read_text(encoding="utf-8")
    checks.append(
        {
            "name": "gitignore_sqlite",
            "ok": "roster/*.sqlite" in gi or "handaily-roster" in gi,
        }
    )

    ok = all(c.get("ok") for c in checks)
    emit({"ok": ok, "checks": checks})
    return 0 if ok else 2

# --- cli ---
def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--db", type=Path, default=None)
    sub = ap.add_subparsers(dest="cmd", required=True)

    p_init = sub.add_parser("init")
    p_init.add_argument("--force", action="store_true")

    p_seed = sub.add_parser("import-bundled-seed")

    p_imp = sub.add_parser("import-wiki")
    p_imp.add_argument(
        "--wiki-db",
        type=Path,
        default=None,
        help="BWIKI sqlite (default: path_policy.default_wiki_db())",
    )
    p_imp.add_argument(
        "--unpacked",
        type=Path,
        default=repo_root() / "data/skin",
    )
    p_imp.add_argument(
        "--en-map",
        type=Path,
        default=repo_root() / "data/wiki/ship-en-names.json",
    )
    p_imp.add_argument(
        "--ids",
        type=str,
        default="",
        help="仅同步这些角色 id（逗号分隔），默认全部",
    )
    p_imp.add_argument(
        "--scope",
        choices=("all", "unpacked"),
        default="all",
        help="all=Wiki 全舰船；unpacked=仅已解包目录",
    )

    p_sync = sub.add_parser("sync-appdata")
    p_sync.add_argument("--data-dir", type=Path, default=None)
    p_sync.add_argument("--ids", type=str, default="")
    p_sync.add_argument("--force-lines", action="store_true")
    p_sync.add_argument(
        "--merge",
        action="store_true",
        help="合并进 AppData 现有角色（默认改为覆盖：仅保留本次同步的自用库角色）",
    )

    p_pub = sub.add_parser("publish-bundled")
    p_pub.add_argument("--ids", type=str, default="", help="override allowlist")

    p_pack = sub.add_parser("export-pack")
    p_pack.add_argument("--ids", type=str, required=True)
    p_pack.add_argument("-o", "--output", type=Path, required=True)

    sub.add_parser("verify")
    sub.add_parser(
        "repair-l2d-binds",
        help="Merge leftover L2D-{N} into Wiki skin{N-1} (slug_N folder convention)",
    )

    args = ap.parse_args()
    if args.cmd == "init":
        return cmd_init(args)
    if args.cmd == "import-bundled-seed":
        return cmd_import_bundled_seed(args)
    if args.cmd == "import-wiki":
        return cmd_import_wiki(args)
    if args.cmd == "sync-appdata":
        return cmd_sync_appdata(args)
    if args.cmd == "publish-bundled":
        return cmd_publish_bundled(args)
    if args.cmd == "export-pack":
        return cmd_export_pack(args)
    if args.cmd == "verify":
        return cmd_verify(args)
    if args.cmd == "repair-l2d-binds":
        return cmd_repair_l2d_binds(args)
    return 1

