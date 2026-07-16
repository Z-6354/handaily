"""TDD: skip and purge unpack slugs ending with _hx."""
from __future__ import annotations

import json
from pathlib import Path

from unpack_complete import is_hx_slug, purge_hx_output_dirs


def test_is_hx_slug_matches_suffix():
    assert is_hx_slug("ankeleiqi_2_hx")
    assert is_hx_slug("z23_hx")
    assert is_hx_slug("EDU_3_HX")
    assert not is_hx_slug("ankeleiqi_2")
    assert not is_hx_slug("qiye")
    assert not is_hx_slug("foo_hx_bar")
    assert not is_hx_slug("hx")


def test_purge_hx_output_dirs_removes_only_hx(tmp_path: Path):
    keep = tmp_path / "ankeleiqi_2"
    keep.mkdir()
    (keep / "marker").write_text("ok", encoding="utf-8")
    hx = tmp_path / "ankeleiqi_2_hx"
    hx.mkdir()
    (hx / "marker").write_text("gone", encoding="utf-8")
    other = tmp_path / "qiye"
    other.mkdir()

    removed = purge_hx_output_dirs(tmp_path)
    assert "ankeleiqi_2_hx" in removed
    assert not hx.exists()
    assert keep.is_dir()
    assert other.is_dir()


def test_unpack_one_skips_hx_and_removes_output(tmp_path: Path):
    from unpack_bundle import unpack_one

    fake_ab = tmp_path / "src" / "ankeleiqi_2_hx.ab"
    fake_ab.parent.mkdir()
    fake_ab.write_bytes(b"not-a-bundle")
    out_root = tmp_path / "out"
    out_dir = out_root / "ankeleiqi_2_hx"
    out_dir.mkdir(parents=True)
    (out_dir / "leftover.txt").write_text("x", encoding="utf-8")

    result = unpack_one(fake_ab, out_root, slug="ankeleiqi_2_hx")
    assert result["skipped"] is True
    assert result.get("skip_reason") == "hx"
    assert not out_dir.exists()


def test_partition_hx_bundles_for_job_log():
    import serve_web

    bundles = [
        {"path": "a.ab", "slug": "qiye"},
        {"path": "b.ab", "slug": "ankeleiqi_2_hx"},
        {"path": "c.ab", "slug": "edu_3"},
    ]
    keep, hx = serve_web.partition_hx_bundles(bundles)
    assert [b["slug"] for b in keep] == ["qiye", "edu_3"]
    assert [b["slug"] for b in hx] == ["ankeleiqi_2_hx"]
