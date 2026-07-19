from pathlib import Path

import pytest

from roster_db import normalize_name_en, connect, apply_schema, fill_english_names
from roster_api import handle, resolve_path, require_bundled_confirm, require_write


def test_normalize_name_en():
    assert normalize_name_en("", "cheshire") == "cheshire"
    assert normalize_name_en("  ", "edu") == "edu"
    assert normalize_name_en("Cheshire", "cheshire") == "Cheshire"


def test_fill_english(tmp_path: Path):
    db = tmp_path / "t.sqlite"
    conn = connect(db)
    apply_schema(conn)
    conn.execute(
        "INSERT INTO characters(id,name_zh,name_en) VALUES (?,?,?)",
        ("edu", "恶毒", ""),
    )
    conn.commit()
    n = fill_english_names(conn)
    assert n["characters"] >= 1
    en = conn.execute("SELECT name_en FROM characters WHERE id='edu'").fetchone()[0]
    assert en == "edu"


def test_require_bundled_confirm():
    assert require_bundled_confirm("local", {}) is None
    assert require_bundled_confirm("bundled", {"confirm_bundled": True}) is None
    err = require_bundled_confirm("bundled", {})
    assert err and "confirm_bundled" in err
    assert require_write("bundled", {}) == err


def test_bundled_write_requires_confirm(tmp_path: Path, monkeypatch: pytest.MonkeyPatch):
    bundled = tmp_path / "bundled"
    bundled.mkdir()
    db = bundled / "handaily-roster.sqlite"
    conn = connect(db)
    apply_schema(conn)
    conn.execute(
        "INSERT INTO characters(id,name_zh,name_en) VALUES (?,?,?)",
        ("x", "X", "x"),
    )
    conn.commit()
    conn.close()

    monkeypatch.setattr("roster_api.bundled_roster_dir", lambda: bundled)
    monkeypatch.setattr("roster_db.bundled_roster_dir", lambda: bundled)

    code, payload = handle("DELETE", "/api/roster/characters/x", {"db": "bundled"}, {})
    assert code == 403
    assert payload.get("ok") is False


def test_local_crud_character(tmp_path: Path, monkeypatch: pytest.MonkeyPatch):
    db = tmp_path / "local.sqlite"
    conn = connect(db)
    apply_schema(conn)
    conn.close()
    monkeypatch.setenv("HANDAILY_ROSTER_DB", str(db))

    code, payload = handle(
        "POST",
        "/api/roster/characters",
        {"db": "local"},
        {"id": "edu", "name_zh": "恶毒", "name_en": ""},
    )
    assert code == 200
    assert payload["ok"] is True

    code, payload = handle("GET", "/api/roster/characters/edu", {"db": "local"}, {})
    assert code == 200
    assert payload["character"]["name_en"] == "edu"
    assert payload["character"]["name_zh"] == "恶毒"

    code, payload = handle(
        "PUT",
        "/api/roster/characters/edu",
        {"db": "local"},
        {"name_zh": "恶毒改", "name_en": "Evil"},
    )
    assert code == 200
    assert payload["character"]["name_zh"] == "恶毒改"
    assert payload["character"]["name_en"] == "Evil"

    code, payload = handle("GET", "/api/roster/characters", {"db": "local", "q": "恶毒"}, {})
    assert code == 200
    assert any(c["id"] == "edu" for c in payload["characters"])

    code, payload = handle("DELETE", "/api/roster/characters/edu", {"db": "local"}, {})
    assert code == 200
    code, payload = handle("GET", "/api/roster/characters/edu", {"db": "local"}, {})
    assert code == 404


def test_skins_and_lines_crud(tmp_path: Path, monkeypatch: pytest.MonkeyPatch):
    db = tmp_path / "local.sqlite"
    conn = connect(db)
    apply_schema(conn)
    conn.execute(
        "INSERT INTO characters(id,name_zh,name_en) VALUES (?,?,?)",
        ("edu", "恶毒", "edu"),
    )
    conn.commit()
    conn.close()
    monkeypatch.setenv("HANDAILY_ROSTER_DB", str(db))

    code, payload = handle(
        "POST",
        "/api/roster/skins",
        {"db": "local"},
        {"id": "edu", "character_id": "edu", "name_zh": "默认", "name_en": ""},
    )
    assert code == 200
    assert payload["skin"]["name_en"] == "edu"

    code, payload = handle("GET", "/api/roster/skins/edu/lines", {"db": "local"}, {})
    assert code == 200
    assert payload["lines"] == []

    code, payload = handle(
        "POST",
        "/api/roster/skins/edu/lines",
        {"db": "local"},
        {"text": "指挥官？", "label": "登录"},
    )
    assert code == 200
    line_id = payload["line"]["id"]
    assert line_id

    code, payload = handle(
        "PUT",
        f"/api/roster/lines/{line_id}",
        {"db": "local"},
        {"text": "改台词"},
    )
    assert code == 200
    assert payload["line"]["text"] == "改台词"

    code, payload = handle("DELETE", f"/api/roster/lines/{line_id}", {"db": "local"}, {})
    assert code == 200


