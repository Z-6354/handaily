"""TDD: folder-like character ids (abeikelongbi_3) must not stay as characters."""
from __future__ import annotations

from pathlib import Path

import pytest

from roster_db import (
    apply_schema,
    connect,
    is_folder_like_character_id,
    purge_folder_like_characters,
    upsert_character,
    upsert_skin,
)


def test_is_folder_like_character_id():
    assert is_folder_like_character_id("abeikelongbi_3")
    assert is_folder_like_character_id("abeikelongbi_3_hx")
    assert is_folder_like_character_id("z23_hx")
    assert is_folder_like_character_id("ship_wedding")
    assert not is_folder_like_character_id("abeikelongbi")
    assert not is_folder_like_character_id("qiye")
    assert not is_folder_like_character_id("p0123abcd")


def test_purge_folder_like_characters_merges_and_deletes(tmp_path: Path):
    db = tmp_path / "t.sqlite"
    conn = connect(db)
    apply_schema(conn)
    upsert_character(conn, {"id": "abeikelongbi", "name_zh": "阿贝克隆比", "source": "wiki"})
    upsert_skin(
        conn,
        {
            "id": "abeikelongbi-skin3",
            "character_id": "abeikelongbi",
            "name_zh": "换装3",
            "skin_index": 3,
            "kanmusu_dir": "abeikelongbi_3",
            "lines": [],
        },
        replace_lines=False,
    )
    # Simulate legacy dirty rows (bypass upsert guard)
    for dirty in ("abeikelongbi_3", "abeikelongbi_3_hx", "aijiang_3"):
        conn.execute(
            "INSERT INTO characters(id, name_zh, name_en, source, updated_at) "
            "VALUES (?,?,?,?,datetime('now'))",
            (dirty, dirty, "", "unpacked"),
        )
    upsert_character(conn, {"id": "aijiang", "name_zh": "埃吉尔", "source": "wiki"})
    upsert_skin(
        conn,
        {
            "id": "aijiang-skin3",
            "character_id": "aijiang",
            "name_zh": "换装3",
            "skin_index": 3,
            "kanmusu_dir": "",
            "lines": [],
        },
        replace_lines=False,
    )
    conn.execute(
        "INSERT INTO skins(id, character_id, name_zh, name_en, kanmusu_dir, pet_model_id, "
        "sort_order, is_default, meta_json, updated_at) "
        "VALUES (?,?,?,?,?,?,0,0,'{}',datetime('now'))",
        ("aijiang_3-L2D", "aijiang_3", "脏皮", "", "aijiang_3", "aijiang_3"),
    )
    conn.commit()

    n = purge_folder_like_characters(conn)
    conn.commit()
    assert n >= 3

    ids = {r[0] for r in conn.execute("SELECT id FROM characters")}
    assert "abeikelongbi_3" not in ids
    assert "abeikelongbi_3_hx" not in ids
    assert "aijiang_3" not in ids
    assert "abeikelongbi" in ids
    assert "aijiang" in ids

    row = conn.execute(
        "SELECT kanmusu_dir FROM skins WHERE id=?", ("abeikelongbi-skin3",)
    ).fetchone()
    assert row is not None
    assert row[0] == "abeikelongbi_3"
    conn.close()


def test_upsert_character_rejects_folder_like(tmp_path: Path):
    db = tmp_path / "t.sqlite"
    conn = connect(db)
    apply_schema(conn)
    with pytest.raises(ValueError, match="folder-like|skin suffix|_3"):
        upsert_character(
            conn, {"id": "abeikelongbi_3", "name_zh": "bad", "source": "unpacked"}
        )
    conn.close()
