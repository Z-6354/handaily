#!/usr/bin/env python3
"""Local web UI for hanimport unpack (stdlib only)."""
from __future__ import annotations

import json
import os
import sys
import threading
import webbrowser
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from typing import Any
from urllib.parse import parse_qs, unquote, urlparse

ROOT = Path(__file__).resolve().parents[1]
REPO_ROOT = ROOT.parent
SCRIPTS_DIR = ROOT / "scripts"
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))

from job_store import (  # noqa: E402
    append_log,
    create_job,
    get_job,
    list_jobs,
    request_pause,
    request_resume,
    update_job,
)
import roster_api  # noqa: E402
from avatar_fetch import is_safe_character_id, resolve_avatar_file  # noqa: E402

WEB_DIR = ROOT / "web"
UNPACK_SCRIPT = SCRIPTS_DIR / "unpack_bundle.py"

BUNDLE_EXTENSIONS = {".ab", ".unity3d", ".bytes"}
BUNDLE_MAGIC = b"UnityFS"
DEFAULT_PORT = 7821


def repo_root() -> Path:
    env = os.environ.get("HANDAILY_ROOT", "").strip()
    if env:
        return Path(env)
    if (REPO_ROOT / "hanpet").is_dir():
        return REPO_ROOT
    return ROOT


def default_live2d() -> Path:
    env = os.environ.get("HANDAILY_LIVE2D_PATH", "").strip()
    if env:
        return Path(env)
    root = repo_root()
    for rel in ("data/live2d", "live2d"):
        p = root / rel
        if p.is_dir():
            return p
    return root / "data/live2d"


def default_model_unpacked() -> Path:
    return repo_root() / "data/model/unpacked"


def resolve_output(input_path: Path, explicit: str | None) -> Path:
    if explicit:
        return Path(explicit)
    norm = str(input_path).replace("\\", "/").lower()
    if "data/model" in norm or "/model/" in norm:
        return default_model_unpacked()
    return default_live2d()


def is_unity_bundle(path: Path) -> bool:
    if path.suffix.lower() in BUNDLE_EXTENSIONS:
        return True
    try:
        with path.open("rb") as f:
            return f.read(len(BUNDLE_MAGIC)) == BUNDLE_MAGIC
    except OSError:
        return False


def infer_slug(path: Path) -> str:
    name = path.stem if path.suffix else path.name
    return name.lower()


def discover_bundles(input_path: Path) -> list[dict[str, str]]:
    out: list[dict[str, str]] = []
    if input_path.is_file():
        if is_unity_bundle(input_path):
            out.append({"path": str(input_path), "slug": infer_slug(input_path)})
        return out
    if not input_path.is_dir():
        return out
    for dirpath, _, filenames in os.walk(input_path):
        for fn in filenames:
            fp = Path(dirpath) / fn
            if is_unity_bundle(fp):
                out.append({"path": str(fp), "slug": infer_slug(fp)})
    out.sort(key=lambda x: x["path"].lower())
    return out


def unitypy_installed() -> bool:
    try:
        import UnityPy  # noqa: F401

        return True
    except ImportError:
        return False


def run_unpack_one(input_file: Path, output_root: Path, slug: str) -> dict[str, Any]:
    import subprocess

    cmd = [
        sys.executable,
        str(UNPACK_SCRIPT),
        "--input",
        str(input_file),
        "--output",
        str(output_root),
        "--slug",
        slug,
    ]
    proc = subprocess.run(cmd, capture_output=True, text=True, encoding="utf-8", errors="replace")
    stdout = proc.stdout.strip()
    stderr = proc.stderr.strip()
    json_line = next((ln for ln in (stdout + "\n" + stderr).splitlines() if ln.strip().startswith("{")), "")
    if not json_line:
        raise RuntimeError(f"解包无输出: {input_file}\n{stdout}\n{stderr}")
    data = json.loads(json_line)
    if not data.get("ok"):
        raise RuntimeError(data.get("error") or f"解包失败: {input_file}")
    return data


