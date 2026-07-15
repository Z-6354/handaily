#!/usr/bin/env python3
"""Match Wiki per-skin line groups to roster skins."""
from __future__ import annotations

import json
import re
from datetime import datetime, timezone
from typing import Any


def normalize_skin_label(label: str | None) -> str:
    s = (label or "").strip()
    if not s:
        return ""
    s = s.replace("「", "").replace("」", "").replace("『", "").replace("』", "")
    s = re.sub(r"^【誓约】", "", s)
    s = re.sub(r"^BD特典", "", s)
    s = re.sub(r"^换装", "", s)
    s = re.sub(r"\s+", "", s)
    return s.casefold()


def _is_default_wiki_skin(skin: str, skin_kind: str) -> bool:
    if skin_kind == "default":
        return True
    t = (skin or "").strip()
    return t in ("", "default", "通常", "默认") or t.casefold() == "default"


def score_match(wiki_norm: str, candidate: str | None) -> int:
    """Higher is better; 0 = no match."""
    c = normalize_skin_label(candidate)
    if not wiki_norm or not c:
        return 0
    if wiki_norm == c:
        return 100
    if wiki_norm in c or c in wiki_norm:
        return 80
    return 0


def match_wiki_group_to_skin(
    group: dict[str, Any],
    skins: list[dict[str, Any]],
) -> tuple[dict[str, Any] | None, str | None]:
    """Return (skin_row, matched_by) or (None, None)."""
    skin_key = str(group.get("skin") or "")
    skin_kind = str(group.get("skin_kind") or "other")
    if _is_default_wiki_skin(skin_key, skin_kind):
        for sk in skins:
            if sk.get("is_default") or str(sk.get("id") or "").endswith("-default"):
                return sk, "default"
        if skins:
            return skins[0], "default"
        return None, None

    wiki_norm = normalize_skin_label(skin_key)
    best: tuple[int, dict[str, Any], str] | None = None
    for sk in skins:
        for field, how in (
            ("name_zh", "name_zh"),
            ("kanmusu_dir", "kanmusu_dir"),
            ("pet_model_id", "pet_model_id"),
            ("id", "id"),
        ):
            sc = score_match(wiki_norm, sk.get(field) if isinstance(sk.get(field), str) else None)
            if sc and (best is None or sc > best[0]):
                best = (sc, sk, how)
    if best and best[0] >= 80:
        return best[1], best[2]
    return None, None


def lines_import_meta(
    status: str,
    *,
    wiki_skin: str | None = None,
    matched_by: str | None = None,
) -> str:
    payload = {
        "lines_import": {
            "status": status,
            "wiki_skin": wiki_skin,
            "matched_by": matched_by,
            "updated_at": datetime.now(timezone.utc).isoformat(),
        }
    }
    return json.dumps(payload, ensure_ascii=False)


def merge_meta_json(existing: str | None, lines_import_block: dict[str, Any]) -> str:
    try:
        cur = json.loads(existing or "{}")
        if not isinstance(cur, dict):
            cur = {}
    except json.JSONDecodeError:
        cur = {}
    cur["lines_import"] = lines_import_block
    return json.dumps(cur, ensure_ascii=False)


def apply_lines_by_skin(
    groups: list[dict[str, Any]],
    skins: list[dict[str, Any]],
    lines_rows_fn,
) -> dict[str, Any]:
    """
    Decide assignments without writing DB.

    Returns:
      {
        assignments: [{skin_id, wiki_skin, matched_by, lines, status}],
        wiki_unmatched: [{skin, skin_kind, line_count}],
        roster_unmatched_ids: [skin_id],
      }
    """
    assignments: list[dict[str, Any]] = []
    wiki_unmatched: list[dict[str, Any]] = []
    claimed: set[str] = set()

    for g in groups or []:
        if not isinstance(g, dict):
            continue
        raw_lines = g.get("lines") or []
        rows = lines_rows_fn(raw_lines)
        sk, how = match_wiki_group_to_skin(g, skins)
        wiki_skin = str(g.get("skin") or "")
        if sk is None:
            wiki_unmatched.append(
                {
                    "skin": wiki_skin,
                    "skin_kind": g.get("skin_kind"),
                    "line_count": len(rows),
                }
            )
            continue
        sid = str(sk.get("id") or "")
        claimed.add(sid)
        status = "ready" if rows else "empty"
        assignments.append(
            {
                "skin_id": sid,
                "wiki_skin": wiki_skin,
                "matched_by": how,
                "lines": rows,
                "status": status,
            }
        )

    roster_unmatched = [
        str(sk.get("id") or "")
        for sk in skins
        if str(sk.get("id") or "") and str(sk.get("id") or "") not in claimed
    ]
    return {
        "assignments": assignments,
        "wiki_unmatched": wiki_unmatched,
        "roster_unmatched_ids": roster_unmatched,
    }
