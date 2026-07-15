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
        "POST", "/api/roster/ops/publish-bundled", {"db": "bundled"}, {"confirm_bundled": True}
    )
    assert code == 400


def test_resolve_path_local_env(tmp_path: Path, monkeypatch: pytest.MonkeyPatch):
    db = tmp_path / "x.sqlite"
    monkeypatch.setenv("HANDAILY_ROSTER_DB", str(db))
    assert resolve_path("local") == db