def suggested_input() -> str | None:
    root = repo_root()
    for rel in (
        "data/model/azurlane/custom",
        "data/model/azurlane/spinepainting",
        "data/model",
        "data/transfer/inbox/azurlane/custom",
    ):
        p = root / rel
        if p.is_dir() and discover_bundles(p):
            return str(p)
    return None


def _folder_has_ext(folder: Path, ext: str) -> bool:
    if not folder.is_dir():
        return False
    lower = ext.lower()
    return any(p.suffix.lower() == lower for p in folder.iterdir() if p.is_file())


def _discover_cubism_folders(root: Path) -> list[Path]:
    """Return Cubism model dirs (contain .moc3) under root, or [root] if root itself is one."""
    if _folder_has_ext(root, ".moc3"):
        return [root]
    out: list[Path] = []
    if not root.is_dir():
        return out
    for child in sorted(root.iterdir()):
        if child.is_dir() and _folder_has_ext(child, ".moc3"):
            out.append(child)
    return out


def _import_config_builders() -> tuple[Any, Any, Any]:
    try:
        from build_model_config import build_folder_configs, discover_spine_folders
        from build_cubism_config import process_slug
    except ImportError:
        sys.path.insert(0, str(SCRIPTS_DIR))
        from build_model_config import build_folder_configs, discover_spine_folders
        from build_cubism_config import process_slug
    return build_folder_configs, discover_spine_folders, process_slug


def _generate_config_for_dir(
    folder: Path,
    *,
    src_dir: Path,
    force: bool,
    dry_run: bool,
) -> dict[str, Any]:
    build_folder_configs, _, process_slug = _import_config_builders()
    if _folder_has_ext(folder, ".skel"):
        return build_folder_configs(folder, dry_run=dry_run, force=force)
    if _folder_has_ext(folder, ".moc3"):
        return process_slug(
            folder.name,
            folder.parent,
            src_dir,
            force=force,
            dry_run=dry_run,
        )
    return {"ok": False, "folder": str(folder), "error": "no .skel or .moc3"}


def start_unpack_job(body: dict[str, Any]) -> str:
    kind = "unpack_then_config" if body.get("generate_config") else "unpack"
    jid = create_job(kind)
    threading.Thread(target=run_unpack_job, args=(jid, body), daemon=True).start()
    return jid


def start_config_job(body: dict[str, Any]) -> str:
    jid = create_job("config")
    threading.Thread(target=run_config_job, args=(jid, body), daemon=True).start()
    return jid


