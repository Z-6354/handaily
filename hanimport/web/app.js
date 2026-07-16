const $ = (id) => document.getElementById(id);

/** Extra absolute file paths appended via system dialog (not the main input). */
let extraFiles = [];

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
  let data;
  try {
    data = await res.json();
  } catch {
    throw new Error(res.statusText || `HTTP ${res.status}`);
  }
  if (!res.ok || data.ok === false) {
    throw new Error(data.error || res.statusText || `HTTP ${res.status}`);
  }
  return data;
}

function setJobBusy(busy) {
  $("btn-scan").disabled = busy;
  $("btn-unpack").disabled = busy;
  $("btn-config").disabled = busy;
  const browseIds = [
    "btn-browse-input",
    "btn-browse-output",
    "btn-browse-files",
    "btn-clear-extra",
  ];
  for (const id of browseIds) {
    const el = $(id);
    if (!el) continue;
    if (id === "btn-clear-extra") {
      el.disabled = busy || !extraFiles.length;
    } else {
      el.disabled = busy;
    }
  }
}

function collectInputs() {
  const main = ($("input-path").value || "").trim();
  const inputs = [];
  if (main) inputs.push(main);
  for (const p of extraFiles) {
    if (p && !inputs.some((x) => x.toLowerCase() === p.toLowerCase())) {
      inputs.push(p);
    }
  }
  return inputs;
}

function renderExtraFiles() {
  const el = $("extra-files");
  const clearBtn = $("btn-clear-extra");
  if (!el) return;
  if (!extraFiles.length) {
    el.className = "extra-files muted";
    el.textContent = "尚未添加";
    if (clearBtn) clearBtn.disabled = true;
    return;
  }
  el.className = "extra-files";
  el.innerHTML = extraFiles
    .map((p, i) => {
      const path = escapeHtml(p);
      return (
        `<li><span class="extra-path" title="${path}">${path}</span>` +
        `<button type="button" class="ghost extra-remove" data-extra-i="${i}">移除</button></li>`
      );
    })
    .join("");
  if (clearBtn) clearBtn.disabled = false;
  el.querySelectorAll("[data-extra-i]").forEach((btn) => {
    btn.addEventListener("click", () => {
      const i = Number(btn.dataset.extraI);
      if (!Number.isFinite(i)) return;
      extraFiles.splice(i, 1);
      renderExtraFiles();
    });
  });
}

async function pickFolderInto(inputId, title) {
  try {
    const data = await api("/api/dialog/folder", { title });
    if (data.cancelled) return;
    if (data.path) $(inputId).value = data.path;
  } catch (e) {
    appendLog(e.message, "err");
  }
}

async function pickExtraFiles() {
  try {
    const data = await api("/api/dialog/files", { title: "选择要解包的文件" });
    if (data.cancelled) return;
    const paths = data.paths || [];
    let added = 0;
    for (const p of paths) {
      const s = String(p || "").trim();
      if (!s) continue;
      if (extraFiles.some((x) => x.toLowerCase() === s.toLowerCase())) continue;
      extraFiles.push(s);
      added += 1;
    }
    renderExtraFiles();
    if (added) appendLog(`已添加 ${added} 个文件`, "ok");
  } catch (e) {
    appendLog(e.message, "err");
  }
}

function renderStatus(s) {
  const lines = [
    `Python: ${s.python}`,
    `UnityPy: ${s.unitypy ? "已安装" : "未安装 — 请运行「安装依赖.bat」"}`,
    `仓库根目录: ${s.repo_root}`,
    `默认 Live2D 输出: ${s.default_live2d}`,
    `默认模型输出: ${s.default_model_unpacked}`,
  ];
  $("status-body").innerHTML = lines
    .map((l) => `<div>${escapeHtml(l)}</div>`)
    .join("");
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
  const warnHtml =
    data.warnings && data.warnings.length
      ? `<div class="warn-list">${data.warnings
          .map((w) => `<div class="muted">${escapeHtml(w)}</div>`)
          .join("")}</div>`
      : "";
  const items = data.bundles
    .map((b) => {
      const slug = escapeHtml(b.slug);
      const path = escapeHtml(b.path);
      const pathAttr = escapeHtml(b.path);
      return (
        `<li><label class="check">` +
        `<input type="checkbox" data-slug="${slug}" data-path="${pathAttr}" checked /> ` +
        `<code>${slug}</code> — ${path}</label></li>`
      );
    })
    .join("");
  $("scan-result").innerHTML =
    `<div>共 ${data.bundles.length} 个 bundle：</div>${warnHtml}<ul class="scan-list">${items}</ul>`;
}

function selectedSlugs() {
  const boxes = [...document.querySelectorAll("#scan-result input[data-slug]")];
  if (!boxes.length) return null;
  return boxes.filter((b) => b.checked).map((b) => b.dataset.slug);
}

