"""Tests for avatar path resolve and catalog lookup."""
from __future__ import annotations

import sqlite3
from pathlib import Path

import avatar_fetch


def test_resolve_avatar_file(tmp_path: Path, monkeypatch):
    monkeypatch.setattr(avatar_fetch, "avatars_dir", lambda: tmp_path)
    assert avatar_fetch.resolve_avatar_file("edu") is None
    p = tmp_path / "edu.jpg"
    p.write_bytes(b"abc")
    found = avatar_fetch.resolve_avatar_file("edu")
    assert found == p
    assert avatar_fetch.avatar_public_url("edu").startswith("/avatars/edu?t=")
    assert avatar_fetch.resolve_avatar_file("../etc") is None
    assert avatar_fetch.resolve_avatar_file("a/b") is None


def test_lookup_avatar_url(tmp_path: Path):
    db = tmp_path / "w.sqlite"
    conn = sqlite3.connect(db)
    conn.execute(
        """
        CREATE TABLE catalog (
          wiki_title TEXT, wiki_path TEXT, display_name TEXT,
          aliases_json TEXT, avatar_url TEXT, rarity TEXT, faction TEXT,
          ship_type TEXT, created_at TEXT, updated_at TEXT
        )
        """
    )
    conn.execute(
        "INSERT INTO catalog(wiki_title, display_name, avatar_url) VALUES (?,?,?)",
        ("恶毒", "恶毒", "https://patchwiki.biligame.com/images/blhx/x/y.jpg"),
    )
    conn.commit()
    conn.close()
    url = avatar_fetch.lookup_avatar_url(db, wiki_title="恶毒", name_zh="")
    assert url and "biligame.com" in url
    assert avatar_fetch.lookup_avatar_url(db, wiki_title="", name_zh="不存在") is None


def test_lookup_avatar_url_by_character_id_pinyin(tmp_path: Path):
    """Unpacked stubs use pinyin id; catalog only has Chinese titles."""
    db = tmp_path / "w.sqlite"
    conn = sqlite3.connect(db)
    conn.execute(
        """
        CREATE TABLE catalog (
          wiki_title TEXT, wiki_path TEXT, display_name TEXT,
          aliases_json TEXT, avatar_url TEXT, rarity TEXT, faction TEXT,
          ship_type TEXT, created_at TEXT, updated_at TEXT
        )
        """
    )
    conn.execute(
        "INSERT INTO catalog(wiki_title, display_name, avatar_url) VALUES (?,?,?)",
        (
            "艾伦·萨姆纳",
            "艾伦·萨姆纳",
            "https://patchwiki.biligame.com/images/blhx/ailun.jpg",
        ),
    )
    conn.execute(
        "INSERT INTO catalog(wiki_title, display_name, avatar_url) VALUES (?,?,?)",
        ("白龙", "白龙", "https://patchwiki.biligame.com/images/blhx/bailong.jpg"),
    )
    conn.execute(
        "INSERT INTO catalog(wiki_title, display_name, avatar_url) VALUES (?,?,?)",
        (
            "埃米尔·贝尔汀",
            "埃米尔·贝尔汀",
            "https://patchwiki.biligame.com/images/blhx/bertin.jpg",
        ),
    )
    conn.commit()
    conn.close()

    assert avatar_fetch.to_pinyin_slug("艾伦·萨姆纳") == "ailunsamuna"
    assert avatar_fetch.to_pinyin_slug("白龙") == "bailong"
    # Wiki 汀→ting；游戏文件夹常用 ding
    assert avatar_fetch.to_pinyin_slug("埃米尔·贝尔汀") == "aimierbeierting"

    url = avatar_fetch.lookup_avatar_url(
        db, wiki_title="ailunsamuna", name_zh="ailunsamuna", character_id="ailunsamuna"
    )
    assert url and "ailun.jpg" in url
    url2 = avatar_fetch.lookup_avatar_url(db, character_id="bailong")
    assert url2 and "bailong.jpg" in url2
    # ding/ting soft match — not a wrong character, just nonstandard game pinyin
    url3 = avatar_fetch.lookup_avatar_url(db, character_id="aimierbeierding")
    assert url3 and "bertin.jpg" in url3
    hit = avatar_fetch.resolve_catalog_by_slug(db, "aimierbeierding")
    assert hit and hit["display_name"] == "埃米尔·贝尔汀"