def run_unpack_job(job_id: str, body: dict[str, Any]) -> None:
    try:
        input_raw = (body.get("input") or "").strip()
        if not input_raw:
            update_job(job_id, status="error", phase="", error="input required")
            append_log(job_id, "error: input required")
            return

        dry_run = bool(body.get("dry_run"))
        if not dry_run and not unitypy_installed():
            update_job(
                job_id,
                status="error",
                phase="",
                error="UnityPy 未安装。请运行 hanimport/scripts/setup-env.bat",
            )
            append_log(job_id, "error: UnityPy not installed")
            return

        input_path = Path(input_raw)
        if not input_path.exists():
            update_job(job_id, status="error", phase="", error=f"路径不存在: {input_path}")
            append_log(job_id, f"error: path missing {input_path}")
            return

        output_root = resolve_output(input_path, (body.get("output") or "").strip() or None)
        continue_on_error = bool(body.get("continue_on_error"))
        generate_config = bool(body.get("generate_config"))
        slug_filter = body.get("slugs")
        allowed: set[str] | None = None
        if isinstance(slug_filter, list) and slug_filter:
            allowed = {str(s).strip().lower() for s in slug_filter if str(s).strip()}

        bundles = discover_bundles(input_path)
        if allowed is not None:
            bundles = [b for b in bundles if b["slug"].lower() in allowed]
        if not bundles:
            update_job(job_id, status="error", phase="", error="未找到 AssetBundle 文件")
            append_log(job_id, "error: no bundles found")
            return

        results: list[dict[str, Any]] = []
        ok_count = 0
        fail_count = 0
        src_dir = input_path if input_path.is_dir() else input_path.parent

        update_job(
            job_id,
            status="running",
            phase="unpack",
            current=0,
            total=len(bundles),
            current_item="",
            ok_count=0,
            fail_count=0,
            results=[],
            error=None,
        )
        append_log(job_id, f"输入: {input_path}")
        append_log(job_id, f"输出: {output_root}{' (dry-run)' if dry_run else ''}")
        append_log(job_id, f"共 {len(bundles)} 个 bundle")

        if not dry_run:
            output_root.mkdir(parents=True, exist_ok=True)

        for i, b in enumerate(bundles, 1):
            slug = b["slug"]
            update_job(job_id, current=i, current_item=slug)
            append_log(job_id, f"解包 {slug} …")
            if dry_run:
                append_log(job_id, f"  dry-run: {b['path']}")
                item = {"slug": slug, "input": b["path"], "ok": True, "dry_run": True}
                results.append(item)
                ok_count += 1
                update_job(job_id, ok_count=ok_count, fail_count=fail_count, results=list(results))
                continue
            try:
                data = run_unpack_one(Path(b["path"]), output_root, slug)
                append_log(job_id, f"  ok ({data.get('kind')}) -> {data.get('output_dir')}")
                item = {"slug": slug, "input": b["path"], **data}
                results.append(item)
                ok_count += 1
                update_job(job_id, ok_count=ok_count, fail_count=fail_count, results=list(results))
            except Exception as exc:  # noqa: BLE001
                fail_count += 1
                append_log(job_id, f"  失败: {exc}")
                results.append({"slug": slug, "input": b["path"], "ok": False, "error": str(exc)})
                update_job(job_id, ok_count=ok_count, fail_count=fail_count, results=list(results))
                if not continue_on_error:
                    update_job(
                        job_id,
                        status="error",
                        phase="",
                        error=str(exc),
                        current_item=slug,
                    )
                    return

        if generate_config and not dry_run:
            config_targets = [
                Path(r["output_dir"])
                for r in results
                if r.get("ok") and r.get("output_dir")
            ]
            # Phase-local counters: snapshot unpack totals, then reset so config
            # ok/fail are independent (avoids ~2× ok_count on unpack_then_config).
            append_log(job_id, f"解包阶段完成 ok={ok_count} fail={fail_count}")
            ok_count = 0
            fail_count = 0
            update_job(
                job_id,
                phase="config",
                current=0,
                total=len(config_targets),
                current_item="",
                ok_count=0,
                fail_count=0,
            )
            append_log(job_id, f"生成配置：{len(config_targets)} 个")
            for i, folder in enumerate(config_targets, 1):
                update_job(job_id, current=i, current_item=folder.name)
                append_log(job_id, f"配置 {folder.name} …")
                try:
                    item = _generate_config_for_dir(
                        folder, src_dir=src_dir, force=False, dry_run=False
                    )
                    if not item.get("ok", True) and item.get("error"):
                        raise RuntimeError(item["error"])
                    append_log(
                        job_id,
                        f"  idle={item.get('idle') or item.get('idle_animation')} "
                        f"click={item.get('click') or item.get('click_animation')}",
                    )
                    results.append({"phase": "config", **item})
                    ok_count += 1
                    update_job(
                        job_id, ok_count=ok_count, fail_count=fail_count, results=list(results)
                    )
                except Exception as exc:  # noqa: BLE001
                    fail_count += 1
                    append_log(job_id, f"  失败: {exc}")
                    if "bundle not found" in str(exc).lower():
                        append_log(
                            job_id,
                            f"  cubism: missing bundle; check src={src_dir}",
                        )
                    results.append(
                        {
                            "phase": "config",
                            "slug": folder.name,
                            "ok": False,
                            "error": str(exc),
                        }
                    )
                    update_job(
                        job_id, ok_count=ok_count, fail_count=fail_count, results=list(results)
                    )
                    if not continue_on_error:
                        update_job(
                            job_id,
                            status="error",
                            phase="",
                            error=str(exc),
                            current_item=folder.name,
                        )
                        return

        update_job(
            job_id,
            status="done",
            phase="",
            current_item="",
            ok_count=ok_count,
            fail_count=fail_count,
            results=list(results),
            error=None,
        )
        append_log(job_id, f"完成 ok={ok_count} fail={fail_count}")
    except Exception as exc:  # noqa: BLE001
        update_job(job_id, status="error", phase="", error=str(exc))
        append_log(job_id, f"error: {exc}")


