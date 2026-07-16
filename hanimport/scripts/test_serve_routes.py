"""Smoke tests: hub redesign static routes and jobs list."""
from __future__ import annotations

import threading
from http.client import HTTPConnection
from http.server import ThreadingHTTPServer

import serve_web
from job_store import create_job


def _server() -> tuple[ThreadingHTTPServer, int]:
    httpd = ThreadingHTTPServer(("127.0.0.1", 0), serve_web.Handler)
    t = threading.Thread(target=httpd.serve_forever, daemon=True)
    t.start()
    port = httpd.server_address[1]
    return httpd, port


def test_pages_and_jobs_list():
    create_job("unpack")
    httpd, port = _server()
    try:
        c = HTTPConnection("127.0.0.1", port, timeout=3)
        for path in (
            "/",
            "/unpack",
            "/roster",
            "/skins",
            "/design-system/tokens.css",
            "/shell.css",
            "/components.css",
            "/pages/hub.css",
            "/pages/unpack.css",
            "/pages/roster.css",
            "/pages/skins.css",
            "/api/jobs",
        ):
            c.request("GET", path)
            r = c.getresponse()
            body = r.read()
            assert r.status == 200, path
            assert body, path
            if path.endswith(".css") or path.endswith(".html") or path in ("/", "/unpack", "/roster", "/skins"):
                # html pages + css: Cache-Control should discourage stale layered CSS
                if path.endswith(".css"):
                    assert "no-cache" in (r.getheader("Cache-Control") or "")

        # HTML must link layered page CSS (not deleted root CSS)
        c.request("GET", "/skins")
        r = c.getresponse()
        html = r.read().decode("utf-8", errors="replace")
        assert r.status == 200
        assert "/pages/skins.css" in html
        assert 'href="/roster.css"' not in html
        assert 'href="/style.css"' not in html

        for old in ("/style.css", "/hub.css", "/roster.css", "/skins.css"):
            c.request("GET", old)
            r = c.getresponse()
            r.read()
            assert r.status == 404, old
    finally:
        httpd.shutdown()


def test_required_web_assets_present():
    missing = [p for p in serve_web.REQUIRED_WEB_ASSETS if not p.is_file()]
    assert missing == [], missing
