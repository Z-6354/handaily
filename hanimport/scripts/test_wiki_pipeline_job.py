"""wiki-pipeline job start / attach."""
from __future__ import annotations

from wiki_pipeline_jobs import start_wiki_pipeline_job
from job_store import get_job, update_job


def test_start_attaches_active_job():
    jid1 = start_wiki_pipeline_job({"force": True})
    # mark as running so find_active picks it up; thread may already run
    update_job(jid1, status="running", phase="characters")
    jid2 = start_wiki_pipeline_job({})
    assert jid2 == jid1
    job = get_job(jid1)
    assert job is not None
    assert job["kind"] == "roster-wiki-pipeline"
