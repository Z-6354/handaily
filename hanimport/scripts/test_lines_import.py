from pathlib import Path

import pytest

from roster_db import (
    _apply_lines_import,
    apply_schema,
    connect,
    skin_db_id,
    upsert_character,
    upsert_skin,
)


def test_apply_lines_import_per_skin(tmp_path: Path):
    db = tmp_path / "t.sqlite"
    conn = connect(db)
    apply_schema(conn)
    upsert_character(conn, {"id": "edu", "name_zh": "恶毒", "name_en": "edu"})
    upsert_skin(
        conn,
        {
            "id": skin_db_id("edu", "default"),
            "character_id": "edu",
            "name_zh": "默认",
            "is_default": True,
            "lines": [],
        },
        replace_lines=False,
    )
    upsert_skin(
        conn,
        {
            "id": "edu-witch",
            "character_id": "edu",
            "name_zh": "待宵的魔女",
            "is_default": False,
            "lines": [],
        },
        replace_lines=False,
    )
    stats = {
        "skins_lines_ok": 0,
        "skins_lines_empty": 0,
        "wiki_skins_unmatched": 0,
        "roster_skins_unmatched": 0,
        "lines_report": [],
    }
    groups = [
        {
            "skin": "default",
            "skin_kind": "default",
            "lines": [{"key": "login", "text": "默认登录台词", "lang": "zh"}],
        },
        {
            "skin": "待宵的魔女",
            "skin_kind": "skin",
            "lines": [{"key": "login", "text": "魔女登录台词", "lang": "zh"}],
        },
        {
            "skin": "Wiki独有皮",
            "skin_kind": "skin",
            "lines": [{"key": "login", "text": "没人要", "lang": "zh"}],
        },
    ]
    _apply_lines_import(conn, "edu", groups, [], stats)
    conn.commit()

    def texts(sid):
        return [
            r[0]
            for r in conn.execute(
                "SELECT text FROM skin_lines WHERE skin_id=? ORDER BY sort_order", (sid,)
            )
        ]

    assert texts(skin_db_id("edu", "default")) == ["默认登录台词"]
    assert texts("edu-witch") == ["魔女登录台词"]
    assert stats["skins_lines_ok"] == 2
    assert stats["wiki_skins_unmatched"] == 1
    meta = conn.execute(
        "SELECT meta_json FROM skins WHERE id=?", (skin_db_id("edu", "default"),)
    ).fetchone()[0]
    assert "ready" in meta
