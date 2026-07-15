"""Unpacked folders bind models onto Wiki skins — never create folder-id skins."""
from __future__ import annotations

from pathlib import Path

from roster_db import (
    apply_schema,
    bind_unpacked_models,
    connect,
    skin_db_id,
    upsert_character,
    upsert_skin,
)


def test_bind_does_not_create_folder_skin(tmp_path: Path):
    db = tmp_path / "t.sqlite"
    conn = connect(db)
    apply_schema(conn)
    upsert_character(conn, {"id": "aijiang", "name_zh": "埃吉尔", "name_en": "Ägir"})
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
            "id": "aijiang-skin3",
            "character_id": "aijiang",
            "name_zh": "换装3",
            "skin_index": 3,
            "is_default": False,
            "lines": [],
        },
        replace_lines=False,
    )
    conn.commit()

    unpacked = tmp_path / "unpacked"
    (unpacked / "aijiang").mkdir(parents=True)
    (unpacked / "aijiang_3").mkdir(parents=True)
    (unpacked / "aijier_3").mkdir(parents=True)  # alias base

    pet = tmp_path / "pet-models"
    (pet / "aijiang_3").mkdir(parents=True)

    n = bind_unpacked_models(
        conn,
        unpacked,
        pet_models=pet,
        alias_map={"aijiang": "埃吉尔", "aijier": "埃吉尔"},
        cn_to_slug={"埃吉尔": "aijiang"},
    )
    assert n >= 1

    ids = {
        r[0]
        for r in conn.execute("SELECT id FROM skins WHERE character_id=?", ("aijiang",))
    }
    assert "aijiang_3" not in ids
    assert "aijier_3" not in ids
    assert ids == {"aijiang-default", "aijiang-skin3"}

    row = conn.execute(
        "SELECT kanmusu_dir, pet_model_id FROM skins WHERE id=?", ("aijiang-skin3",)
    ).fetchone()
    assert row is not None
    assert row[0] in ("aijiang_3", "aijier_3")
    conn.close()