def test_resolve_catalog_uses_aliases_and_ship_codes(tmp_path: Path, monkeypatch):
    db = tmp_path / "w.sqlite"
    conn = sqlite3.connect(db)
    conn.execute(
        """
        CREATE TABLE catalog (
          wiki_title TEXT, wiki_path TEXT, display_name TEXT,
          aliases_json TEXT, avatar_url TEXT, rarity TEXT, faction TEXT,
          ship_type TEXT, created_at TEXT, updated_at TEXT
        )
        """
    )
    for name, url in (
        ("武藏", "https://patchwiki.biligame.com/images/blhx/wuzang.jpg"),
        ("Z23", "https://patchwiki.biligame.com/images/blhx/z23.jpg"),
        ("玛莉萝丝", "https://patchwiki.biligame.com/images/blhx/mary.jpg"),
        ("拉·加利索尼埃", "https://patchwiki.biligame.com/images/blhx/galli.jpg"),
    ):
        conn.execute(
            "INSERT INTO catalog(wiki_title, display_name, avatar_url) VALUES (?,?,?)",
            (name, name, url),
        )
    conn.commit()
    conn.close()

    monkeypatch.setattr(
        avatar_fetch,
        "_load_slug_aliases",
        lambda: {
            "wuzang": "武藏",
            "z23": "Z23",
            "maliluosi": "玛莉萝丝",
            "jialisuoniye": "拉·加利索尼埃",
        },
    )
    avatar_fetch._catalog_slug_index.cache_clear()

    assert avatar_fetch.resolve_catalog_by_slug(db, "wuzang")["display_name"] == "武藏"
    assert avatar_fetch.resolve_catalog_by_slug(db, "z23")["display_name"] == "Z23"
    assert (
        avatar_fetch.resolve_catalog_by_slug(db, "maliluosi_3_doa")["display_name"]
        == "玛莉萝丝"
    )
    assert (
        avatar_fetch.resolve_catalog_by_slug(db, "jialisuoniye")["display_name"]
        == "拉·加利索尼埃"
    )


def test_fetch_one_uses_character_id_when_name_is_slug(tmp_path: Path, monkeypatch):
    wiki = tmp_path / "w.sqlite"
    conn = sqlite3.connect(wiki)
    conn.execute(
        """
        CREATE TABLE catalog (
          wiki_title TEXT, wiki_path TEXT, display_name TEXT,
          aliases_json TEXT, avatar_url TEXT, rarity TEXT, faction TEXT,
          ship_type TEXT, created_at TEXT, updated_at TEXT
        )
        """
    )
    conn.execute(
        "INSERT INTO catalog(wiki_title, display_name, avatar_url) VALUES (?,?,?)",
        ("白龙", "白龙", "https://patchwiki.biligame.com/images/blhx/bailong.jpg"),
    )
    conn.commit()
    conn.close()

    av = tmp_path / "avatars"
    av.mkdir()
    monkeypatch.setattr(avatar_fetch, "avatars_dir", lambda: av)

    def fake_download(url: str, character_id: str, timeout: float = 20.0):
        dest = av / f"{character_id}.jpg"
        dest.write_bytes(b"img")
        return dest

    monkeypatch.setattr(avatar_fetch, "download_avatar", fake_download)
    result = avatar_fetch.fetch_one(
        {"id": "bailong", "name_zh": "bailong", "wiki_title": "bailong"},
        wiki_db=wiki,
    )
    assert result["status"] == "ok"
    assert (av / "bailong.jpg").is_file()
