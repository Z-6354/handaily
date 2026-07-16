from job_store import create_job, get_job, update_job, append_log


def test_create_and_progress():
    jid = create_job("unpack")
    snap = get_job(jid)
    assert snap["status"] == "queued"
    assert snap["kind"] == "unpack"
    update_job(jid, status="running", current=1, total=3, current_item="a")
    append_log(jid, "unpack a")
    snap2 = get_job(jid)
    assert snap2["current"] == 1
    assert snap2["total"] == 3
    assert "unpack a" in snap2["log_tail"]
