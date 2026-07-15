"""Import skins replaces prior wrong rows (not leave-incremental orphans)."""
from __future__ import annotations

from pathlib import Path

from roster_db import (
    _upsert_skins_from_slots,
    apply_schema,
    connect,
    skin_db_id,
    upsert_character,
    upsert_skin,
)


def test_upsert_skins_from_slots_deletes_wrong_skins(tmp_path: Path):
    db = tmp_path / "t.sqlite"
    conn = connect(db)
    apply_schema(conn)
    upsert_character(conn, {"id": "cheshire", "name_zh": "柴郡", "name_en": "Cheshire"})
    upsert_skin(
        conn,
        {
            "id": skin_db_id("cheshire", "default"),
            "character_id": "cheshire",
            "name_zh": "默认",
            "is_default": True,
            "lines": [{"category": "x", "text": "旧"}],
        },
        replace_lines=True,
    )
    upsert_skin(
        conn,
        {
            "id": "cheshire-skin57",
            "character_id": "cheshire",
            "name_zh": "柴郡换装.jpg",
            "is_default": False,
            "lines": [{"category": "y", "text": "脏数据"}],
        },
        replace_lines=True,
    )
    upsert_skin(
        conn,
        {
            "id": "cheshire-retrofit",
            "character_id": "cheshire",
            "name_zh": "改造",
            "is_default": False,
            "lines": [],
        },
        replace_lines=False,
    )

    keep = _upsert_skins_from_slots(
        conn,
        "cheshire",
        [
            {"key": "default", "label": "通常", "kind": "default", "sort_order": 0},
            {"key": "skin1", "label": "猫猫的茶会", "kind": "skin", "sort_order": 1},
            {"key": "oath", "label": "誓约", "kind": "oath", "sort_order": 2},
        ],
    )

    ids = {
        r[0]
        for r in conn.execute(
            "SELECT id FROM skins WHERE character_id=?", ("cheshire",)
        )
    }
    assert ids == {
        "cheshire-default",
        "cheshire-skin1",
        "cheshire-oath",
    }
    assert keep == ids
    assert "cheshire-skin57" not in ids
    leftover = conn.execute(
        "SELECT count(*) FROM skin_lines WHERE skin_id='cheshire-skin57'"
    ).fetchone()[0]
    assert leftover == 0
    conn.close()
