"""Tests for model preview asset path resolution + HTTP routes."""
from __future__ import annotations

import threading
from http.client import HTTPConnection
from http.server import ThreadingHTTPServer
from pathlib import Path

import web.asset_routes as asset_routes
import web.serve_web as serve_web


def test_resolve_live2d_rejects_traversal(tmp_path: Path):
    root = tmp_path / "live2d"
    (root / "ship").mkdir(parents=True)
    (root / "ship" / "a.skel").write_bytes(b"skel")
    (tmp_path / "secret.txt").write_text("nope", encoding="utf-8")
    assert asset_routes.resolve_live2d_asset("ship/../secret.txt", roots=[root]) is None
    assert asset_routes.resolve_live2d_asset("../secret.txt", roots=[root]) is None
    assert asset_routes.resolve_live2d_asset("ship/a.skel", roots=[root]) == (
        root / "ship" / "a.skel"
    ).resolve()


def test_resolve_unpacked_nested(tmp_path: Path):
    root = tmp_path / "unpacked"
    nested = root / "foo" / "bar"
    nested.mkdir(parents=True)
    f = nested / "x.moc3"
    f.write_bytes(b"moc")
    got = asset_routes.resolve_unpacked_asset("foo/bar/x.moc3", root=root)
    assert got == f.resolve()
    assert asset_routes.resolve_unpacked_asset("foo/../foo/bar/x.moc3", root=root) is None


def _server() -> tuple[ThreadingHTTPServer, int]:
    httpd = ThreadingHTTPServer(("127.0.0.1", 0), serve_web.Handler)
    t = threading.Thread(target=httpd.serve_forever, daemon=True)
    t.start()
    return httpd, httpd.server_address[1]


def test_http_assets_live2d_and_traversal(tmp_path: Path, monkeypatch):
    root = tmp_path / "live2d"
    (root / "demo").mkdir(parents=True)
    skel = root / "demo" / "demo.skel"
    skel.write_bytes(b"SKELDATA")
    monkeypatch.setattr(asset_routes, "default_live2d_roots", lambda: [root])
    monkeypatch.setattr(
        "roster.skin_probe.default_live2d_roots", lambda: [root]
    )

    httpd, port = _server()
    try:
        c = HTTPConnection("127.0.0.1", port, timeout=3)
        c.request("GET", "/assets/pet/demo/demo.skel")
        r = c.getresponse()
        body = r.read()
        assert r.status == 200, body
        assert body == b"SKELDATA"

        c.request("GET", "/assets/pet/demo/../demo/demo.skel")
        r = c.getresponse()
        r.read()
        assert r.status in (403, 404)

        c.request("GET", "/assets/pet/../../etc/passwd")
        r = c.getresponse()
        r.read()
        assert r.status in (403, 404)
    finally:
        httpd.shutdown()
