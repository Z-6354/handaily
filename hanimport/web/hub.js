(function () {
  "use strict";

  function escapeHtml(s) {
    return String(s)
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;")
      .replace(/"/g, "&quot;");
  }

  function statusLabel(status) {
    if (status === "running" || status === "queued") return "进行中";
    if (status === "done") return "完成";
    if (status === "error") return "失败";
    return status || "未知";
  }

  function statusClass(status) {
    if (status === "done") return "ok";
    if (status === "error") return "err";
    return "running";
  }

  function formatTime(ts) {
    if (!ts) return "";
    return new Date(ts * 1000).toLocaleString("zh-CN", {
      month: "numeric",
      day: "numeric",
      hour: "2-digit",
      minute: "2-digit",
    });
  }

  function renderEnv(st) {
    var el = document.getElementById("env-summary");
    if (!el) return;
    var lines = [
      "Python " + st.python,
      st.unitypy ? "UnityPy 已安装" : "UnityPy 未安装 — 请运行「安装依赖.bat」",
      "仓库 " + st.repo_root,
    ];
    el.innerHTML = lines
      .map(function (line) {
        return "<div>" + escapeHtml(line) + "</div>";
      })
      .join("");
    if (!st.unitypy) el.classList.add("warn-text");
  }

  function renderJobs(data) {
    var el = document.getElementById("recent-jobs");
    if (!el) return;
    var jobs = (data && data.jobs) || [];
    if (!jobs.length) {
      el.innerHTML =
        '<p class="empty-state">尚无解包任务。<a href="/unpack">前往解包</a></p>';
      el.classList.remove("muted");
      return;
    }
    el.classList.remove("muted");
    var rows = jobs
      .map(function (job) {
        var pct =
          job.total > 0 ? Math.round((100 * job.current) / job.total) : 0;
        var progress =
          job.total > 0
            ? job.current + "/" + job.total + " (" + pct + "%)"
            : "";
        return (
          '<a class="job-row" href="/unpack?job=' +
          encodeURIComponent(job.id) +
          '">' +
          '<span class="job-id">' +
          escapeHtml(job.id) +
          "</span>" +
          '<span class="job-kind">' +
          escapeHtml(job.kind || "") +
          "</span>" +
          '<span class="job-status ' +
          statusClass(job.status) +
          '">' +
          escapeHtml(statusLabel(job.status)) +
          "</span>" +
          '<span class="job-progress">' +
          escapeHtml(progress) +
          "</span>" +
          '<span class="job-time">' +
          escapeHtml(formatTime(job.updated_at)) +
          "</span>" +
          "</a>"
        );
      })
      .join("");
    el.innerHTML = '<div class="job-list">' + rows + "</div>";
  }

  async function load() {
    await HanShell.mount({ active: "hub" });

    try {
      var st = await fetch("/api/status").then(function (r) {
        return r.json();
      });
      renderEnv(st);
    } catch (_err) {
      var env = document.getElementById("env-summary");
      if (env) env.textContent = "无法读取环境状态";
    }

    try {
      var jobs = await fetch("/api/jobs?limit=10").then(function (r) {
        return r.json();
      });
      renderJobs(jobs);
    } catch (_err) {
      var recent = document.getElementById("recent-jobs");
      if (recent) recent.textContent = "无法加载最近任务";
    }
  }

  load();
})();
