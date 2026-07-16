"""Incremental skins sync + purge folder-like dirty skins."""
from __future__ import annotations

from pathlib import Path

from roster_db import (
    apply_schema,
    character_skins_in_sync,
    connect,
    expected_skin_ids_from_slots,
    list_character_ids_needing_lines,
    purge_folder_like_skins,
    skin_db_id,
    upsert_character,
    upsert_skin,
)


def test_purge_folder_like_skins(tmp_path: Path):
    db = tmp_path / "t.sqlite"
    conn = connect(db)
    apply_schema(conn)
    upsert_character(conn, {"id": "aijiang", "name_zh": "埃吉尔"})
    upsert_skin(
        conn,
        {
            "id": skin_db_id("aijiang", "default"),
            "character_id": "aijiang",
            "name_zh": "默认",
            "is_default": True,
            "lines": [],
        },
        replace_lines=False,
    )
    upsert_skin(
        conn,
        {
            "id": "aijier_3",
            "character_id": "aijiang",
            "name_zh": "脏",
            "is_default": False,
            "lines": [{"category": "x", "text": "y"}],
        },
        replace_lines=True,
    )
    n = purge_folder_like_skins(conn)
    assert n == 1
    ids = {r[0] for r in conn.execute("SELECT id FROM skins")}
    assert ids == {"aijiang-default"}
    assert (
        conn.execute(
            "SELECT count(*) FROM skin_lines WHERE skin_id='aijier_3'"
        ).fetchone()[0]
        == 0
    )
    conn.close()


def test_character_skins_in_sync(tmp_path: Path):
    db = tmp_path / "t.sqlite"
    conn = connect(db)
    apply_schema(conn)
    upsert_character(conn, {"id": "c", "name_zh": "测"})
    slots = [
        {"key": "default", "kind": "default", "label": "通常"},
        {"key": "skin1", "kind": "skin", "label": "换装1"},
    ]
    keep = expected_skin_ids_from_slots("c", slots)
    assert keep == {"c-default", "c-skin1"}
    for sid in keep:
        upsert_skin(
            conn,
            {
                "id": sid,
                "character_id": "c",
                "name_zh": sid,
                "is_default": sid.endswith("-default"),
                "lines": [],
            },
            replace_lines=False,
        )
    assert character_skins_in_sync(conn, "c", slots)
    upsert_skin(
        conn,
        {
            "id": "c-skin99",
            "character_id": "c",
            "name_zh": "extra",
            "is_default": False,
            "lines": [],
        },
        replace_lines=False,
    )
    assert not character_skins_in_sync(conn, "c", slots)
    conn.close()


def test_list_needing_lines_empty(tmp_path: Path):
    db = tmp_path / "t.sqlite"
    conn = connect(db)
    apply_schema(conn)
    upsert_character(conn, {"id": "c", "name_zh": "测"})
    upsert_skin(
        conn,
        {
            "id": "c-default",
            "character_id": "c",
            "name_zh": "默认",
            "is_default": True,
            "meta_json": '{"lines_import":{"status":"empty"}}',
            "lines": [],
        },
        replace_lines=False,
    )
    assert list_character_ids_needing_lines(conn) == ["c"]
    conn.close()
