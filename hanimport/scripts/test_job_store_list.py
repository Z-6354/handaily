from job_store import create_job, update_job, list_jobs, get_job
import time


def test_list_jobs_order_and_limit():
    # clear by creating fresh module state — tests run in process; use unique kinds
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
