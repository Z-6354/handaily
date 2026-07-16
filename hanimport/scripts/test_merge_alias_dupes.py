"""Merge same-name alias duplicates (aijiang / aijier → 埃吉尔)."""
from __future__ import annotations

from pathlib import Path

from roster_db import (
    LIVE2D_ALIASES,
    apply_schema,
    connect,
    merge_roster_duplicates_by_name,
    preferred_slug_for_cn,
    upsert_character,
    upsert_skin,
)


def test_preferred_slug_aegir():
    assert preferred_slug_for_cn("埃吉尔", LIVE2D_ALIASES) == "aijiang"


def test_merge_aijiang_aijier(tmp_path: Path):
    db = tmp_path / "t.sqlite"
    conn = connect(db)
    apply_schema(conn)
    upsert_character(conn, {"id": "aijiang", "name_zh": "埃吉尔", "source": "wiki"})
    upsert_character(conn, {"id": "aijier", "name_zh": "埃吉尔", "source": "wiki"})
    upsert_skin(
        conn,
        {
            "id": "aijiang-default",
            "character_id": "aijiang",
            "name_zh": "默认",
            "is_default": True,
            "lines": [{"text": "a"}],
        },
        replace_lines=True,
    )
    upsert_skin(
        conn,
        {
            "id": "aijier-default",
            "character_id": "aijier",
            "name_zh": "默认",
            "is_default": True,
            "kanmusu_dir": "aijier",
            "lines": [{"text": "b"}],
        },
        replace_lines=True,
    )
    n = merge_roster_duplicates_by_name(conn, LIVE2D_ALIASES)
    assert n >= 1
    ids = {r[0] for r in conn.execute("SELECT id FROM characters")}
    assert ids == {"aijiang"}
    assert "aijier" not in ids
    skins = list(conn.execute("SELECT id, kanmusu_dir FROM skins WHERE character_id='aijiang'"))
    assert any(s[0] == "aijiang-default" for s in skins)
    # donor bind preserved onto canon default if empty
    row = conn.execute(
        "SELECT kanmusu_dir FROM skins WHERE id='aijiang-default'"
    ).fetchone()
    assert row is not None
    assert row[0] == "aijier"
    conn.close()
