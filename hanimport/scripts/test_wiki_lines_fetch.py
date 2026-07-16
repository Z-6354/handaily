from pathlib import Path
import json
import sqlite3

from wiki_lines_fetch import (
    list_missing_line_targets,
    ship_has_lines_by_skin,
    wiki_request_headers,
)


def test_wiki_request_headers_include_referer():
    h = wiki_request_headers("柴郡")
    assert "User-Agent" in h
    assert h.get("Accept")
    assert "biligame.com" in h.get("Referer", "")
    assert "%E6%9F%B4%E9%83%A1" in h["Referer"] or "柴郡" in h["Referer"]


def test_ship_has_lines_by_skin(tmp_path: Path):
    db = tmp_path / "w.sqlite"
    conn = sqlite3.connect(db)
    conn.execute(
        """
        CREATE TABLE ships (
          wiki_title TEXT PRIMARY KEY,
          display_name TEXT,
          lines_by_skin_json TEXT DEFAULT '[]'
        )
        """
    )
    conn.execute(
        "INSERT INTO ships(wiki_title, display_name, lines_by_skin_json) VALUES (?,?,?)",
        ("绫波", "绫波", json.dumps([{"skin": "default", "lines": []}])),
    )
    conn.execute(
        "INSERT INTO ships(wiki_title, display_name, lines_by_skin_json) VALUES (?,?,?)",
        ("空舰", "空舰", "[]"),
    )
    conn.commit()
    conn.close()
    assert ship_has_lines_by_skin(db, wiki_title="绫波") is True
    assert ship_has_lines_by_skin(db, name_zh="空舰") is False


def test_ship_has_requires_skins_json_when_column_exists(tmp_path: Path):
    db = tmp_path / "w.sqlite"
    conn = sqlite3.connect(db)
    conn.execute(
        """
        CREATE TABLE ships (
          wiki_title TEXT PRIMARY KEY,
          display_name TEXT,
          lines_by_skin_json TEXT DEFAULT '[]',
          skins_json TEXT DEFAULT '[]'
        )
        """
    )
    conn.execute(
        "INSERT INTO ships(wiki_title, display_name, lines_by_skin_json, skins_json) VALUES (?,?,?,?)",
        (
            "有线无皮",
            "有线无皮",
            json.dumps([{"skin": "default", "lines": [{"category": "x", "text": "y"}]}]),
            "[]",
        ),
    )
    conn.commit()
    conn.close()
    assert ship_has_lines_by_skin(db, wiki_title="有线无皮") is False


def test_list_missing(tmp_path: Path):
    wiki = tmp_path / "w.sqlite"
    w = sqlite3.connect(wiki)
    w.execute(
        """
        CREATE TABLE ships (
          wiki_title TEXT PRIMARY KEY,
          display_name TEXT,
          lines_by_skin_json TEXT DEFAULT '[]',
          skins_json TEXT DEFAULT '[]'
        )
        """
    )
    w.execute(
        "INSERT INTO ships(wiki_title, display_name, lines_by_skin_json, skins_json) VALUES (?,?,?,?)",
        (
            "有了",
            "有了",
            json.dumps([{"skin": "default", "lines": [{"k": 1}]}]),
            json.dumps([{"key": "default", "label": "通常"}]),
        ),
    )
    w.commit()
    w.close()

    roster = tmp_path / "r.sqlite"
    r = sqlite3.connect(roster)
    r.row_factory = sqlite3.Row
    r.execute(
        "CREATE TABLE characters (id TEXT PRIMARY KEY, name_zh TEXT, wiki_title TEXT)"
    )
    r.execute(
        "INSERT INTO characters(id, name_zh, wiki_title) VALUES (?,?,?)",
        ("a", "有了", "有了"),
    )
    r.execute(
        "INSERT INTO characters(id, name_zh, wiki_title) VALUES (?,?,?)",
        ("b", "缺了", "缺了"),
    )
    r.commit()
    missing = list_missing_line_targets(r, wiki)
    r.close()
    assert [m["id"] for m in missing] == ["b"]
