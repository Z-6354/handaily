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