function selectedPaths() {
  const boxes = [...document.querySelectorAll("#scan-result input[data-path]")];
  if (!boxes.length) return null;
  return boxes.filter((b) => b.checked).map((b) => b.dataset.path);
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
  const started = Date.now();
  const maxMs = 6 * 60 * 60 * 1000; // 6h hard cap for local batches
  for (;;) {
    if (Date.now() - started > maxMs) {
      throw new Error("任务轮询超时（超过 6 小时）");
    }
    const data = await api(`/api/jobs/${encodeURIComponent(jobId)}`);
    const job = data.job;
    if (!job) throw new Error("job not found");
    onTick(job);
    if (job.status === "done" || job.status === "error") return job;
    const updated = Number(job.updated_at) || 0;
    // Worker crashed / hung without terminal status
    if (updated && Date.now() / 1000 - updated > 600) {
      throw new Error("任务超过 10 分钟无更新，可能已中断 — 可重新开始解包");
    }
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
  const inputs = collectInputs();
  if (!inputs.length) {
    appendLog("请先选择输入文件夹，或添加文件", "err");
    return;
  }
  clearLog();
  appendLog("扫描中…");
  $("btn-scan").disabled = true;
  try {
    const data = await api("/api/scan", {
      input: inputs[0],
      inputs,
    });
    renderScan(data);
    appendLog(`扫描完成：${data.bundles.length} 个 bundle`, "ok");
    for (const w of data.warnings || []) appendLog(w);
  } catch (e) {
    appendLog(e.message, "err");
  } finally {
    $("btn-scan").disabled = false;
  }
}

async function onUnpack() {
  const inputs = collectInputs();
  if (!inputs.length) {
    appendLog("请先选择输入文件夹，或添加文件", "err");
    return;
  }
  const paths = selectedPaths();
  const slugs = selectedSlugs();
  if (Array.isArray(paths) && paths.length === 0) {
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
      input: inputs[0],
      inputs,
      output: $("output-path").value.trim() || null,
      dry_run: $("dry-run").checked,
      continue_on_error: $("opt-continue").checked,
      generate_config: $("opt-gen-config").checked,
    };
    if (paths && paths.length) body.paths = paths;
    else if (slugs) body.slugs = slugs;
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
  const out = ($("output-path").value || "").trim();
  const input = out || ($("input-path").value || "").trim();
  if (!input) {
    appendLog("请先填写输出目录（或输入路径）", "err");
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
$("btn-browse-input")?.addEventListener("click", () =>
  pickFolderInto("input-path", "选择输入文件夹"),
);
$("btn-browse-output")?.addEventListener("click", () =>
  pickFolderInto("output-path", "选择输出目录"),
);
$("btn-browse-files")?.addEventListener("click", pickExtraFiles);
$("btn-clear-extra")?.addEventListener("click", () => {
  extraFiles = [];
  renderExtraFiles();
});
renderExtraFiles();
refreshStatus();

async function resumeJobFromUrl() {
  const jobId = new URLSearchParams(location.search).get("job");
  if (!jobId) return;

  const banner = $("job-banner");
  let job;
  try {
    const res = await fetch(`/api/jobs/${encodeURIComponent(jobId)}`);
    if (!res.ok) {
      if (banner) {
        banner.textContent = "任务不存在或已清理";
        banner.hidden = false;
      }
      return;
    }
    const data = await res.json();
    job = data.job;
    if (!job) {
      if (banner) {
        banner.textContent = "任务不存在或已清理";
        banner.hidden = false;
      }
      return;
    }
  } catch (_e) {
    if (banner) {
      banner.textContent = "无法连接服务器，请确认开发服已启动";
      banner.hidden = false;
    }
    return;
  }

  setJobBusy(true);
  $("progress-wrap").hidden = false;
  clearLog();
  appendLog(`恢复任务：${jobId}`);
  const logState = { seen: 0, lastFirst: undefined };
  const onTick = (j) => {
    renderProgress(j);
    appendLogTail(j, logState);
  };
  onTick(job);

  try {
    if (job.status === "done" || job.status === "error") {
      if (job.status === "error") {
        appendLog(job.error || "任务失败", "err");
      } else {
        appendLog(
          `完成 ok=${job.ok_count || 0} fail=${job.fail_count || 0}`,
          job.fail_count ? "err" : "ok",
        );
      }
      const bundles = bundlesFromJobResults(job.results);
      if (bundles.length) renderScan({ bundles });
      return;
    }
    const finished = await pollJob(jobId, onTick);
    if (finished.status === "error") {
      appendLog(finished.error || "任务失败", "err");
    } else {
      appendLog(
        `完成 ok=${finished.ok_count || 0} fail=${finished.fail_count || 0}`,
        finished.fail_count ? "err" : "ok",
      );
    }
    const bundles = bundlesFromJobResults(finished.results);
    if (bundles.length) renderScan({ bundles });
  } catch (e) {
    appendLog(e.message, "err");
  } finally {
    setJobBusy(false);
  }
}

resumeJobFromUrl();
