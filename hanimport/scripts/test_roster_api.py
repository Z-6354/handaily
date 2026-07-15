from pathlib import Path

from roster_db import normalize_name_en, connect, apply_schema, fill_english_names


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