def test_list_skins_with_status(tmp_path: Path, monkeypatch: pytest.MonkeyPatch):
    db = tmp_path / "local.sqlite"
    conn = connect(db)
    apply_schema(conn)
    conn.execute(
        "INSERT INTO characters(id,name_zh,name_en) VALUES (?,?,?)",
        ("edu", "恶毒", "edu"),
    )
    conn.execute(
        """
        INSERT INTO skins(id,character_id,name_zh,name_en,pet_model_id,kanmusu_dir,sort_order,is_default)
        VALUES (?,?,?,?,?,?,?,?)
        """,
        ("edu", "edu", "默认", "edu", "edu_pet", "edu_km", 0, 1),
    )
    conn.commit()
    conn.close()
    monkeypatch.setenv("HANDAILY_ROSTER_DB", str(db))

    pet_root = tmp_path / "pet"
    pet_root.mkdir()
    pet = pet_root / "edu_pet"
    pet.mkdir()
    (pet / "edu_pet.skel").write_bytes(b"1")
    (pet / "edu_pet.atlas").write_text("edu_pet.png\nsize:1,1\n", encoding="utf-8")
    (pet / "edu_pet.png").write_bytes(b"\x89PNG\r\n\x1a\n")
    monkeypatch.setenv("HANDAILY_PET_PATH", str(pet_root))
    monkeypatch.setenv("HANDAILY_SKIN_PATH", str(tmp_path / "unpacked"))
    (tmp_path / "unpacked").mkdir()

    code, payload = handle("GET", "/api/roster/skins", {"db": "local", "limit": "10"}, {})
    assert code == 200
    assert payload["total"] == 1
    sk = payload["skins"][0]
    assert sk["pet_status"] == "ready"
    assert sk["kanmusu_status"] == "absent"
    assert sk["character_name_zh"] == "恶毒"

    code, payload = handle(
        "GET", "/api/roster/characters/edu", {"db": "local"}, {}
    )
    assert code == 200
    assert payload["skins"][0]["pet_status"] == "ready"

    code, payload = handle(
        "GET",
        "/api/roster/skins",
        {"db": "local", "filter": "missing"},
        {},
    )
    assert code == 200
    # Default kanmusu empty → absent (not missing); pet is ready
    assert payload["total"] == 0

    code, payload = handle(
        "GET",
        "/api/roster/skins",
        {"db": "local", "filter": "ready"},
        {},
    )
    assert code == 200
    assert payload["total"] == 1

    code, payload = handle(
        "GET",
        "/api/roster/skins",
        {"db": "local", "filter": "dual_ready"},
        {},
    )
    assert code == 200
    assert payload["total"] == 0


def test_ops_fill_english_and_local_only(tmp_path: Path, monkeypatch: pytest.MonkeyPatch):
    db = tmp_path / "local.sqlite"
    conn = connect(db)
    apply_schema(conn)
    conn.execute(
        "INSERT INTO characters(id,name_zh,name_en) VALUES (?,?,?)",
        ("edu", "恶毒", ""),
    )
    conn.commit()
    conn.close()
    monkeypatch.setenv("HANDAILY_ROSTER_DB", str(db))

    code, payload = handle("POST", "/api/roster/ops/fill-english", {"db": "local"}, {})
    assert code == 200
    assert payload["ok"] is True
    assert payload["filled"]["characters"] >= 1

    code, payload = handle("POST", "/api/roster/ops/import-wiki", {"db": "bundled"}, {"confirm_bundled": True})
    assert code == 400

    code, payload = handle("POST", "/api/roster/ops/sync-appdata", {"db": "bundled"}, {"confirm_bundled": True})
    assert code == 400

    code, payload = handle(
        "POST", "/api/roster/ops/sync-appdata", {"db": "local"}, {"replace": True}
    )
    assert code == 403
    assert "confirm_replace" in str(payload.get("error") or "")

    code, payload = handle(
        "POST", "/api/roster/ops/publish-bundled", {"db": "bundled"}, {"confirm_bundled": True}
    )
    assert code == 400


