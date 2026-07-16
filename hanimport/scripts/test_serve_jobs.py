"""TDD: async unpack/config job workers in serve_web."""
from __future__ import annotations

import json
import threading
import time
from http.client import HTTPConnection
from http.server import ThreadingHTTPServer
from pathlib import Path

import serve_web
from job_store import get_job


def _wait_done(jid: str, timeout: float = 5.0) -> dict:
    deadline = time.time() + timeout
    while time.time() < deadline:
        snap = get_job(jid)
        assert snap is not None
        if snap["status"] in ("done", "error"):
            return snap
        time.sleep(0.05)
    raise AssertionError(f"job {jid} did not finish: {get_job(jid)}")


def test_start_unpack_job_dry_run(tmp_path: Path):
    bundle = tmp_path / "aidang.ab"
    bundle.write_bytes(b"UnityFS" + b"\x00" * 8)
    body = {
        "input": str(tmp_path),
        "dry_run": True,
        "generate_config": False,
        "continue_on_error": False,
    }
    jid = serve_web.start_unpack_job(body)
    snap = _wait_done(jid)
    assert snap["status"] == "done"
    assert snap["kind"] == "unpack"
    assert snap["phase"] == ""
    assert snap["total"] >= 1
    assert snap["ok_count"] + snap.get("skip_count", 0) >= 1
    assert snap["fail_count"] == 0


def test_unpack_then_config_resets_phase_counters(tmp_path: Path, monkeypatch):
    """Config phase resets ok_count so unpack successes are not double-counted."""
    bundle = tmp_path / "aidang.ab"
    bundle.write_bytes(b"UnityFS" + b"\x00" * 8)
    out_dir = tmp_path / "unpacked" / "aidang"
    out_dir.mkdir(parents=True)
    (out_dir / "aidang.skel").write_bytes(b"skel")

    def fake_unpack(input_file, output_root, slug):
        return {"ok": True, "kind": "spine", "output_dir": str(out_dir), "slug": slug}

    def fake_config(folder, *, src_dir, force, dry_run):
        return {"ok": True, "slug": folder.name, "idle": "idle", "click": "touch"}

    monkeypatch.setattr(serve_web, "unitypy_installed", lambda: True)
    monkeypatch.setattr(serve_web, "run_unpack_one", fake_unpack)
    monkeypatch.setattr(serve_web, "_generate_config_for_dir", fake_config)

    jid = serve_web.start_unpack_job(
        {
            "input": str(tmp_path),
            "output": str(tmp_path / "unpacked"),
            "dry_run": False,
            "generate_config": True,
            "continue_on_error": False,
        }
    )
    snap = _wait_done(jid)
    assert snap["status"] == "done"
    assert snap["phase"] == ""
    assert snap["ok_count"] == 1  # config phase only, not unpack+config
    assert snap["fail_count"] == 0
    log = "\n".join(snap.get("log_tail") or [])
    assert "解包阶段完成 ok=1" in log


def test_jobs_http_dry_run(tmp_path: Path):
    bundle = tmp_path / "qiye.ab"
    bundle.write_bytes(b"UnityFS" + b"\x00" * 8)

    server = ThreadingHTTPServer(("127.0.0.1", 0), serve_web.Handler)
    port = server.server_address[1]
    t = threading.Thread(target=server.serve_forever, daemon=True)
    t.start()
    try:
        conn = HTTPConnection("127.0.0.1", port, timeout=5)
        payload = json.dumps(
            {
                "input": str(tmp_path),
                "dry_run": True,
                "generate_config": False,
            }
        ).encode("utf-8")
        conn.request(
            "POST",
            "/api/jobs/unpack",
            body=payload,
            headers={"Content-Type": "application/json", "Content-Length": str(len(payload))},
        )
        resp = conn.getresponse()
        data = json.loads(resp.read().decode("utf-8"))
        assert resp.status == 200
        assert data.get("ok") is True
        jid = data["job_id"]

        deadline = time.time() + 5.0
        job = None
        while time.time() < deadline:
            conn.request("GET", f"/api/jobs/{jid}")
            r2 = conn.getresponse()
            body2 = json.loads(r2.read().decode("utf-8"))
            assert body2.get("ok") is True
            job = body2["job"]
            if job["status"] in ("done", "error"):
                break
            time.sleep(0.05)
        assert job is not None
        assert job["status"] == "done"
        assert job["ok_count"] + job.get("skip_count", 0) >= 1
    finally:
        server.shutdown()
