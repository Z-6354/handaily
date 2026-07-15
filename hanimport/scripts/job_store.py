from __future__ import annotations

import threading
import time
import uuid
from typing import Any

_lock = threading.Lock()
_JOBS: dict[str, dict[str, Any]] = {}
_LOG_MAX = 200


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
        return dict(j) if j else None


def list_jobs(limit: int = 20) -> list[dict[str, Any]]:
    n = max(1, min(int(limit), 50))
    with _lock:
        items = sorted(_JOBS.values(), key=lambda j: j["updated_at"], reverse=True)
        return [dict(j) for j in items[:n]]


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
