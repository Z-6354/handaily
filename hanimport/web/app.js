const $ = (id) => document.getElementById(id);

function appendLog(line, cls = "") {
  const el = $("log");
  const span = document.createElement("span");
  if (cls) span.className = cls;
  span.textContent = line + "\n";
  el.appendChild(span);
  el.scrollTop = el.scrollHeight;
}

function clearLog() {
  $("log").textContent = "";
}

async function api(path, body) {
  const res = await fetch(path, {
    method: body ? "POST" : "GET",
    headers: body ? { "Content-Type": "application/json" } : undefined,
    body: body ? JSON.stringify(body) : undefined,
  });
  const data = await res.json();
  if (!res.ok || data.ok === false) {
    throw new Error(data.error || res.statusText);
  }
  return data;
}

function setJobBusy(busy) {
  $("btn-scan").disabled = busy;
  $("btn-unpack").disabled = busy;
  $("btn-config").disabled = busy;
}

function renderStatus(s) {
  const lines = [
    `Python: ${s.python}`,
    `UnityPy: ${s.unitypy ? "已安装" : "未安装 — 请运行「安装依赖.bat」"}`,
    `仓库根目录: ${s.repo_root}`,
    `默认 Live2D 输出: ${s.default_live2d}`,
    `默认模型输出: ${s.default_model_unpacked}`,
  ];
  $("status-body").innerHTML = lines.map((l) => `<div>${l}</div>`).join("");
}

function escapeHtml(s) {
  return String(s)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

function renderScan(data) {
  if (!data.bundles?.length) {
    $("scan-result").innerHTML = '<span class="muted">未找到 AssetBundle</span>';
    return;
  }
  const items = data.bundles
    .map((b) => {
      const slug = escapeHtml(b.slug);
      const path = escapeHtml(b.path);
      return `<li><label class="check"><input type="checkbox" data-slug="${slug}" checked /> <code>${slug}</code> — ${path}</label></li>`;
    })
    .join("");
  $("scan-result").innerHTML =
    `<div>共 ${data.bundles.length} 个 bundle：</div><ul class="scan-list">${items}</ul>`;
}

function selectedSlugs() {
  const boxes = [...document.querySelectorAll("#scan-result input[data-slug]")];
  if (!boxes.length) return null;
  return boxes.filter((b) => b.checked).map((b) => b.dataset.slug);
}

function renderProgress(job) {
  const wrap = $("progress-wrap");
  wrap.hidden = false;
  const pct = job.total ? Math.round((100 * job.current) / job.total) : 0;
  $("progress-fill").style.width = pct + "%";
  $("progress-label").textContent =
    `${job.phase || job.kind} ${job.current}/${job.total} ${job.current_item || ""} (${pct}%)`;
}

function appendLogTail(job, state) {
  const lines = job.log_tail || [];
  const first = lines.length ? lines[0] : "";
  // Ring buffer full: length stays at cap while head slides — reset sync.
  if (
    lines.length &&
    lines.length === state.seen &&
    state.lastFirst !== undefined &&
    first !== state.lastFirst
  ) {
    state.seen = 0;
    clearLog();
  } else if (lines.length < state.seen) {
    state.seen = 0;
    clearLog();
  }
  for (; state.seen < lines.length; state.seen++) {
    appendLog(lines[state.seen]);
  }
  state.lastFirst = first;
}

/** Only remap unpack/bundle rows into scan checkboxes — skip config-phase results. */
function bundlesFromJobResults(results) {
  return (results || [])
    .filter(
      (r) =>
        r &&
        r.phase !== "config" &&
        r.input &&
        (r.slug || r.folder),
    )
    .map((r) => ({
      slug: r.slug || r.folder || "",
      path: r.input,
    }));
}

async function pollJob(jobId, onTick) {
  for (;;) {
    const data = await api(`/api/jobs/${jobId}`);
    const job = data.job;
    onTick(job);
    if (job.status === "done" || job.status === "error") return job;
    await new Promise((r) => setTimeout(r, 400));
  }
}

async function refreshStatus() {
  try {
    const s = await api("/api/status");
    renderStatus(s);
    if (s.suggested_input && !$("input-path").value) {
      $("input-path").value = s.suggested_input;
    }
  } catch (e) {
    $("status-body").innerHTML = `<span class="err">${e.message}</span>`;
  }
}

async function onScan() {
  const input = $("input-path").value.trim();
  if (!input) {
    appendLog("请先填写输入路径", "err");
    return;
  }
  clearLog();
  appendLog("扫描中…");
  $("btn-scan").disabled = true;
  try {
    const data = await api("/api/scan", { input });
    renderScan(data);
    appendLog(`扫描完成：${data.bundles.length} 个 bundle`, "ok");
  } catch (e) {
    appendLog(e.message, "err");
  } finally {
    $("btn-scan").disabled = false;
  }
}

async function onUnpack() {
  const input = $("input-path").value.trim();
  if (!input) {
    appendLog("请先填写输入路径", "err");
    return;
  }
  const slugs = selectedSlugs();
  if (Array.isArray(slugs) && slugs.length === 0) {
    appendLog("请至少勾选一个 bundle", "err");
    return;
  }
  clearLog();
  appendLog("提交解包任务…");
  setJobBusy(true);
  $("progress-wrap").hidden = true;
  $("progress-fill").style.width = "0%";
  const logState = { seen: 0, lastFirst: undefined };
  try {
    const body = {
      input,
      output: $("output-path").value.trim() || null,
      dry_run: $("dry-run").checked,
      continue_on_error: $("opt-continue").checked,
      generate_config: $("opt-gen-config").checked,
    };
    if (slugs) body.slugs = slugs;
    const { job_id } = await api("/api/jobs/unpack", body);
    appendLog(`任务已启动：${job_id}`);
    const job = await pollJob(job_id, (j) => {
      renderProgress(j);
      appendLogTail(j, logState);
    });
    if (job.status === "error") {
      appendLog(job.error || "任务失败", "err");
    } else {
      appendLog(
        `完成 ok=${job.ok_count || 0} fail=${job.fail_count || 0}`,
        job.fail_count ? "err" : "ok",
      );
    }
    const bundles = bundlesFromJobResults(job.results);
    if (bundles.length) {
      renderScan({ bundles });
    }
  } catch (e) {
    appendLog(e.message, "err");
  } finally {
    setJobBusy(false);
  }
}

async function onConfig() {
  const input = $("input-path").value.trim();
  if (!input) {
    appendLog("请先填写输入路径", "err");
    return;
  }
  clearLog();
  appendLog("提交配置生成任务…");
  setJobBusy(true);
  $("progress-wrap").hidden = true;
  $("progress-fill").style.width = "0%";
  const logState = { seen: 0, lastFirst: undefined };
  try {
    const { job_id } = await api("/api/jobs/config", {
      input,
      dry_run: $("dry-run").checked,
    });
    appendLog(`任务已启动：${job_id}`);
    const job = await pollJob(job_id, (j) => {
      renderProgress(j);
      appendLogTail(j, logState);
    });
    if (job.status === "error") {
      appendLog(job.error || "任务失败", "err");
    } else {
      appendLog(
        `完成 ok=${job.ok_count || 0} fail=${job.fail_count || 0}`,
        job.fail_count ? "err" : "ok",
      );
    }
  } catch (e) {
    appendLog(e.message, "err");
  } finally {
    setJobBusy(false);
  }
}

$("btn-scan").addEventListener("click", onScan);
$("btn-unpack").addEventListener("click", onUnpack);
$("btn-config").addEventListener("click", onConfig);
refreshStatus();
