"""TDD: aggregate per-character skin/probe stats for roster filters."""
from __future__ import annotations

from pathlib import Path

from character_assets import (
    aggregate_character_assets,
    best_asset_status,
    dir_mtime,
)
from roster_db import apply_schema, connect, upsert_character, upsert_skin
from skin_probe import STATUS_MISSING, STATUS_READY, STATUS_UNBOUND


def test_best_asset_status():
    assert best_asset_status([]) == STATUS_UNBOUND
    assert best_asset_status([STATUS_UNBOUND]) == STATUS_UNBOUND
    assert best_asset_status([STATUS_UNBOUND, STATUS_MISSING]) == STATUS_MISSING
    assert (
        best_asset_status([STATUS_MISSING, STATUS_READY, STATUS_UNBOUND])
        == STATUS_READY
    )


def test_aggregate_character_assets(tmp_path: Path):
    db = tmp_path / "t.sqlite"
    conn = connect(db)
    apply_schema(conn)
    upsert_character(conn, {"id": "edu", "name_zh": "恶毒", "faction": "鸢尾"})
    upsert_skin(
        conn,
        {
            "id": "edu-default",
            "character_id": "edu",
            "name_zh": "默认",
            "is_default": True,
            "pet_model_id": "",
            "kanmusu_dir": "",
            "lines": [],
        },
        replace_lines=False,
    )
    upsert_skin(
        conn,
        {
            "id": "edu-skin1",
            "character_id": "edu",
            "name_zh": "皮1",
            "pet_model_id": "edu_pet",
            "kanmusu_dir": "edu_km",
            "lines": [],
        },
        replace_lines=False,
    )
    conn.commit()

    live = tmp_path / "live2d"
    km = tmp_path / "unpacked"
    (live / "edu_pet").mkdir(parents=True)
    (live / "edu_pet" / "edu_pet.atlas").write_text("a", encoding="utf-8")
    (live / "edu_pet" / "edu_pet.skel").write_bytes(b"s")
    (km / "edu_km").mkdir(parents=True)
    (km / "edu_km" / "edu_km.moc3").write_bytes(b"m")
    (km / "edu_km" / "edu_km.model3.json").write_text("{}", encoding="utf-8")

    agg = aggregate_character_assets(
        conn, "edu", live2d_roots=[live], kanmusu_root=km
    )
    assert agg["skin_count"] == 2
    assert agg["pet_status"] == STATUS_READY
    assert agg["kanmusu_status"] == STATUS_READY
    assert agg["import_mtime"] is not None
    assert agg["import_mtime"] >= dir_mtime(km / "edu_km")

    # missing pet only
    upsert_skin(
        conn,
        {
            "id": "edu-skin2",
            "character_id": "edu",
            "name_zh": "皮2",
            "pet_model_id": "nope",
            "kanmusu_dir": "",
            "lines": [],
        },
        replace_lines=False,
    )
    conn.commit()
    agg2 = aggregate_character_assets(
        conn, "edu", live2d_roots=[live], kanmusu_root=km
    )
    assert agg2["skin_count"] == 3
    assert agg2["pet_status"] == STATUS_READY  # still has ready from skin1
    assert agg2["kanmusu_status"] == STATUS_READY
    conn.close()


def test_aggregate_all_unbound(tmp_path: Path):
    db = tmp_path / "t.sqlite"
    conn = connect(db)
    apply_schema(conn)
    upsert_character(conn, {"id": "x", "name_zh": "x"})
    conn.commit()
    agg = aggregate_character_assets(
        conn, "x", live2d_roots=[tmp_path / "l"], kanmusu_root=tmp_path / "k"
    )
    assert agg["skin_count"] == 0
    assert agg["pet_status"] == STATUS_UNBOUND
    assert agg["kanmusu_status"] == STATUS_UNBOUND
    assert agg["import_mtime"] is None
    conn.close()
