from job_store import create_job, update_job, list_jobs, get_job, append_log
import time


def test_list_jobs_order_and_limit():
    ids = []
    for i in range(3):
        jid = create_job("unpack")
        ids.append(jid)
        update_job(jid, status="done" if i else "running", current=i, total=3)
        time.sleep(0.01)
    listed = list_jobs(2)
    assert len(listed) == 2
    assert listed[0]["updated_at"] >= listed[1]["updated_at"]
    assert get_job(ids[-1])["id"] == listed[0]["id"]
    # Hub list must not leak live ring buffers
    assert "log_tail" not in listed[0]
    assert "results" not in listed[0]


def test_get_job_snapshot_isolates_log_tail():
    jid = create_job("unpack")
    append_log(jid, "line-a")
    snap = get_job(jid)
    assert snap is not None
    snap["log_tail"].append("mutated-by-caller")
    again = get_job(jid)
    assert again is not None
    assert again["log_tail"] == ["line-a"]
