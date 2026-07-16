"""Pipeline v2: concurrency clamp + post-run validation."""
from __future__ import annotations

import json
import sqlite3
from pathlib import Path

from wiki_lines_fetch import fetch_concurrency
from wiki_pipeline_validate import validate_roster_wiki_state


def test_fetch_concurrency_clamp(monkeypatch):
    monkeypatch.setenv("BLHX_WIKI_FETCH_CONCURRENCY", "1")
    assert fetch_concurrency() == 1
    monkeypatch.setenv("BLHX_WIKI_FETCH_CONCURRENCY", "9")
    assert fetch_concurrency() == 4
    monkeypatch.setenv("BLHX_WIKI_FETCH_CONCURRENCY", "bad")
    assert fetch_concurrency() == 2


def test_validate_roster_wiki_state(tmp_path: Path):
    wiki = tmp_path / "w.sqlite"
    w = sqlite3.connect(wiki)
    w.execute(
        """
        CREATE TABLE ships (
          wiki_title TEXT PRIMARY KEY,
          display_name TEXT,
          skins_json TEXT,
          lines_by_skin_json TEXT
        )
        """
    )
    w.execute(
        "INSERT INTO ships VALUES (?,?,?,?)",
        (
            "对齐",
            "对齐",
            json.dumps(
                [
                    {"key": "default", "kind": "default", "label": "通常"},
                    {"key": "skin1", "kind": "skin", "label": "换装1"},
                ]
            ),
            json.dumps([{"skin": "default", "lines": [{"text": "a"}]}]),
        ),
    )
    w.execute(
        "INSERT INTO ships VALUES (?,?,?,?)",
        ("缺皮", "缺皮", "[]", "[]"),
    )
    w.commit()
    w.close()

    roster = tmp_path / "r.sqlite"
    from roster_db import apply_schema, connect, upsert_character, upsert_skin

    conn = connect(roster)
    apply_schema(conn)
    upsert_character(conn, {"id": "a", "name_zh": "对齐", "wiki_title": "对齐"})
    upsert_character(conn, {"id": "b", "name_zh": "缺皮", "wiki_title": "缺皮"})
    upsert_skin(
        conn,
        {
            "id": "a-default",
            "character_id": "a",
            "name_zh": "通常",
            "is_default": True,
            "lines": [{"category": "x", "text": "y"}],
        },
        replace_lines=True,
    )
    upsert_skin(
        conn,
        {
            "id": "a-skin1",
            "character_id": "a",
            "name_zh": "换装1",
            "is_default": False,
            "lines": [],
        },
        replace_lines=False,
    )
    # b: no authority skins
    upsert_skin(
        conn,
        {
            "id": "b-default",
            "character_id": "b",
            "name_zh": "默认",
            "is_default": True,
            "meta_json": json.dumps(
                {"lines_import": {"status": "unmatched"}}, ensure_ascii=False
            ),
            "lines": [],
        },
        replace_lines=False,
    )
    conn.commit()
    conn.close()

    report = validate_roster_wiki_state(roster, wiki)
    assert report["chars"] == 2
    assert report["wiki_skins_json_pct"] == 50.0
    assert report["aligned_with_slots"] == 1
    assert report["aligned_pct"] == 100.0  # only among chars that have slots
    assert report["unmatched_skins"] >= 1
    assert report["ok"] is False  # skins coverage < 95
    assert report["samples"]
