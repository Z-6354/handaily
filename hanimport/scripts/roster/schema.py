"""Roster DB paths, connect, schema apply (sheared from db.py C1)."""

from __future__ import annotations

import argparse
import hashlib
import json
import logging
import os
import re
import shutil
import sqlite3
import sys
import zipfile
from pathlib import Path

# --- schema ---

def repo_root() -> Path:
    return Path(__file__).resolve().parents[3]

def roster_dir() -> Path:
    return repo_root() / "data" / "roster"

def schema_path() -> Path:
    return roster_dir() / "schema.sql"

def default_local_db() -> Path:
    override = os.environ.get("HANDAILY_ROSTER_DB", "").strip()
    if override:
        return Path(override)
    return roster_dir() / "handaily-roster.sqlite"

def appdata_data_dir() -> Path:
    override = os.environ.get("HANDAILY_DATA_DIR", "").strip()
    if override:
        return Path(override)
    appdata = os.environ.get("APPDATA") or ""
    return Path(appdata) / "xiaohan-daily" / "data"

def bundled_roster_dir() -> Path:
    return repo_root() / "hanpet" / "bundled" / "roster"

def allowlist_path() -> Path:
    return roster_dir() / "bundled-allowlist.json"

def emit(obj) -> None:
    sys.stdout.buffer.write((json.dumps(obj, ensure_ascii=False, indent=2) + "\n").encode("utf-8"))

def load_json(path: Path, default):
    if not path.is_file():
        return default
    return json.loads(path.read_text(encoding="utf-8-sig"))

def connect(db_path: Path) -> sqlite3.Connection:
    db_path.parent.mkdir(parents=True, exist_ok=True)
    conn = sqlite3.connect(str(db_path), timeout=60.0)
    conn.row_factory = sqlite3.Row
    conn.execute("PRAGMA foreign_keys = ON")
    # Concurrent web reads + wiki pipeline writes (default DELETE journal locks hard)
    conn.execute("PRAGMA journal_mode=WAL")
    conn.execute("PRAGMA busy_timeout=60000")
    return conn

def apply_schema(conn: sqlite3.Connection) -> None:
    sql = schema_path().read_text(encoding="utf-8")
    conn.executescript(sql)
    conn.execute(
        "INSERT INTO meta(key, value) VALUES(?, ?) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        ("schema_version", "1"),
    )
    conn.commit()

def cmd_init(args: argparse.Namespace) -> int:
    db = Path(args.db) if args.db else default_local_db()
    if db.is_file() and not args.force:
        emit({"ok": True, "db": str(db), "note": "already exists (use --force to recreate)"})
        return 0
    if db.is_file() and args.force:
        db.unlink()
    conn = connect(db)
    apply_schema(conn)
    conn.close()
    emit({"ok": True, "db": str(db), "action": "init"})
    return 0

