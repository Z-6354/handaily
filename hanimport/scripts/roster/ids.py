"""Roster id / alias / skin-label helpers (sheared from db.py C1)."""

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

