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


def test_create_rejects_folder_like_character_id(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
):
    db = tmp_path / "local.sqlite"
    conn = connect(db)
    apply_schema(conn)
    conn.close()
    monkeypatch.setenv("HANDAILY_ROSTER_DB", str(db))

    code, payload = handle(
        "POST",
        "/api/roster/characters",
        {"db": "local"},
        {"id": "abeikelongbi_3", "name_zh": "不应创建"},
    )
    assert code == 400
    assert payload["ok"] is False
    err = payload.get("error") or ""
    assert "abeikelongbi" in err


def test_list_characters_puts_unnamed_stubs_last(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
):
    """无标准舰名（name_zh==id）的角色排在列表末尾。"""
    db = tmp_path / "local.sqlite"
    conn = connect(db)
    apply_schema(conn)
    conn.execute(
        "INSERT INTO characters(id,name_zh,name_en,source) VALUES (?,?,?,?)",
        ("gezi", "gezi", "", "unpacked"),
    )
    conn.execute(
        "INSERT INTO characters(id,name_zh,name_en,source) VALUES (?,?,?,?)",
        ("bailong", "白龙", "", "wiki"),
    )
    conn.execute(
        "INSERT INTO characters(id,name_zh,name_en,source) VALUES (?,?,?,?)",
        ("abeikelongbi", "阿贝克隆比", "", "wiki"),
    )
    conn.execute(
        "INSERT INTO characters(id,name_zh,name_en,source) VALUES (?,?,?,?)",
        ("kubo", "kubo", "", "unpacked"),
    )
    conn.execute(
        "INSERT INTO characters(id,name_zh,name_en,source) VALUES (?,?,?,?)",
        ("z23", "Z23", "", "wiki"),
    )
    conn.commit()
    conn.close()
    monkeypatch.setenv("HANDAILY_ROSTER_DB", str(db))

    code, payload = handle(
        "GET", "/api/roster/characters", {"db": "local", "limit": "50"}, {}
    )
    assert code == 200
    ids = [c["id"] for c in payload["characters"]]
    assert ids.index("abeikelongbi") < ids.index("gezi")
    assert ids.index("bailong") < ids.index("gezi")
    assert ids.index("z23") < ids.index("kubo")
    # stubs cluster at end
    assert ids[-2:] == ["gezi", "kubo"] or set(ids[-2:]) == {"gezi", "kubo"}


def test_list_filter_faction_and_skin_count(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
):
    db = tmp_path / "local.sqlite"
    conn = connect(db)
    apply_schema(conn)
    for cid, name, faction in (
        ("a", "甲", "白鹰"),
        ("b", "乙", "重樱"),
        ("c", "丙", "白鹰"),
    ):
        conn.execute(
            "INSERT INTO characters(id,name_zh,name_en,faction) VALUES (?,?,?,?)",
            (cid, name, "", faction),
        )
    # a: 0 skins, c: 3 skins
    for i in range(3):
        conn.execute(
            "INSERT INTO skins(id,character_id,name_zh,name_en,sort_order,is_default) "
            "VALUES (?,?,?,?,?,0)",
            (f"c-s{i}", "c", f"皮{i}", "", i),
        )
    conn.commit()
    conn.close()
    monkeypatch.setenv("HANDAILY_ROSTER_DB", str(db))

    code, payload = handle(
        "GET",
        "/api/roster/characters",
        {"db": "local", "faction": "白鹰", "limit": "50"},
        {},
    )
    assert code == 200
    ids = {c["id"] for c in payload["characters"]}
    assert ids == {"a", "c"}

    code, payload = handle(
        "GET",
        "/api/roster/characters",
        {"db": "local", "skin_count": "none", "limit": "50"},
        {},
    )
    assert {c["id"] for c in payload["characters"]} == {"a", "b"}

    code, payload = handle(
        "GET",
        "/api/roster/characters",
        {"db": "local", "skin_count": "many", "limit": "50"},
        {},
    )
    assert [c["id"] for c in payload["characters"]] == ["c"]

    code, payload = handle("GET", "/api/roster/factions", {"db": "local"}, {})
    assert code == 200
    assert payload["factions"] == ["白鹰", "重樱"] or set(payload["factions"]) == {
        "白鹰",
        "重樱",
    }


def test_list_filter_kanmusu_ready(tmp_path: Path, monkeypatch: pytest.MonkeyPatch):
    db = tmp_path / "local.sqlite"
    live = tmp_path / "live2d"
    km = tmp_path / "unpacked"
    km.mkdir()
    (km / "ready_ship").mkdir()
    (km / "ready_ship" / "x.moc3").write_bytes(b"m")
    (km / "ready_ship" / "x.model3.json").write_text("{}", encoding="utf-8")

    conn = connect(db)
    apply_schema(conn)
    conn.execute(
        "INSERT INTO characters(id,name_zh,name_en) VALUES (?,?,?)",
        ("ready", "有皮", ""),
    )
    conn.execute(
        "INSERT INTO characters(id,name_zh,name_en) VALUES (?,?,?)",
        ("empty", "无皮", ""),
    )
    conn.execute(
        "INSERT INTO skins(id,character_id,name_zh,name_en,kanmusu_dir,sort_order,is_default) "
        "VALUES (?,?,?,?,?,?,0)",
        ("ready-s", "ready", "默认", "", "ready_ship", 0),
    )
    conn.commit()
    conn.close()
    monkeypatch.setenv("HANDAILY_ROSTER_DB", str(db))
    monkeypatch.setenv("HANDAILY_MODEL_UNPACKED", str(km))
    monkeypatch.setenv("HANDAILY_LIVE2D_PATH", str(live))

    code, payload = handle(
        "GET",
        "/api/roster/characters",
        {"db": "local", "kanmusu": "ready", "limit": "50"},
        {},
    )
    assert code == 200
    assert [c["id"] for c in payload["characters"]] == ["ready"]
    assert payload["characters"][0]["kanmusu_status"] == "ready"


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

    live = tmp_path / "live2d"
    live.mkdir()
    pet = live / "edu_pet"
    pet.mkdir()
    (pet / "edu.skel").write_bytes(b"1")
    (pet / "edu.atlas").write_text("a", encoding="utf-8")
    monkeypatch.setenv("HANDAILY_LIVE2D_PATH", str(live))
    monkeypatch.setenv("HANDAILY_MODEL_UNPACKED", str(tmp_path / "unpacked"))
    (tmp_path / "unpacked").mkdir()

    code, payload = handle("GET", "/api/roster/skins", {"db": "local", "limit": "10"}, {})
    assert code == 200
    assert payload["total"] == 1
    sk = payload["skins"][0]
    assert sk["pet_status"] == "ready"
    assert sk["kanmusu_status"] == "missing"
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
        "POST", "/api/roster/ops/publish-bundled", {"db": "bundled"}, {"confirm_bundled": True}
    )
    assert code == 400


def test_resolve_path_local_env(tmp_path: Path, monkeypatch: pytest.MonkeyPatch):
    db = tmp_path / "x.sqlite"
    monkeypatch.setenv("HANDAILY_ROSTER_DB", str(db))
    assert resolve_path("local") == db
