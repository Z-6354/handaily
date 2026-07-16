"""Post-pipeline validation: Wiki skins coverage + local alignment + unmatched lines."""
from __future__ import annotations

import json
import sqlite3
from pathlib import Path
from typing import Any

from roster_db import (
    character_skins_in_sync,
    connect,
)


def _wiki_slots_map(wiki_db: Path) -> dict[str, list[dict]]:
    if not wiki_db.is_file():
        return {}
    conn = sqlite3.connect(str(wiki_db))
    conn.row_factory = sqlite3.Row
    out: dict[str, list[dict]] = {}
    try:
        cols = {r[1] for r in conn.execute("PRAGMA table_info(ships)")}
        if "skins_json" not in cols:
            return {}
        for row in conn.execute(
            "SELECT wiki_title, display_name, skins_json FROM ships"
        ):
            try:
                slots = json.loads(row["skins_json"] or "[]")
            except json.JSONDecodeError:
                slots = []
            if not isinstance(slots, list):
                slots = []
            for key in (row["wiki_title"], row["display_name"]):
                if key:
                    out[str(key)] = slots
    finally:
        conn.close()
    return out


def validate_roster_wiki_state(
    roster_db: Path,
    wiki_db: Path,
    *,
    min_skins_json_pct: float = 95.0,
    min_aligned_pct: float = 95.0,
) -> dict[str, Any]:
    """Summarize coverage / skin id alignment / unmatched lines after pipeline."""
    slots_map = _wiki_slots_map(wiki_db)
    conn = connect(roster_db)
    try:
        chars = conn.execute(
            "SELECT id, name_zh, wiki_title FROM characters ORDER BY id"
        ).fetchall()
        total = len(chars)
        with_slots = 0
        aligned = 0
        misaligned_samples: list[str] = []
        for cid, name_zh, wiki_title in chars:
            key = (wiki_title or name_zh or "").strip()
            slots = slots_map.get(key) or slots_map.get(str(name_zh or ""))
            if not slots:
                continue
            with_slots += 1
            if character_skins_in_sync(conn, str(cid), slots):
                aligned += 1
            elif len(misaligned_samples) < 5:
                misaligned_samples.append(f"{name_zh or cid}")

        unmatched = 0
        empty = 0
        ready = 0
        for sid, meta_raw in conn.execute("SELECT id, meta_json FROM skins"):
            status = ""
            try:
                meta = json.loads(meta_raw or "{}") or {}
                status = str((meta.get("lines_import") or {}).get("status") or "")
            except json.JSONDecodeError:
                status = ""
            n = conn.execute(
                "SELECT count(*) FROM skin_lines WHERE skin_id=?", (sid,)
            ).fetchone()[0]
            if status == "unmatched":
                unmatched += 1
            elif status == "ready" or (not status and n > 0):
                ready += 1
            elif status in ("empty", "stale_flat") or n == 0:
                empty += 1

        skins_pct = round(100.0 * with_slots / total, 1) if total else 0.0
        aligned_pct = (
            round(100.0 * aligned / with_slots, 1) if with_slots else 100.0
        )
        ok = skins_pct >= min_skins_json_pct and aligned_pct >= min_aligned_pct
        samples = list(misaligned_samples)
        if not ok and unmatched and len(samples) < 5:
            # pad with unmatched skin ids
            for (sid,) in conn.execute(
                """
                SELECT id FROM skins
                WHERE meta_json LIKE '%"status": "unmatched"%'
                   OR meta_json LIKE '%"status":"unmatched"%'
                LIMIT 5
                """
            ):
                if len(samples) >= 5:
                    break
                if sid not in samples:
                    samples.append(str(sid))

        return {
            "ok": ok,
            "chars": total,
            "wiki_skins_json_chars": with_slots,
            "wiki_skins_json_pct": skins_pct,
            "aligned_with_slots": aligned,
            "aligned_pct": aligned_pct,
            "lines_ready": ready,
            "lines_empty": empty,
            "unmatched_skins": unmatched,
            "samples": samples,
            "summary": (
                f"验收：skins_json {skins_pct}% · 皮对齐 {aligned_pct}% · "
                f"台词就绪 {ready} · 无台词 {empty} · 未匹配 {unmatched}"
                + (" · 通过" if ok else " · 未达标")
            ),
        }
    finally:
        conn.close()
