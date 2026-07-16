from pathlib import Path

import pytest

from skin_probe import enrich_skin, probe_kanmusu, probe_pet


def test_probe_pet_unbound():
    assert probe_pet("")["status"] == "unbound"
    assert probe_pet(None)["status"] == "unbound"


def test_probe_pet_missing_and_ready(tmp_path: Path):
    root = tmp_path / "live2d"
    root.mkdir()
    assert probe_pet("gone", roots=[root])["status"] == "missing"

    empty = root / "empty"
    empty.mkdir()
    assert probe_pet("empty", roots=[root])["status"] == "missing"

    ready = root / "edu"
    ready.mkdir()
    (ready / "edu.skel").write_bytes(b"x")
    (ready / "edu.atlas").write_text("a", encoding="utf-8")
    r = probe_pet("edu", roots=[root])
    assert r["status"] == "ready"
    assert r["path"] and r["path"].endswith("edu")


def test_probe_pet_rejects_traversal(tmp_path: Path):
    root = tmp_path / "live2d"
    root.mkdir()
    assert probe_pet("../secret", roots=[root])["status"] == "missing"


def test_probe_kanmusu_states(tmp_path: Path):
    root = tmp_path / "unpacked"
    root.mkdir()
    assert probe_kanmusu("", root=root)["status"] == "unbound"
    assert probe_kanmusu("missing_dir", root=root)["status"] == "missing"

    d = root / "ship_a"
    d.mkdir()
    (d / "ship.model3.json").write_text("{}", encoding="utf-8")
    r = probe_kanmusu("ship_a", root=root)
    assert r["status"] == "ready"


def test_enrich_skin(tmp_path: Path):
    live = tmp_path / "live2d"
    km = tmp_path / "unpacked"
    live.mkdir()
    km.mkdir()
    skin = {
        "id": "s1",
        "pet_model_id": "",
        "kanmusu_dir": "x",
    }
    out = enrich_skin(skin, live2d_roots=[live], kanmusu_root=km)
    assert out["pet_status"] == "unbound"
    assert out["kanmusu_status"] == "missing"
