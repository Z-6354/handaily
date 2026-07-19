"""Tests for handaily-skin-slot local pack/unpack."""
from __future__ import annotations

import json
import sys
import zipfile
from pathlib import Path

import pytest

SCRIPTS = Path(__file__).resolve().parents[1] / "scripts"
if str(SCRIPTS) not in sys.path:
    sys.path.insert(0, str(SCRIPTS))

from roster.db import apply_schema, connect, upsert_character, upsert_skin  # noqa: E402
from roster.skin_probe import _has_spine_assets  # noqa: E402
from roster.skin_slot_pack import (  # noqa: E402
    FORMAT,
    SkipReason,
    build_manifest,
    check_slot_eligible,
    lines_from_db,
    pack_slot,
    slot_zip_name,
    unpack_slot,
)


def _make_pet(folder: Path) -> None:
    folder.mkdir(parents=True, exist_ok=True)
    (folder / "demo.skel").write_bytes(b"skel")
    (folder / "demo.atlas").write_text("demo.png\nsize:1,1\n", encoding="utf-8")
    (folder / "demo.png").write_bytes(b"\x89PNG\r\n\x1a\n")


def _make_skin(folder: Path) -> None:
    folder.mkdir(parents=True, exist_ok=True)
    (folder / "demo.model3.json").write_text("{}", encoding="utf-8")
    (folder / "demo.moc3").write_bytes(b"moc3")


def _seed(conn, *, pet_id: str = "demo_pet", km: str = "") -> None:
    upsert_character(
        conn,
        {
            "id": "cheshire",
            "name_zh": "柴郡",
            "name_en": "Cheshire",
            "faction": "皇家",
            "wiki_title": "柴郡",
            "source": "test",
        },
    )
    upsert_skin(
        conn,
        {
            "id": "cheshire-default",
            "character_id": "cheshire",
            "name_zh": "默认皮肤",
            "is_default": 1,
            "pet_model_id": pet_id,
            "kanmusu_dir": km,
        },
        replace_lines=True,
    )
    conn.execute(
        """
        INSERT INTO skin_lines(skin_id, wiki_key, label, lang, text, animation, sort_order)
        VALUES (?, ?, ?, ?, ?, ?, ?)
        """,
        ("cheshire-default", "main_1", "主界面", "zh", "你好", "", 1),
    )
    conn.commit()


def test_slot_zip_name():
    assert slot_zip_name("cheshire", "cheshire-default") == (
        "cheshire__cheshire-default.slot.zip"
    )
    with pytest.raises(ValueError):
        slot_zip_name("../x", "a")


def test_check_skips_unbound_and_skin_only(tmp_path: Path):
    pet_root = tmp_path / "pet"
    skin_root = tmp_path / "skin"
    pet_root.mkdir()
    skin_root.mkdir()

    skip, _, _ = check_slot_eligible(
        {"pet_model_id": "", "kanmusu_dir": ""},
        pet_root=pet_root,
        skin_root=skin_root,
    )
    assert skip and skip.code == "unbound"

    skip, _, _ = check_slot_eligible(
        {"pet_model_id": "", "kanmusu_dir": "foo"},
        pet_root=pet_root,
        skin_root=skin_root,
    )
    assert skip and skip.code == "skin_only"

    skip, _, _ = check_slot_eligible(
        {"pet_model_id": "missing", "kanmusu_dir": ""},
        pet_root=pet_root,
        skin_root=skin_root,
    )
    assert skip and skip.code == "no_pet"


def test_build_manifest_and_lines(tmp_path: Path):
    db = tmp_path / "r.sqlite"
    conn = connect(db)
    apply_schema(conn)
    _seed(conn, pet_id="demo_pet", km="")
    lines = lines_from_db(conn, "cheshire-default")
    assert lines[0]["text"] == "你好"
    assert "audio_url" not in lines[0]

    m = build_manifest(
        {
            "id": "cheshire",
            "name_zh": "柴郡",
            "name_en": "Cheshire",
            "faction": "皇家",
            "wiki_title": "柴郡",
        },
        {
            "id": "cheshire-default",
            "name_zh": "默认皮肤",
            "is_default": 1,
            "pet_model_id": "demo_pet",
            "kanmusu_dir": "",
        },
        has_pet=True,
        has_kanmusu=False,
        packed_at="2026-07-19T00:00:00Z",
    )
    assert m["format"] == FORMAT
    assert m["skin"]["has_pet"] is True
    assert m["skin"]["has_kanmusu"] is False
    assert m["skin"]["is_oath"] is False
    conn.close()


def test_pack_and_unpack_roundtrip(tmp_path: Path):
    pet_root = tmp_path / "pet"
    skin_root = tmp_path / "skin"
    avatar_dir = tmp_path / "avatars"
    out_dir = tmp_path / "out"
    dest = tmp_path / "dest"
    _make_pet(pet_root / "demo_pet")
    _make_skin(skin_root / "demo_skin")
    avatar_dir.mkdir()
    (avatar_dir / "cheshire.webp").write_bytes(b"WEBP")

    db = tmp_path / "r.sqlite"
    conn = connect(db)
    apply_schema(conn)
    _seed(conn, pet_id="demo_pet", km="demo_skin")

    result = pack_slot(
        conn,
        "cheshire-default",
        pet_root=pet_root,
        skin_root=skin_root,
        out_dir=out_dir,
        avatar_dir=avatar_dir,
    )
    assert result.skipped is None
    assert result.path is not None
    assert result.path.name == "cheshire__cheshire-default.slot.zip"

    with zipfile.ZipFile(result.path) as zf:
        names = set(zf.namelist())
        assert "manifest.json" in names
        assert "lines.json" in names
        assert "avatar.webp" in names
        assert any(n.startswith("pet/demo_pet/") for n in names)
        assert any(n.startswith("skin/demo_skin/") for n in names)

    manifest = unpack_slot(result.path, dest_root=dest)
    assert manifest["character"]["name_zh"] == "柴郡"
    assert _has_spine_assets(dest / "pet" / "demo_pet")
    assert (dest / "skin" / "demo_skin" / "demo.model3.json").is_file()
    assert (dest / "avatars" / "cheshire.webp").read_bytes() == b"WEBP"
    assert (dest / "lines" / "cheshire-default.json").is_file()
    conn.close()


def test_pack_skips_unbound(tmp_path: Path):
    db = tmp_path / "r.sqlite"
    conn = connect(db)
    apply_schema(conn)
    _seed(conn, pet_id="", km="")
    # overwrite pet empty
    conn.execute(
        "UPDATE skins SET pet_model_id='', kanmusu_dir='' WHERE id=?",
        ("cheshire-default",),
    )
    conn.commit()
    r = pack_slot(
        conn,
        "cheshire-default",
        pet_root=tmp_path / "pet",
        skin_root=tmp_path / "skin",
        out_dir=tmp_path / "out",
    )
    assert r.path is None
    assert isinstance(r.skipped, SkipReason)
    assert r.skipped.code == "unbound"
    conn.close()