def run_config_job(job_id: str, body: dict[str, Any]) -> None:
    try:
        input_raw = (body.get("input") or "").strip()
        if not input_raw:
            update_job(job_id, status="error", phase="", error="input required")
            append_log(job_id, "error: input required")
            return

        input_path = Path(input_raw)
        if not input_path.exists():
            update_job(job_id, status="error", phase="", error=f"路径不存在: {input_path}")
            append_log(job_id, f"error: path missing {input_path}")
            return

        dry_run = bool(body.get("dry_run"))
        force = bool(body.get("force"))
        # Optional body.src overrides Cubism AssetBundle lookup root.
        src_raw = (body.get("src") or "").strip()
        src_dir = Path(src_raw) if src_raw else (repo_root() / "data/model/azurlane/custom")

        build_folder_configs, discover_spine_folders, process_slug = _import_config_builders()
        spine_folders = discover_spine_folders(input_path)
        cubism_folders = _discover_cubism_folders(input_path)
        spine_set = {f.resolve() for f in spine_folders}
        cubism_folders = [f for f in cubism_folders if f.resolve() not in spine_set]

        targets: list[tuple[str, Path]] = [("spine", f) for f in spine_folders] + [
            ("cubism", f) for f in cubism_folders
        ]
        if not targets:
            update_job(
                job_id,
                status="error",
                phase="",
                error="未找到模型目录（需含 .skel 或 .moc3）",
            )
            append_log(job_id, "error: no spine/cubism folders")
            return

        results: list[dict[str, Any]] = []
        ok_count = 0
        fail_count = 0
        update_job(
            job_id,
            status="running",
            phase="config",
            current=0,
            total=len(targets),
            current_item="",
            ok_count=0,
            fail_count=0,
            results=[],
            error=None,
        )
        append_log(job_id, f"配置输入: {input_path}{' (dry-run)' if dry_run else ''}")
        append_log(job_id, f"共 {len(targets)} 个目录")

        for i, (kind, folder) in enumerate(targets, 1):
            update_job(job_id, current=i, current_item=folder.name)
            append_log(job_id, f"配置 {folder.name} ({kind}) …")
            try:
                if kind == "spine":
                    item = build_folder_configs(folder, dry_run=dry_run, force=force)
                else:
                    item = process_slug(
                        folder.name,
                        folder.parent,
                        src_dir,
                        force=force,
                        dry_run=dry_run,
                    )
                    if not item.get("ok"):
                        raise RuntimeError(item.get("error") or "cubism config failed")
                append_log(
                    job_id,
                    f"  idle={item.get('idle') or item.get('idle_animation')} "
                    f"click={item.get('click') or item.get('click_animation')}",
                )
                results.append(item)
                ok_count += 1
                update_job(job_id, ok_count=ok_count, fail_count=fail_count, results=list(results))
            except Exception as exc:  # noqa: BLE001
                fail_count += 1
                append_log(job_id, f"  失败: {exc}")
                if kind == "cubism" and "bundle not found" in str(exc).lower():
                    append_log(
                        job_id,
                        f"  cubism: missing bundle; check src={src_dir}",
                    )
                results.append({"slug": folder.name, "ok": False, "error": str(exc)})
                update_job(job_id, ok_count=ok_count, fail_count=fail_count, results=list(results))
                update_job(
                    job_id,
                    status="error",
                    phase="",
                    error=str(exc),
                    current_item=folder.name,
                )
                return

        update_job(
            job_id,
            status="done",
            phase="",
            current_item="",
            ok_count=ok_count,
            fail_count=fail_count,
            results=list(results),
            error=None,
        )
        append_log(job_id, f"配置完成 ok={ok_count} fail={fail_count}")
    except Exception as exc:  # noqa: BLE001
        update_job(job_id, status="error", phase="", error=str(exc))
        append_log(job_id, f"error: {exc}")


