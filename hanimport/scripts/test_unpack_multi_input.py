"""TDD: multi-path discover + paths-based unpack job."""
from __future__ import annotations

from pathlib import Path

import serve_web
from job_store import get_job
import time


def _wait_done(jid: str, timeout: float = 5.0) -> dict:
    deadline = time.time() + timeout
    while time.time() < deadline:
        snap = get_job(jid)
        assert snap is not None
        if snap["status"] in ("done", "error"):
            return snap
        time.sleep(0.05)
    raise AssertionError(f"job {jid} did not finish: {get_job(jid)}")


def test_discover_bundles_many_merges_and_dedupes(tmp_path: Path):
    d1 = tmp_path / "dir1"
    d1.mkdir()
    a = d1 / "aidang.ab"
    a.write_bytes(b"UnityFS" + b"\x00" * 8)
    b = tmp_path / "qiye.ab"
    b.write_bytes(b"UnityFS" + b"\x00" * 8)
    # same file listed twice via dir + file path
    bundles, warnings = serve_web.discover_bundles_many([str(d1), str(b), str(a)])
    slugs = {x["slug"] for x in bundles}
    assert slugs == {"aidang", "qiye"}
    assert len(bundles) == 2
    assert warnings == []


def test_discover_bundles_many_slug_conflict_keeps_first(tmp_path: Path):
    f1 = tmp_path / "a" / "same.ab"
    f2 = tmp_path / "b" / "same.ab"
    f1.parent.mkdir()
    f2.parent.mkdir()
    f1.write_bytes(b"UnityFS" + b"\x00" * 8)
    f2.write_bytes(b"UnityFS" + b"\x00" * 8)
    bundles, warnings = serve_web.discover_bundles_many([str(f1), str(f2)])
    assert len(bundles) == 1
    assert Path(bundles[0]["path"]).resolve() == f1.resolve()
    assert any("slug" in w.lower() or "冲突" in w for w in warnings)


def test_unpack_job_paths_only(tmp_path: Path):
    keep = tmp_path / "keep.ab"
    skip = tmp_path / "skip.ab"
    keep.write_bytes(b"UnityFS" + b"\x00" * 8)
    skip.write_bytes(b"UnityFS" + b"\x00" * 8)
    jid = serve_web.start_unpack_job(
        {
            "input": str(tmp_path),
            "paths": [str(keep)],
            "dry_run": True,
            "generate_config": False,
        }
    )
    snap = _wait_done(jid)
    assert snap["status"] == "done"
    assert snap["total"] == 1
    assert snap["ok_count"] == 1
    results = snap.get("results") or []
    assert len(results) == 1
    assert results[0]["slug"] == "keep"


def test_collect_scan_inputs_from_body():
    assert serve_web.collect_scan_inputs({"input": "A"}) == ["A"]
    assert serve_web.collect_scan_inputs({"inputs": ["A", "B", ""]}) == ["A", "B"]
    assert serve_web.collect_scan_inputs({"input": "A", "inputs": ["B", "A"]}) == ["A", "B"]
