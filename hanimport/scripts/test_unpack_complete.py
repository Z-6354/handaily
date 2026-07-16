"""TDD: complete vs half-finished unpack dirs."""
from __future__ import annotations

from pathlib import Path

from unpack_complete import is_unpack_complete, prepare_unpack_dir


def test_live2d_complete(tmp_path: Path):
    d = tmp_path / "ship_2"
    d.mkdir()
    (d / "ship_2.moc3").write_bytes(b"moc")
    (d / "ship_2.model3.json").write_text("{}", encoding="utf-8")
    (d / "texture_00.png").write_bytes(b"png")
    assert is_unpack_complete(d, "ship_2")
    assert prepare_unpack_dir(d, "ship_2") == "skip"
    assert d.is_dir()


def test_spine_complete(tmp_path: Path):
    d = tmp_path / "qiye"
    d.mkdir()
    (d / "qiye.atlas").write_text("atlas", encoding="utf-8")
    (d / "qiye.skel").write_bytes(b"skel")
    assert is_unpack_complete(d, "qiye")
    assert prepare_unpack_dir(d, "qiye") == "skip"


def test_half_finished_missing_model3_is_deleted(tmp_path: Path):
    d = tmp_path / "half"
    d.mkdir()
    (d / "half.moc3").write_bytes(b"moc")
    # no model3.json → incomplete
    assert not is_unpack_complete(d, "half")
    assert prepare_unpack_dir(d, "half") == "ready"
    assert not d.exists()


def test_empty_dir_is_deleted(tmp_path: Path):
    d = tmp_path / "empty"
    d.mkdir()
    assert prepare_unpack_dir(d, "empty") == "ready"
    assert not d.exists()


def test_missing_dir_is_ready(tmp_path: Path):
    d = tmp_path / "missing"
    assert prepare_unpack_dir(d, "missing") == "ready"
    assert not d.exists()
