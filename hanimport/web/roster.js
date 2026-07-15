(() => {
  "use strict";

  let db = "local";
  let metaPath = "";
  let offset = 0;
  const limit = 50;
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
    const keys = ["ok", "filled", "error", "message", "counts", "allowlist", "bundled_db", "deleted"];
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
    $("btn-import-wiki").disabled = !localOnly;
    $("btn-sync-appdata").disabled = !localOnly;
    $("btn-publish").disabled = !localOnly;
    offset = 0;
    selectedCharId = null;
    selectedSkinId = null;
    selectedLineId = null;
    creatingChar = false;
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
    for (const ch of data.characters || []) {
      const li = document.createElement("li");
      li.dataset.id = ch.id;
      if (ch.id === selectedCharId) li.classList.add("selected");
      li.innerHTML =
        `<div class="id">${escapeHtml(ch.id)}</div>` +
        `<div>${escapeHtml(ch.name_zh || "")}` +
        ` · ${enDisplay(ch.name_en, ch.id)}</div>`;
      li.addEventListener("click", () => selectCharacter(ch.id));
      list.appendChild(li);
    }
    $("pager-label").textContent = total
      ? `${offset + 1}–${Math.min(offset + limit, total)} / ${total}`
      : "0";
    $("btn-prev").disabled = offset <= 0;
    $("btn-next").disabled = offset + limit >= total;
  }

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

  function renderSkins(skins) {
    const list = $("skin-list");
    list.innerHTML = "";
    $("skin-form").hidden = true;
    for (const sk of skins) {
      const li = document.createElement("li");
      li.dataset.id = sk.id;
      if (sk.id === selectedSkinId) li.classList.add("selected");
      const def = sk.is_default ? " · 默认" : "";
      li.innerHTML =
        `<div class="id">${escapeHtml(sk.id)}${def}</div>` +
        `<div>${escapeHtml(sk.name_zh || "")}` +
        ` · ${enDisplay(sk.name_en, sk.id)}</div>`;
      li.addEventListener("click", () => selectSkin(sk));
      list.appendChild(li);
    }
  }

  function selectSkin(sk) {
    creatingSkin = false;
    selectedSkinId = sk.id;
    selectedLineId = null;
    creatingLine = false;
    document.querySelectorAll("#skin-list li").forEach((el) => {
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
    document.querySelectorAll("#skin-list li.selected").forEach((el) => el.classList.remove("selected"));
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

    $("btn-import-wiki").addEventListener("click", () => {
      if (db !== "local") {
        appendLog("导入 Wiki 仅支持自用库，请切回自用库", "err");
        return;
      }
      runOp("导入 Wiki", "/api/roster/ops/import-wiki", {});
    });
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
  }

  if (window.HanShell) HanShell.mount({ active: "roster" });

  bind();
  setDb("local");
})();
