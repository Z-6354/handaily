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
        for path in ("/", "/unpack", "/roster", "/design-system/tokens.css", "/api/jobs"):
            c.request("GET", path)
            r = c.getresponse()
            body = r.read()
            assert r.status == 200, path
            assert body, path
    finally:
        httpd.shutdown()