class Handler(BaseHTTPRequestHandler):
    server_version = "hanimport-web/0.1"

    def log_message(self, fmt: str, *args) -> None:
        sys.stderr.write("[hanimport-web] " + (fmt % args) + "\n")

    def _send_json(self, code: int, payload: dict[str, Any]) -> None:
        body = json.dumps(payload, ensure_ascii=False).encode("utf-8")
        self.send_response(code)
        self.send_header("Content-Type", "application/json; charset=utf-8")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def _read_json(self) -> dict[str, Any]:
        length = int(self.headers.get("Content-Length", "0"))
        raw = self.rfile.read(length) if length else b"{}"
        if not raw.strip():
            return {}
        return json.loads(raw.decode("utf-8"))

    def _parse_path_query(self) -> tuple[str, dict[str, str]]:
        parsed = urlparse(self.path)
        path = unquote(parsed.path)
        query: dict[str, str] = {}
        for key, values in parse_qs(parsed.query, keep_blank_values=True).items():
            if values:
                query[key] = values[0]
        return path, query

    def _dispatch_roster(self, method: str) -> bool:
        path, query = self._parse_path_query()
        if not path.startswith("/api/roster"):
            return False
        body: dict[str, Any] = {}
        if method in ("POST", "PUT", "DELETE", "PATCH"):
            try:
                body = self._read_json()
            except json.JSONDecodeError:
                self._send_json(400, {"ok": False, "error": "invalid JSON"})
                return True
        try:
            code, payload = roster_api.handle(method, path, query, body)
        except Exception as exc:  # noqa: BLE001
            self._send_json(500, {"ok": False, "error": str(exc)})
            return True
        self._send_json(code, payload)
        return True

    def _serve_static(self, path: str) -> None:
        static_routes: dict[str, Path] = {
            "/": WEB_DIR / "index.html",
            "/index.html": WEB_DIR / "index.html",
            "/roster": WEB_DIR / "roster.html",
            "/roster.html": WEB_DIR / "roster.html",
            "/unpack": WEB_DIR / "unpack.html",
            "/unpack.html": WEB_DIR / "unpack.html",
            "/style.css": WEB_DIR / "style.css",
            "/app.js": WEB_DIR / "app.js",
            "/roster.js": WEB_DIR / "roster.js",
            "/roster.css": WEB_DIR / "roster.css",
            "/shell.css": WEB_DIR / "shell.css",
            "/shell.js": WEB_DIR / "shell.js",
            "/hub.js": WEB_DIR / "hub.js",
            "/hub.css": WEB_DIR / "hub.css",
            "/design-system/tokens.css": WEB_DIR / "design-system" / "tokens.css",
        }
        file_path = static_routes.get(path)
        if file_path is None:
            self.send_error(404)
            return

        if not file_path.is_file():
            self.send_error(404)
            return
        content = file_path.read_bytes()
        ctype = "text/html; charset=utf-8"
        if file_path.suffix == ".css":
            ctype = "text/css; charset=utf-8"
        elif file_path.suffix == ".js":
            ctype = "application/javascript; charset=utf-8"
        self.send_response(200)
        self.send_header("Content-Type", ctype)
        self.send_header("Content-Length", str(len(content)))
        self.end_headers()
        self.wfile.write(content)

    def do_GET(self) -> None:
        if self._dispatch_roster("GET"):
            return

        path, query = self._parse_path_query()
        if path == "/api/jobs":
            raw = query.get("limit", "20")
            try:
                lim = int(raw)
            except (TypeError, ValueError):
                lim = 20
            self._send_json(200, {"ok": True, "jobs": list_jobs(lim)})
            return

        if path == "/api/status":
            root = repo_root()
            self._send_json(
                200,
                {
                    "ok": True,
                    "python": sys.version.split()[0],
                    "unitypy": unitypy_installed(),
                    "repo_root": str(root),
                    "default_live2d": str(default_live2d()),
                    "default_model_unpacked": str(default_model_unpacked()),
                    "suggested_input": suggested_input(),
                },
            )
            return

        if path.startswith("/api/jobs/"):
            rest = path[len("/api/jobs/") :].strip("/")
            if "/" in rest:
                self._send_json(404, {"ok": False, "error": "job not found"})
                return
            jid = rest
            if not jid:
                self._send_json(404, {"ok": False, "error": "job not found"})
                return
            snap = get_job(jid)
            if not snap:
                self._send_json(404, {"ok": False, "error": "job not found"})
                return
            self._send_json(200, {"ok": True, "job": snap})
            return

        if path.startswith("/avatars/"):
            self._serve_avatar(path)
            return

        self._serve_static(path)

    def _serve_avatar(self, path: str) -> None:
        cid = path[len("/avatars/") :].split("?", 1)[0].strip("/")
        if not is_safe_character_id(cid) or "/" in cid or "\\" in cid or ".." in cid:
            self.send_error(404)
            return
        file_path = resolve_avatar_file(cid)
        if not file_path:
            self.send_error(404)
            return
        content = file_path.read_bytes()
        ctype = "application/octet-stream"
        suf = file_path.suffix.lower()
        if suf in (".jpg", ".jpeg"):
            ctype = "image/jpeg"
        elif suf == ".png":
            ctype = "image/png"
        elif suf == ".webp":
            ctype = "image/webp"
        self.send_response(200)
        self.send_header("Content-Type", ctype)
        self.send_header("Content-Length", str(len(content)))
        self.send_header("Cache-Control", "public, max-age=3600")
        self.end_headers()
        self.wfile.write(content)

    def do_POST(self) -> None:
        if self._dispatch_roster("POST"):
            return

        path, _query = self._parse_path_query()
        try:
            body = self._read_json()
        except json.JSONDecodeError:
            self._send_json(400, {"ok": False, "error": "invalid JSON"})
            return

        if path == "/api/jobs/unpack":
            jid = start_unpack_job(body)
            self._send_json(200, {"ok": True, "job_id": jid})
            return

        if path == "/api/jobs/config":
            jid = start_config_job(body)
            self._send_json(200, {"ok": True, "job_id": jid})
            return

        m_pause = path.endswith("/pause") or path.endswith("/resume")
        if path.startswith("/api/jobs/") and m_pause:
            parts = path.strip("/").split("/")
            # api jobs {id} pause|resume
            if len(parts) == 4 and parts[0] == "api" and parts[1] == "jobs":
                jid = parts[2]
                action = parts[3]
                ok = request_pause(jid) if action == "pause" else request_resume(jid)
                if not ok:
                    self._send_json(404, {"ok": False, "error": "job not found or not pausable"})
                    return
                snap = get_job(jid)
                self._send_json(200, {"ok": True, "job": snap})
                return

        if path == "/api/scan":
            input_raw = (body.get("input") or "").strip()
            if not input_raw:
                self._send_json(400, {"ok": False, "error": "input required"})
                return
            input_path = Path(input_raw)
            if not input_path.exists():
                self._send_json(400, {"ok": False, "error": f"路径不存在: {input_path}"})
                return
            bundles = discover_bundles(input_path)
            self._send_json(200, {"ok": True, "bundles": bundles})
            return

        if path == "/api/unpack":
            input_raw = (body.get("input") or "").strip()
            if not input_raw:
                self._send_json(400, {"ok": False, "error": "input required"})
                return
            if not unitypy_installed():
                self._send_json(
                    500,
                    {
                        "ok": False,
                        "error": "UnityPy 未安装。请运行 hanimport/scripts/setup-env.bat",
                    },
                )
                return
            input_path = Path(input_raw)
            if not input_path.exists():
                self._send_json(400, {"ok": False, "error": f"路径不存在: {input_path}"})
                return
            output_root = resolve_output(input_path, (body.get("output") or "").strip() or None)
            dry_run = bool(body.get("dry_run"))
            bundles = discover_bundles(input_path)
            if not bundles:
                self._send_json(400, {"ok": False, "error": "未找到 AssetBundle 文件"})
                return

            log: list[str] = []
            results: list[dict[str, Any]] = []
            log.append(f"输入: {input_path}")
            log.append(f"输出: {output_root}{' (dry-run)' if dry_run else ''}")
            log.append(f"共 {len(bundles)} 个 bundle")

            if dry_run:
                for b in bundles:
                    log.append(f"  - {b['slug']} <= {b['path']}")
                self._send_json(
                    200,
                    {
                        "ok": True,
                        "message": "预览完成（未写入文件）",
                        "log": log,
                        "results": [{"slug": b["slug"], "input": b["path"]} for b in bundles],
                    },
                )
                return

            output_root.mkdir(parents=True, exist_ok=True)
            for b in bundles:
                fp = Path(b["path"])
                slug = b["slug"]
                log.append(f"解包 {slug} …")
                try:
                    data = run_unpack_one(fp, output_root, slug)
                    log.append(f"  ok ({data.get('kind')}) -> {data.get('output_dir')}")
                    results.append({"slug": slug, "input": b["path"], **data})
                except Exception as exc:  # noqa: BLE001
                    log.append(f"  失败: {exc}")
                    self._send_json(500, {"ok": False, "error": str(exc), "log": log})
                    return

            self._send_json(
                200,
                {
                    "ok": True,
                    "message": f"解包完成：{len(results)} 个",
                    "log": log,
                    "results": results,
                },
            )
            return

        if path == "/api/config":
            input_raw = (body.get("input") or "").strip()
            if not input_raw:
                self._send_json(400, {"ok": False, "error": "input required"})
                return
            input_path = Path(input_raw)
            if not input_path.exists():
                self._send_json(400, {"ok": False, "error": f"路径不存在: {input_path}"})
                return
            dry_run = bool(body.get("dry_run"))
            force = bool(body.get("force"))
            try:
                from build_model_config import build_folder_configs, discover_spine_folders
            except ImportError:
                sys.path.insert(0, str(SCRIPTS_DIR))
                from build_model_config import build_folder_configs, discover_spine_folders

            folders = discover_spine_folders(input_path)
            if not folders:
                self._send_json(400, {"ok": False, "error": "未找到 Spine 模型目录（需含 .skel）"})
                return
            log = []
            results = []
            for folder in folders:
                log.append(f"配置 {folder.name} …")
                try:
                    item = build_folder_configs(folder, dry_run=dry_run, force=force)
                    log.append(
                        f"  idle={item.get('idle')} click={item.get('click')} "
                        f"touch_areas={item.get('touch_areas')}"
                    )
                    results.append(item)
                except Exception as exc:  # noqa: BLE001
                    log.append(f"  失败: {exc}")
                    self._send_json(500, {"ok": False, "error": str(exc), "log": log})
                    return
            self._send_json(
                200,
                {
                    "ok": True,
                    "message": f"配置完成：{len(results)} 个",
                    "log": log,
                    "results": results,
                },
            )
            return

        self.send_error(404)

    def do_PUT(self) -> None:
        if self._dispatch_roster("PUT"):
            return
        self.send_error(404)

    def do_DELETE(self) -> None:
        if self._dispatch_roster("DELETE"):
            return
        self.send_error(404)


def main() -> int:
    port = DEFAULT_PORT
    if len(sys.argv) > 1:
        port = int(sys.argv[1])
    host = "127.0.0.1"
    server = ThreadingHTTPServer((host, port), Handler)
    url = f"http://{host}:{port}/"
    print(f"[hanimport-web] serving {url}")
    print(f"[hanimport-web] repo root: {repo_root()}")
    threading.Timer(0.8, lambda: webbrowser.open(url)).start()
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        print("\n[hanimport-web] stopped")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
