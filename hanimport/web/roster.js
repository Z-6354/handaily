(() => {
  "use strict";

  let db = "local";
  let metaPath = "";
  let offset = 0;
  let limit = 48;
  let pipelineJobId = null;
  let pipelinePollTimer = null;
  let pipelineToastHidden = false;
  let pipelineAutoStarted = false;
  let total = 0;
  let selectedCharId = null;
  let selectedSkinId = null;
  let selectedLineId = null;
  let creatingChar = false;
  let creatingSkin = false;
  let creatingLine = false;
  let searchTimer = null;

  const $ = (id) => document.getElementById(id);
  const logEl = $("log");

  function appendLog(msg, cls) {
    if (!logEl) return;
    const line = document.createElement("div");
    if (cls) line.className = cls;
    line.textContent = typeof msg === "string" ? msg : JSON.stringify(msg, null, 2);
    logEl.appendChild(line);
    logEl.scrollTop = logEl.scrollHeight;
  }

  function summarize(data) {
    if (!data || typeof data !== "object") return String(data);
    const keys = ["ok", "filled", "error", "message", "counts", "allowlist", "bundled_db", "deleted", "skins_lines_ok", "skins_lines_empty", "wiki_skins_unmatched", "roster_skins_unmatched"];
    const pick = {};
    for (const k of keys) {
      if (k in data) pick[k] = data[k];
    }
    if (Object.keys(pick).length === 0) return JSON.stringify(data).slice(0, 400);
    return JSON.stringify(pick);
  }

  async function rosterFetch(path, opts = {}) {
    const method = (opts.method || "GET").toUpperCase();
    const u = new URL(path, location.origin);
    u.searchParams.set("db", db);
    const needsBody = method !== "GET" && method !== "HEAD";
    let body = opts.body ? { ...opts.body } : needsBody ? {} : undefined;

    if (needsBody && db === "bundled") {
      if (
        !confirm(
          "确认写入自带预览库？\n" +
            metaPath +
            "\n可能进入发行预览包"
        )
      ) {
        throw new Error("cancelled");
      }
      body.confirm_bundled = true;
    }

    const init = { method, headers: {} };
    if (needsBody) {
      init.headers["Content-Type"] = "application/json";
      init.body = JSON.stringify(body || {});
    }
    const res = await fetch(u.toString(), init);
    let data = {};
    try {
      data = await res.json();
    } catch {
      data = { ok: false, error: res.statusText || "invalid JSON" };
    }
    if (!res.ok || data.ok === false) {
      throw new Error(data.error || res.statusText || `HTTP ${res.status}`);
    }
    return data;
  }

  function setDb(next) {
    db = next;
    $("db-local").classList.toggle("active", db === "local");
    $("db-bundled").classList.toggle("active", db === "bundled");
    $("bundled-badge").hidden = db !== "bundled";
    const localOnly = db === "local";
    $("btn-sync-appdata").disabled = !localOnly;
    $("btn-publish").disabled = !localOnly;
    offset = 0;
    selectedCharId = null;
    selectedSkinId = null;
    selectedLineId = null;
    creatingChar = false;
    pipelineAutoStarted = false;
    pipelineToastHidden = false;
    if (db !== "local") {
      showPipelineToast(false);
    }
    clearDetail();
    refreshAll();
  }

  function clearDetail() {
    $("char-empty").hidden = false;
    $("char-form").hidden = true;
    $("skins-block").hidden = true;
    $("btn-save-char").disabled = true;
    $("btn-del-char").disabled = true;
    $("btn-new-line").disabled = true;
    $("lines-empty").hidden = false;
    $("line-list").innerHTML = "";
    $("line-form").hidden = true;
    $("skin-form").hidden = true;
  }

  function fillEnOnBlur(input, idGetter) {
    input.addEventListener("blur", () => {
      if (!(input.value || "").trim()) {
        const id = idGetter();
        if (id) input.value = id;
      }
    });
  }

  async function loadMeta() {
    const data = await rosterFetch("/api/roster/meta");
    metaPath = data.path || "";
    $("meta-path").textContent = metaPath;
    const c = data.counts || {};
    $("meta-counts").textContent =
      `角色 ${c.characters ?? 0} · 皮肤 ${c.skins ?? 0} · 台词 ${c.lines ?? 0}`;
  }

  function enDisplay(en, id) {
    if ((en || "").trim()) return en;
    return `<span class="en-placeholder">${escapeHtml(id)}</span>`;
  }

  function escapeHtml(s) {
    return String(s)
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;")
      .replace(/"/g, "&quot;");
  }

  async function loadCharacters() {
    const q = ($("char-search").value || "").trim();
    const params = new URLSearchParams({
      offset: String(offset),
      limit: String(limit),
    });
    if (q) params.set("q", q);
    const data = await rosterFetch(`/api/roster/characters?${params}`);
    total = data.total || 0;
    const list = $("char-list");
    list.innerHTML = "";
    let missing = 0;
    for (const ch of data.characters || []) {
      const card = document.createElement("button");
      card.type = "button";
      card.className = "char-card";
      card.dataset.id = ch.id;
      card.setAttribute("role", "listitem");
      if (ch.id === selectedCharId) card.classList.add("selected");
      const nameZh = ch.name_zh || ch.id;
      const initial = String(nameZh).trim().charAt(0) || "?";
      if (ch.avatar_url) {
        card.innerHTML =
          `<img class="char-av" src="${escapeHtml(ch.avatar_url)}" alt="" loading="lazy" />` +
          `<div class="char-card-name">${escapeHtml(nameZh)}</div>` +
          `<div class="char-card-id">${escapeHtml(ch.id)}</div>`;
      } else {
        missing += 1;
        card.innerHTML =
          `<div class="char-av char-av-fallback" aria-hidden="true">${escapeHtml(initial)}</div>` +
          `<div class="char-card-name">${escapeHtml(nameZh)}</div>` +
          `<div class="char-card-id">${escapeHtml(ch.id)}</div>`;
      }
      card.addEventListener("click", () => selectCharacter(ch.id));
      list.appendChild(card);
    }
    $("pager-label").textContent = total
      ? `${offset + 1}–${Math.min(offset + limit, total)} / ${total}`
      : "0";
    $("btn-prev").disabled = offset <= 0;
    $("btn-next").disabled = offset + limit >= total;

    if (db === "local" && !pipelineAutoStarted) {
      pipelineAutoStarted = true;
      maybeStartWikiPipeline(true);
    }
  }

  const PHASE_TITLE = {
    characters: "Wiki 补齐 · 同步角色",
    avatars_skins: "Wiki 补齐 · 头像与皮肤",
    lines: "Wiki 补齐 · 导入台词",
    done: "Wiki 补齐 · 完成",
  };

  function showPipelineToast(show) {
    const el = $("pipeline-toast");
    if (!el) return;
    if (pipelineToastHidden && show) return;
    el.hidden = !show;
  }

  function renderPipelineToast(job) {
    if (!job) return;
    const title = $("pipeline-toast-title");
    const sub = $("pipeline-toast-sub");
    const fill = $("pipeline-toast-fill");
    const counts = $("pipeline-toast-counts");
    const btn = $("pipeline-toast-pause");
    const total = job.total || 0;
    const current = job.current || 0;
    const pct = total ? Math.round((100 * current) / total) : 0;
    fill.style.width = pct + "%";
    counts.textContent =
      `${current}/${total} · 成功 ${job.ok_count || 0} · 跳过 ${job.skip_count || 0} · 失败 ${job.fail_count || 0}`;

    const phaseLabel = PHASE_TITLE[job.phase] || "Wiki 补齐";
    if (job.status === "paused") {
      title.textContent = phaseLabel + " · 已暂停";
      btn.hidden = false;
      btn.textContent = "继续";
      sub.textContent = job.current_item ? `当前：${job.current_item}` : "队列已暂停";
    } else if (job.status === "done") {
      title.textContent = PHASE_TITLE.done;
      btn.hidden = true;
      sub.textContent = `台词就绪 ${job.ok_count || 0} · 空 ${job.skip_count || 0} · 未匹配 ${job.fail_count || 0}`;
    } else if (job.status === "error") {
      title.textContent = "Wiki 补齐失败";
      btn.hidden = true;
      sub.textContent = job.error || "未知错误";
    } else {
      title.textContent = phaseLabel;
      btn.hidden = false;
      btn.textContent = "暂停";
      sub.textContent = job.current_item
        ? `当前：${job.current_item}`
        : "角色 → 头像/皮肤 → 台词";
    }
    showPipelineToast(true);
  }

  async function pollPipelineJob(jobId) {
    pipelineJobId = jobId;
    if (pipelinePollTimer) clearInterval(pipelinePollTimer);
    const tick = async () => {
      try {
        const res = await fetch(`/api/jobs/${encodeURIComponent(jobId)}`);
        if (!res.ok) return;
        const data = await res.json();
        const job = data.job;
        if (!job) return;
        renderPipelineToast(job);
        if (job.status === "done" || job.status === "error") {
          clearInterval(pipelinePollTimer);
          pipelinePollTimer = null;
          await loadCharacters();
          if (selectedCharId) await loadCharacterDetail(selectedCharId);
          if (job.status === "done") {
            setTimeout(() => {
              pipelineToastHidden = false;
              showPipelineToast(false);
            }, 4200);
          }
        } else if (job.phase === "avatars_skins" && job.ok_count > 0) {
          await loadCharacters();
        }
      } catch (_e) {
        /* ignore transient */
      }
    };
    await tick();
    pipelinePollTimer = setInterval(tick, 600);
  }

  async function maybeStartWikiPipeline(force) {
    if (db !== "local") return;
    try {
      const jobsRes = await fetch("/api/jobs?limit=20");
      if (jobsRes.ok) {
        const payload = await jobsRes.json();
        const active = (payload.jobs || []).find(
          (j) =>
            j.kind === "roster-wiki-pipeline" &&
            ["queued", "running", "paused"].includes(j.status)
        );
        if (active) {
          pipelineToastHidden = false;
          await pollPipelineJob(active.id);
          return;
        }
      }
      if (!force) return;
      const data = await rosterFetch("/api/roster/ops/wiki-pipeline", {
        method: "POST",
        body: {},
      });
      if (data.job_id) {
        pipelineToastHidden = false;
        await pollPipelineJob(data.job_id);
      }
    } catch (e) {
      if (e.message !== "cancelled") {
        appendLog("Wiki 补齐启动失败: " + (e.message || e), "err");
      }
    }
  }

  /* pipeline toast owns auto fill */

  async function selectCharacter(id) {
    creatingChar = false;
    selectedCharId = id;
    selectedSkinId = null;
    selectedLineId = null;
    creatingSkin = false;
    creatingLine = false;
    await loadCharacterDetail(id);
    await loadCharacters();
  }

  async function loadCharacterDetail(id) {
    const data = await rosterFetch(`/api/roster/characters/${encodeURIComponent(id)}`);
    const ch = data.character;
    $("char-empty").hidden = true;
    $("char-form").hidden = false;
    $("skins-block").hidden = false;
    $("btn-save-char").disabled = false;
    $("btn-del-char").disabled = false;
    $("f-id").value = ch.id;
    $("f-id").readOnly = true;
    $("f-name-zh").value = ch.name_zh || "";
    $("f-name-en").value = ch.name_en || "";
    $("f-wiki").value = ch.wiki_title || "";
    $("f-cv").value = ch.cv || "";
    $("f-faction").value = ch.faction || "";
    $("f-ship-type").value = ch.ship_type || "";
    $("f-rarity").value = ch.rarity || "";
    $("f-persona").value = ch.persona_id || "";
    $("f-desc").value = ch.description || "";
    renderSkins(data.skins || []);
    clearLinePanel();
  }

  function startNewCharacter() {
    creatingChar = true;
    selectedCharId = null;
    selectedSkinId = null;
    $("char-empty").hidden = true;
    $("char-form").hidden = false;
    $("skins-block").hidden = true;
    $("btn-save-char").disabled = false;
    $("btn-del-char").disabled = true;
    $("f-id").value = "";
    $("f-id").readOnly = false;
    $("f-name-zh").value = "";
    $("f-name-en").value = "";
    $("f-wiki").value = "";
    $("f-cv").value = "";
    $("f-faction").value = "";
    $("f-ship-type").value = "";
    $("f-rarity").value = "";
    $("f-persona").value = "";
    $("f-desc").value = "";
    $("skin-list").innerHTML = "";
    clearLinePanel();
    document.querySelectorAll("#char-list li.selected").forEach((el) => el.classList.remove("selected"));
  }

  function charPayload() {
    const id = ($("f-id").value || "").trim();
    return {
      id,
      name_zh: ($("f-name-zh").value || "").trim(),
      name_en: ($("f-name-en").value || "").trim(),
      wiki_title: ($("f-wiki").value || "").trim(),
      cv: ($("f-cv").value || "").trim(),
      faction: ($("f-faction").value || "").trim(),
      ship_type: ($("f-ship-type").value || "").trim(),
      rarity: ($("f-rarity").value || "").trim(),
      persona_id: ($("f-persona").value || "").trim() || id,
      description: ($("f-desc").value || "").trim(),
    };
  }

  async function saveCharacter() {
    const body = charPayload();
    if (!body.id || !body.name_zh) {
      appendLog("id 与中文名必填", "err");
      return;
    }
    try {
      if (creatingChar) {
        const data = await rosterFetch("/api/roster/characters", { method: "POST", body });
        appendLog("新建角色 " + summarize(data), "ok");
        creatingChar = false;
        selectedCharId = body.id;
        $("f-id").readOnly = true;
      } else {
        const data = await rosterFetch(
          `/api/roster/characters/${encodeURIComponent(body.id)}`,
          { method: "PUT", body }
        );
        appendLog("更新角色 " + summarize(data), "ok");
      }
      await refreshAll();
      if (selectedCharId) await loadCharacterDetail(selectedCharId);
    } catch (e) {
      if (e.message !== "cancelled") appendLog(String(e.message || e), "err");
    }
  }

  async function deleteCharacter() {
    if (!selectedCharId || creatingChar) return;
    const nameZh = ($("f-name-zh").value || "").trim();
    if (!confirm(`删除角色？\nid: ${selectedCharId}\n中文名: ${nameZh}`)) return;
    try {
      const data = await rosterFetch(
        `/api/roster/characters/${encodeURIComponent(selectedCharId)}`,
        { method: "DELETE", body: {} }
      );
      appendLog("删除角色 " + summarize(data), "ok");
      selectedCharId = null;
      clearDetail();
      await refreshAll();
    } catch (e) {
      if (e.message !== "cancelled") appendLog(String(e.message || e), "err");
    }
  }

  function statusDot(kind, status, idHint) {
    const label =
      status === "ready" ? "就绪" : status === "missing" ? "缺文件" : "未绑定";
    const tip = idHint ? `${label} · ${idHint}` : label;
    return (
      `<span class="probe-cell probe-${escapeHtml(status || "unbound")}" title="${escapeHtml(tip)}">` +
      `<span class="probe-dot" aria-hidden="true"></span>` +
      `<span class="probe-label">${escapeHtml(label)}</span>` +
      (idHint
        ? `<code class="probe-id">${escapeHtml(idHint)}</code>`
        : "") +
      `</span>`
    );
  }

  function linesStatusDot(status, wikiSkin, count) {
    const map = {
      ready: "就绪",
      empty: "缺台词",
      unmatched: "未匹配",
      stale_flat: "旧复制",
    };
    const label = map[status] || status || "缺台词";
    const tipParts = [label];
    if (count != null) tipParts.push(`${count} 条`);
    if (wikiSkin) tipParts.push(`Wiki: ${wikiSkin}`);
    const tip = tipParts.join(" · ");
    const cls = status === "ready" ? "probe-ready" : status === "unmatched" || status === "stale_flat" ? "probe-missing" : "probe-unbound";
    return (
      `<span class="probe-cell ${cls}" title="${escapeHtml(tip)}">` +
      `<span class="probe-dot" aria-hidden="true"></span>` +
      `<span class="probe-label">${escapeHtml(label)}</span>` +
      `</span>`
    );
  }

  function renderSkins(skins) {
    const list = $("skin-list");
    list.innerHTML = "";
    $("skin-form").hidden = true;
    const link = $("link-all-skins");
    if (link && selectedCharId) {
      link.href =
        `/skins?db=${encodeURIComponent(db)}&character_id=${encodeURIComponent(selectedCharId)}`;
    }
    for (const sk of skins) {
      const tr = document.createElement("tr");
      tr.dataset.id = sk.id;
      if (sk.id === selectedSkinId) tr.classList.add("selected");
      const def = sk.is_default ? '<span class="def-tag">默认</span>' : "";
      tr.innerHTML =
        `<td class="skin-name">` +
        `<div class="id">${escapeHtml(sk.id)}</div>` +
        `<div>${escapeHtml(sk.name_zh || "")}${def}</div>` +
        `</td>` +
        `<td>${statusDot("pet", sk.pet_status, sk.pet_model_id || "")}</td>` +
        `<td>${statusDot("km", sk.kanmusu_status, sk.kanmusu_dir || "")}</td>` +
        `<td>${linesStatusDot(sk.lines_status, sk.lines_wiki_skin, sk.lines_count)}</td>` +
        `<td class="skin-pick">编辑</td>`;
      tr.addEventListener("click", () => selectSkin(sk));
      list.appendChild(tr);
    }
  }

  function selectSkin(sk) {
    creatingSkin = false;
    selectedSkinId = sk.id;
    selectedLineId = null;
    creatingLine = false;
    document.querySelectorAll("#skin-list tr").forEach((el) => {
      el.classList.toggle("selected", el.dataset.id === sk.id);
    });
    $("skin-form").hidden = false;
    $("s-id").value = sk.id;
    $("s-id").readOnly = true;
    $("s-name-zh").value = sk.name_zh || "";
    $("s-name-en").value = sk.name_en || "";
    $("s-pet").value = sk.pet_model_id || "";
    $("s-kanmusu").value = sk.kanmusu_dir || "";
    $("s-sort").value = sk.sort_order ?? 0;
    $("s-default").checked = !!sk.is_default;
    $("btn-new-line").disabled = false;
    loadLines(sk.id);
  }

  function startNewSkin() {
    if (!selectedCharId || creatingChar) {
      appendLog("请先保存/选择角色", "err");
      return;
    }
    creatingSkin = true;
    selectedSkinId = null;
    selectedLineId = null;
    $("skin-form").hidden = false;
    $("s-id").value = "";
    $("s-id").readOnly = false;
    $("s-name-zh").value = "";
    $("s-name-en").value = "";
    $("s-pet").value = "";
    $("s-kanmusu").value = "";
    $("s-sort").value = 0;
    $("s-default").checked = false;
    $("btn-new-line").disabled = true;
    clearLinePanel();
    document.querySelectorAll("#skin-list tr.selected").forEach((el) => el.classList.remove("selected"));
  }

  function skinPayload() {
    const id = ($("s-id").value || "").trim();
    return {
      id,
      character_id: selectedCharId,
      name_zh: ($("s-name-zh").value || "").trim(),
      name_en: ($("s-name-en").value || "").trim(),
      pet_model_id: ($("s-pet").value || "").trim(),
      kanmusu_dir: ($("s-kanmusu").value || "").trim(),
      sort_order: Number($("s-sort").value || 0),
      is_default: $("s-default").checked,
    };
  }

  async function saveSkin() {
    const body = skinPayload();
    if (!body.id || !body.name_zh || !body.character_id) {
      appendLog("皮肤 id、中文名与角色必填", "err");
      return;
    }
    try {
      if (creatingSkin) {
        const data = await rosterFetch("/api/roster/skins", { method: "POST", body });
        appendLog("新建皮肤 " + summarize(data), "ok");
        creatingSkin = false;
        selectedSkinId = body.id;
      } else {
        const data = await rosterFetch(
          `/api/roster/skins/${encodeURIComponent(body.id)}`,
          { method: "PUT", body }
        );
        appendLog("更新皮肤 " + summarize(data), "ok");
      }
      await loadCharacterDetail(selectedCharId);
      const sk = { ...body, is_default: body.is_default };
      selectSkin(sk);
    } catch (e) {
      if (e.message !== "cancelled") appendLog(String(e.message || e), "err");
    }
  }

  async function deleteSkin() {
    if (!selectedSkinId || creatingSkin) return;
    const nameZh = ($("s-name-zh").value || "").trim();
    if (!confirm(`删除皮肤？\nid: ${selectedSkinId}\n中文名: ${nameZh}`)) return;
    try {
      const data = await rosterFetch(
        `/api/roster/skins/${encodeURIComponent(selectedSkinId)}`,
        { method: "DELETE", body: {} }
      );
      appendLog("删除皮肤 " + summarize(data), "ok");
      selectedSkinId = null;
      await loadCharacterDetail(selectedCharId);
      clearLinePanel();
    } catch (e) {
      if (e.message !== "cancelled") appendLog(String(e.message || e), "err");
    }
  }

  function clearLinePanel() {
    $("lines-empty").hidden = false;
    $("line-list").innerHTML = "";
    $("line-form").hidden = true;
    $("btn-new-line").disabled = !selectedSkinId || creatingSkin;
  }

  async function loadLines(skinId) {
    const data = await rosterFetch(
      `/api/roster/skins/${encodeURIComponent(skinId)}/lines`
    );
    $("lines-empty").hidden = true;
    const list = $("line-list");
    list.innerHTML = "";
    $("line-form").hidden = true;
    for (const ln of data.lines || []) {
      const li = document.createElement("li");
      li.dataset.id = String(ln.id);
      if (String(ln.id) === String(selectedLineId)) li.classList.add("selected");
      const label = ln.label || ln.wiki_key || `#${ln.id}`;
      li.innerHTML =
        `<div>${escapeHtml(label)}</div>` +
        `<div class="meta-line">${escapeHtml((ln.text || "").slice(0, 80))}</div>`;
      li.addEventListener("click", () => selectLine(ln));
      list.appendChild(li);
    }
    if (!(data.lines || []).length) {
      $("lines-empty").hidden = false;
      $("lines-empty").textContent = "暂无台词";
    }
  }

  function selectLine(ln) {
    creatingLine = false;
    selectedLineId = ln.id;
    document.querySelectorAll("#line-list li").forEach((el) => {
      el.classList.toggle("selected", el.dataset.id === String(ln.id));
    });
    $("line-form").hidden = false;
    $("l-label").value = ln.label || "";
    $("l-wiki-key").value = ln.wiki_key || "";
    $("l-text").value = ln.text || "";
    $("l-anim").value = ln.animation || "";
    $("l-sort").value = ln.sort_order ?? 0;
  }

  function startNewLine() {
    if (!selectedSkinId) return;
    creatingLine = true;
    selectedLineId = null;
    $("lines-empty").hidden = true;
    $("line-form").hidden = false;
    $("l-label").value = "";
    $("l-wiki-key").value = "";
    $("l-text").value = "";
    $("l-anim").value = "";
    $("l-sort").value = 0;
    document.querySelectorAll("#line-list li.selected").forEach((el) => el.classList.remove("selected"));
  }

  function linePayload() {
    return {
      label: ($("l-label").value || "").trim(),
      wiki_key: ($("l-wiki-key").value || "").trim(),
      text: ($("l-text").value || "").trim(),
      animation: ($("l-anim").value || "").trim(),
      sort_order: Number($("l-sort").value || 0),
    };
  }

  async function saveLine() {
    const body = linePayload();
    if (!body.text) {
      appendLog("台词 text 必填", "err");
      return;
    }
    try {
      if (creatingLine) {
        const data = await rosterFetch(
          `/api/roster/skins/${encodeURIComponent(selectedSkinId)}/lines`,
          { method: "POST", body }
        );
        appendLog("新建台词 " + summarize(data), "ok");
        creatingLine = false;
        selectedLineId = data.line && data.line.id;
      } else {
        const data = await rosterFetch(`/api/roster/lines/${selectedLineId}`, {
          method: "PUT",
          body,
        });
        appendLog("更新台词 " + summarize(data), "ok");
      }
      await loadLines(selectedSkinId);
      if (selectedLineId) {
        const data = await rosterFetch(
          `/api/roster/skins/${encodeURIComponent(selectedSkinId)}/lines`
        );
        const ln = (data.lines || []).find((x) => x.id === selectedLineId);
        if (ln) selectLine(ln);
      }
    } catch (e) {
      if (e.message !== "cancelled") appendLog(String(e.message || e), "err");
    }
  }

  async function deleteLine() {
    if (!selectedLineId || creatingLine) return;
    const label = ($("l-label").value || "").trim() || `#${selectedLineId}`;
    if (!confirm(`删除台词？\nid: ${selectedLineId}\nlabel: ${label}`)) return;
    try {
      const data = await rosterFetch(`/api/roster/lines/${selectedLineId}`, {
        method: "DELETE",
        body: {},
      });
      appendLog("删除台词 " + summarize(data), "ok");
      selectedLineId = null;
      $("line-form").hidden = true;
      await loadLines(selectedSkinId);
    } catch (e) {
      if (e.message !== "cancelled") appendLog(String(e.message || e), "err");
    }
  }

  async function runOp(name, path, body) {
    appendLog(`→ ${name} …`);
    try {
      const data = await rosterFetch(path, { method: "POST", body: body || {} });
      appendLog(`${name} 完成 ` + summarize(data), "ok");
      if (
        name.includes("Wiki") &&
        (data.wiki_skins_unmatched || data.roster_skins_unmatched || data.skins_lines_empty)
      ) {
        appendLog(
          `台词需检查：Wiki未匹配 ${data.wiki_skins_unmatched || 0} · 库皮未匹配 ${data.roster_skins_unmatched || 0} · 空台词 ${data.skins_lines_empty || 0}`,
          "err"
        );
        for (const item of (data.lines_report || []).slice(0, 12)) {
          appendLog("  · " + JSON.stringify(item), "muted");
        }
      }
      await refreshAll();
      if (selectedCharId) await loadCharacterDetail(selectedCharId);
      return data;
    } catch (e) {
      if (e.message === "cancelled") {
        appendLog(`${name} 已取消`, "muted");
        return null;
      }
      appendLog(`${name} 失败: ${e.message || e}`, "err");
      return null;
    }
  }

  async function onPublish() {
    if (
      !confirm(
        "发布自带库：将按 bundled-allowlist.json 覆盖 hanpet/bundled 预览库。\n确认继续？"
      )
    ) {
      appendLog("发布已取消", "muted");
      return;
    }
    await runOp("发布自带库", "/api/roster/ops/publish-bundled", {
      confirm_bundled: true,
    });
  }

  async function refreshAll() {
    try {
      await loadMeta();
      await loadCharacters();
    } catch (e) {
      appendLog("刷新失败: " + (e.message || e), "err");
    }
  }

  function bind() {
    $("db-local").addEventListener("click", () => setDb("local"));
    $("db-bundled").addEventListener("click", () => setDb("bundled"));
    $("btn-new-char").addEventListener("click", startNewCharacter);
    $("btn-save-char").addEventListener("click", saveCharacter);
    $("btn-del-char").addEventListener("click", deleteCharacter);
    $("btn-new-skin").addEventListener("click", startNewSkin);
    $("btn-save-skin").addEventListener("click", saveSkin);
    $("btn-del-skin").addEventListener("click", deleteSkin);
    $("btn-new-line").addEventListener("click", startNewLine);
    $("btn-save-line").addEventListener("click", saveLine);
    $("btn-del-line").addEventListener("click", deleteLine);

    $("btn-prev").addEventListener("click", () => {
      offset = Math.max(0, offset - limit);
      loadCharacters().catch((e) => appendLog(String(e.message || e), "err"));
    });
    $("btn-next").addEventListener("click", () => {
      offset += limit;
      loadCharacters().catch((e) => appendLog(String(e.message || e), "err"));
    });
    $("char-search").addEventListener("input", () => {
      clearTimeout(searchTimer);
      searchTimer = setTimeout(() => {
        offset = 0;
        loadCharacters().catch((e) => appendLog(String(e.message || e), "err"));
      }, 250);
    });

    fillEnOnBlur($("f-name-en"), () => ($("f-id").value || "").trim());
    fillEnOnBlur($("s-name-en"), () => ($("s-id").value || "").trim());

    $("btn-sync-appdata").addEventListener("click", () => {
      if (db !== "local") {
        appendLog("同步 AppData 仅支持自用库，请切回自用库", "err");
        return;
      }
      runOp("同步 AppData", "/api/roster/ops/sync-appdata", {});
    });
    $("btn-publish").addEventListener("click", onPublish);
    $("btn-fill-english").addEventListener("click", () => {
      runOp("补齐空英文名", "/api/roster/ops/fill-english", {});
    });

    $("pipeline-toast-close")?.addEventListener("click", () => {
      pipelineToastHidden = true;
      showPipelineToast(false);
    });
    $("pipeline-toast-pause")?.addEventListener("click", async () => {
      if (!pipelineJobId) return;
      const btn = $("pipeline-toast-pause");
      const action = btn.textContent === "继续" ? "resume" : "pause";
      try {
        const res = await fetch(
          `/api/jobs/${encodeURIComponent(pipelineJobId)}/${action}`,
          {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: "{}",
          }
        );
        const data = await res.json();
        if (data.job) renderPipelineToast(data.job);
      } catch (e) {
        appendLog("Wiki 补齐控制失败: " + (e.message || e), "err");
      }
    });
  }

  if (window.HanShell) HanShell.mount({ active: "roster" });

  bind();
  const boot = new URLSearchParams(location.search);
  const bootDb = boot.get("db") === "bundled" ? "bundled" : "local";
  const bootChar = (boot.get("character") || "").trim();
  setDb(bootDb);
  if (bootChar) {
    selectCharacter(bootChar).catch((e) => appendLog(String(e.message || e), "err"));
  }
})();
