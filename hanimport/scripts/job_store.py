from __future__ import annotations

import threading
import time
import uuid
from typing import Any

_lock = threading.Lock()
_JOBS: dict[str, dict[str, Any]] = {}
_LOG_MAX = 200


def _snapshot(job: dict[str, Any]) -> dict[str, Any]:
    """Detached copy so callers cannot mutate live log_tail/results under the lock."""
    snap = dict(job)
    snap["log_tail"] = list(job.get("log_tail") or [])
    snap["results"] = list(job.get("results") or [])
    return snap


def create_job(kind: str) -> str:
    jid = uuid.uuid4().hex[:12]
    now = time.time()
    with _lock:
        _JOBS[jid] = {
            "id": jid,
            "kind": kind,
            "status": "queued",
            "phase": "",
            "current": 0,
            "total": 0,
            "current_item": "",
            "ok_count": 0,
            "fail_count": 0,
            "log_tail": [],
            "error": None,
            "results": [],
            "created_at": now,
            "updated_at": now,
        }
    return jid


def get_job(job_id: str) -> dict[str, Any] | None:
    with _lock:
        j = _JOBS.get(job_id)
        return _snapshot(j) if j else None


def list_jobs(limit: int = 20) -> list[dict[str, Any]]:
    """Newest-first job summaries for the hub (no log_tail/results payloads)."""
    n = max(1, min(int(limit), 50))
    with _lock:
        items = sorted(_JOBS.values(), key=lambda j: j["updated_at"], reverse=True)
        out: list[dict[str, Any]] = []
        for j in items[:n]:
            snap = _snapshot(j)
            snap.pop("log_tail", None)
            snap.pop("results", None)
            out.append(snap)
        return out


def update_job(job_id: str, **fields: Any) -> None:
    with _lock:
        j = _JOBS.get(job_id)
        if not j:
            return
        for k, v in fields.items():
            if k == "log_tail":
                continue
            j[k] = v
        j["updated_at"] = time.time()


def append_log(job_id: str, line: str) -> None:
    with _lock:
        j = _JOBS.get(job_id)
        if not j:
            return
        j["log_tail"].append(line)
        if len(j["log_tail"]) > _LOG_MAX:
            j["log_tail"] = j["log_tail"][-_LOG_MAX:]
        j["updated_at"] = time.time()