def test_list_characters_summary(tmp_path: Path, monkeypatch: pytest.MonkeyPatch):
    db = tmp_path / "local.sqlite"
    conn = connect(db)
    apply_schema(conn)
    conn.execute(
        "INSERT INTO characters(id,name_zh,name_en) VALUES (?,?,?)",
        ("edu", "恶毒", "edu"),
    )
    conn.execute(
        """
        INSERT INTO skins(id,character_id,name_zh,name_en,pet_model_id,kanmusu_dir,sort_order,is_default)
        VALUES (?,?,?,?,?,?,?,?)
        """,
        ("edu", "edu", "默认", "edu", "edu_pet", "edu_km", 0, 1),
    )
    conn.commit()
    conn.close()
    monkeypatch.setenv("HANDAILY_ROSTER_DB", str(db))

    pet_root = tmp_path / "pet"
    pet_root.mkdir()
    pet = pet_root / "edu_pet"
    pet.mkdir()
    (pet / "edu_pet.skel").write_bytes(b"1")
    (pet / "edu_pet.atlas").write_text("edu_pet.png\nsize:1,1\n", encoding="utf-8")
    (pet / "edu_pet.png").write_bytes(b"\x89PNG\r\n\x1a\n")
    monkeypatch.setenv("HANDAILY_PET_PATH", str(pet_root))
    monkeypatch.setenv("HANDAILY_SKIN_PATH", str(tmp_path / "unpacked"))
    (tmp_path / "unpacked").mkdir()

    code, payload = handle(
        "GET", "/api/roster/characters", {"db": "local", "summary": "1"}, {}
    )
    assert code == 200
    assert payload["total"] == 1
    ch = payload["characters"][0]
    assert ch["id"] == "edu"
    assert ch["kanmusu_status"] == "absent"
    assert ch["pet_status"] == "ready"
    assert ch["lines_status"] == "empty"
    assert ch["skin_count"] == 1


def test_list_characters_wiki_match(tmp_path: Path, monkeypatch: pytest.MonkeyPatch):
    import sqlite3

    from roster_api import invalidate_list_caches

    wiki = tmp_path / "wiki.sqlite"
    wconn = sqlite3.connect(wiki)
    wconn.execute(
        "CREATE TABLE ships (display_name TEXT, wiki_title TEXT)"
    )
    wconn.execute(
        "INSERT INTO ships(display_name, wiki_title) VALUES (?, ?)",
        ("企业", "企业"),
    )
    wconn.commit()
    wconn.close()
    monkeypatch.setenv("BLHX_WIKI_DB_PATH", str(wiki))
    invalidate_list_caches()

    db = tmp_path / "local.sqlite"
    conn = connect(db)
    apply_schema(conn)
    conn.execute(
        "INSERT INTO characters(id,name_zh,name_en,wiki_title) VALUES (?,?,?,?)",
        ("qiye", "企业", "Enterprise", "企业"),
    )
    conn.execute(
        "INSERT INTO characters(id,name_zh,name_en,wiki_title) VALUES (?,?,?,?)",
        ("orphan_ab", "orphan_ab", "orphan_ab", ""),
    )
    conn.execute(
        "INSERT INTO characters(id,name_zh,name_en,wiki_title) VALUES (?,?,?,?)",
        ("fake", "虚构舰", "fake", ""),
    )
    conn.commit()
    conn.close()
    monkeypatch.setenv("HANDAILY_ROSTER_DB", str(db))

    code, payload = handle(
        "GET",
        "/api/roster/characters",
        {"db": "local", "wiki_match": "known", "limit": "50"},
        {},
    )
    assert code == 200
    ids = {c["id"] for c in payload["characters"]}
    assert ids == {"qiye"}
    assert payload["total"] == 1

    code, payload = handle(
        "GET",
        "/api/roster/characters",
        {"db": "local", "wiki_match": "unknown", "limit": "50"},
        {},
    )
    assert code == 200
    ids = {c["id"] for c in payload["characters"]}
    assert ids == {"orphan_ab", "fake"}
    assert payload["total"] == 2

    code, payload = handle(
        "GET",
        "/api/roster/characters",
        {"db": "local", "wiki_match": "all", "limit": "50"},
        {},
    )
    assert code == 200
    assert payload["total"] == 3


def test_resolve_path_local_env(tmp_path: Path, monkeypatch: pytest.MonkeyPatch):
    db = tmp_path / "x.sqlite"
    monkeypatch.setenv("HANDAILY_ROSTER_DB", str(db))
    assert resolve_path("local") == db


def test_meta_paths(tmp_path: Path, monkeypatch: pytest.MonkeyPatch):
    db = tmp_path / "local.sqlite"
    conn = connect(db)
    apply_schema(conn)
    conn.close()
    monkeypatch.setenv("HANDAILY_ROSTER_DB", str(db))
    pet = tmp_path / "pet"
    pet.mkdir()
    unpacked = tmp_path / "unpacked"
    unpacked.mkdir()
    wiki = tmp_path / "wiki.sqlite"
    wiki.write_bytes(b"")
    monkeypatch.setenv("HANDAILY_PET_PATH", str(pet))
    monkeypatch.setenv("HANDAILY_SKIN_PATH", str(unpacked))
    monkeypatch.setenv("BLHX_WIKI_DB_PATH", str(wiki))

    code, payload = handle("GET", "/api/roster/meta", {"db": "local"}, {})
    assert code == 200
    assert payload.get("ok") is True
    for key in ("roster_db", "live2d", "unpacked", "wiki_db", "wiki_db_exists"):
        assert key in payload, f"missing meta key: {key}"
    assert payload["roster_db"] == str(db.resolve())
    # meta.live2d is the pet root (data/pet); kanmusu uses unpacked/skin
    assert payload["live2d"] == str(pet.resolve())
    assert payload["unpacked"] == str(unpacked.resolve())
    assert payload["wiki_db"] == str(wiki.resolve())
    assert payload["wiki_db_exists"] is True
