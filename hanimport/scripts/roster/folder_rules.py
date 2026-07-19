"""Pure pet/skin folder naming rules (no DB IO)."""
from __future__ import annotations

import re

SKIN_SUFFIX = re.compile(
    r"_(?:\d+|h|g|hx|doa|painting|idol|younv|summer|school|winter|swimsuit|wedding|newyear|cn|jp|en|super|asmr)$",
    re.I,
)

META_SUFFIX_RE = re.compile(
    r"(?:[·・‧\.．]\s*)?META\s*$",
    re.IGNORECASE,
)

MIDDLE_DOT_RE = re.compile(r"[·・‧]")

YI_NUM_NAME_RE = re.compile(r"^伊(\d+)$")

U_BOAT_NAME_RE = re.compile(r"^U-(\d+)$", re.I)

MU_IDOL_NAME_RE = re.compile(r"^(.+)\(μ兵装\)$")

IDOL_SLUG_BY_ADULT_CN: dict[str, str] = {
    "希佩尔海军上将": "xipeier",
}

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
