"""TDD: enrich unpacked pinyin stubs with Wiki Chinese names; merge hash twins."""
from __future__ import annotations

import sqlite3
from pathlib import Path

from roster_db import (
    HASH_PERSONA_ID,
    apply_schema,
    connect,
    enrich_unpacked_character_names,
    merge_roster_duplicates_by_name,
    upsert_character,
    upsert_skin,
)


def _wiki_catalog(tmp_path: Path) -> Path:
    db = tmp_path / "wiki.sqlite"
    conn = sqlite3.connect(db)
    conn.execute(
        """
        CREATE TABLE catalog (
          wiki_title TEXT, wiki_path TEXT, display_name TEXT,
          aliases_json TEXT, avatar_url TEXT, rarity TEXT, faction TEXT,
          ship_type TEXT, created_at TEXT, updated_at TEXT
        )
        """
    )
    conn.execute(
        "INSERT INTO catalog(wiki_title, display_name, avatar_url) VALUES (?,?,?)",
        ("艾伦·萨姆纳", "艾伦·萨姆纳", "https://patchwiki.biligame.com/images/blhx/a.jpg"),
    )
    conn.execute(
        "INSERT INTO catalog(wiki_title, display_name, avatar_url) VALUES (?,?,?)",
        ("白龙", "白龙", "https://patchwiki.biligame.com/images/blhx/b.jpg"),
    )
    conn.commit()
    conn.close()
    return db


def test_enrich_unpacked_character_names(tmp_path: Path):
    wiki = _wiki_catalog(tmp_path)
    db = tmp_path / "roster.sqlite"
    conn = connect(db)
    apply_schema(conn)
    upsert_character(
        conn, {"id": "ailunsamuna", "name_zh": "ailunsamuna", "source": "unpacked"}
    )
    upsert_character(conn, {"id": "bailong", "name_zh": "bailong", "source": "unpacked"})
    conn.commit()

    n = enrich_unpacked_character_names(conn, wiki)
    assert n >= 2
    row = conn.execute(
        "SELECT name_zh, wiki_title FROM characters WHERE id='ailunsamuna'"
    ).fetchone()
    assert row["name_zh"] == "艾伦·萨姆纳"
    assert row["wiki_title"] == "艾伦·萨姆纳"
    row2 = conn.execute(
        "SELECT name_zh FROM characters WHERE id='bailong'"
    ).fetchone()
    assert row2["name_zh"] == "白龙"
    conn.close()


def test_merge_prefers_pinyin_over_hash(tmp_path: Path):
    wiki = _wiki_catalog(tmp_path)
    db = tmp_path / "roster.sqlite"
    conn = connect(db)
    apply_schema(conn)
    upsert_character(
        conn, {"id": "ailunsamuna", "name_zh": "ailunsamuna", "source": "unpacked"}
    )
    upsert_skin(
        conn,
        {
            "id": "ailunsamuna-default",
            "character_id": "ailunsamuna",
            "name_zh": "默认",
            "is_default": True,
            "kanmusu_dir": "ailunsamuna",
            "lines": [],
        },
        replace_lines=False,
    )
    upsert_character(
        conn,
        {
            "id": "p630d1901",
            "name_zh": "艾伦·萨姆纳",
            "wiki_title": "艾伦·萨姆纳",
            "source": "wiki",
        },
    )
    upsert_skin(
        conn,
        {
            "id": "p630d1901-default",
            "character_id": "p630d1901",
            "name_zh": "默认",
            "is_default": True,
            "lines": [],
        },
        replace_lines=False,
    )
    conn.commit()

    enrich_unpacked_character_names(conn, wiki)
    n = merge_roster_duplicates_by_name(conn)
    conn.commit()
    assert n >= 1
    ids = {r[0] for r in conn.execute("SELECT id FROM characters")}
    assert "ailunsamuna" in ids
    assert "p630d1901" not in ids
    assert HASH_PERSONA_ID.match("p630d1901")
    conn.close()
