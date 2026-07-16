(() => {
  "use strict";

  let db = "local";
  let offset = 0;
  const limit = 50;
  let total = 0;
  let searchTimer = null;

  const $ = (id) => document.getElementById(id);

  function escapeHtml(s) {
    return String(s)
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;")
      .replace(/"/g, "&quot;");
  }

  function statusCell(status, idHint) {
    const label =
      status === "ready" ? "就绪" : status === "missing" ? "缺文件" : "未绑定";
    const tip = idHint ? `${label} · ${idHint}` : label;
    return (
      `<span class="probe-cell probe-${escapeHtml(status || "unbound")}" title="${escapeHtml(tip)}">` +
      `<span><span class="probe-dot" aria-hidden="true"></span>` +
      `<span class="probe-label">${escapeHtml(label)}</span></span>` +
      (idHint
        ? `<code class="probe-id">${escapeHtml(idHint)}</code>`
        : "") +
      `</span>`
    );
  }

  function linesCell(status, wikiSkin, count) {
    const map = {
      ready: "就绪",
      empty: "无台词",
      unmatched: "未匹配",
      stale_flat: "旧复制",
    };
    const label = map[status] || status || "无台词";
    const tip = [label, count != null ? `${count}条` : null, wikiSkin ? `Wiki:${wikiSkin}` : null]
      .filter(Boolean)
      .join(" · ");
    const cls =
      status === "ready"
        ? "probe-ready"
        : status === "unmatched" || status === "stale_flat"
          ? "probe-missing"
          : "probe-unbound";
    return (
      `<span class="probe-cell ${cls}" title="${escapeHtml(tip)}">` +
      `<span class="probe-dot" aria-hidden="true"></span>` +
      `<span class="probe-label">${escapeHtml(label)}</span></span>`
    );
  }

  function qs() {
    const params = new URLSearchParams(location.search);
    return params;
  }

  function syncUrl() {
    const u = new URL(location.href);
    u.searchParams.set("db", db);
    u.searchParams.set("offset", String(offset));
    const q = ($("sk-q").value || "").trim();
    const cid = ($("sk-character").value || "").trim();
    const filt = $("sk-filter").value || "";
    if (q) u.searchParams.set("q", q);
    else u.searchParams.delete("q");
    if (cid) u.searchParams.set("character_id", cid);
    else u.searchParams.delete("character_id");
    if (filt) u.searchParams.set("filter", filt);
    else u.searchParams.delete("filter");
    history.replaceState(null, "", u);
  }

  async function load() {
    syncUrl();
    const params = new URLSearchParams({
      db,
      offset: String(offset),
      limit: String(limit),
    });
    const q = ($("sk-q").value || "").trim();
    const cid = ($("sk-character").value || "").trim();
    const filt = $("sk-filter").value || "";
    if (q) params.set("q", q);
    if (cid) params.set("character_id", cid);
    if (filt) params.set("filter", filt);

    const res = await fetch(`/api/roster/skins?${params}`);
    const data = await res.json();
    if (!res.ok || data.ok === false) {
      $("sk-meta").textContent = data.error || `HTTP ${res.status}`;
      return;
    }
    total = data.total || 0;
    $("sk-meta").textContent =
      `共 ${total} 条 · 显示 ${offset + 1}–${Math.min(offset + limit, total) || 0}`;
    const tbody = $("sk-tbody");
    tbody.innerHTML = "";
    for (const sk of data.skins || []) {
      const tr = document.createElement("tr");
      const charLabel =
        escapeHtml(sk.character_name_zh || sk.character_id || "") +
        `<div class="id">${escapeHtml(sk.character_id || "")}</div>`;
      const rosterHref =
        `/roster?db=${encodeURIComponent(db)}&character=${encodeURIComponent(sk.character_id || "")}`;
      tr.innerHTML =
        `<td>${charLabel}</td>` +
        `<td class="skin-name"><div class="id">${escapeHtml(sk.id)}</div>` +
        `<div>${escapeHtml(sk.name_zh || "")}` +
        (sk.is_default ? '<span class="def-tag">默认</span>' : "") +
        `</div></td>` +
        `<td>${statusCell(sk.pet_status, sk.pet_model_id || "")}</td>` +
        `<td>${statusCell(sk.kanmusu_status, sk.kanmusu_dir || "")}</td>` +
        `<td>${linesCell(sk.lines_status, sk.lines_wiki_skin, sk.lines_count)}</td>` +
        `<td><a class="text-link" href="${rosterHref}">角色库</a></td>`;
      tbody.appendChild(tr);
    }
    $("sk-prev").disabled = offset <= 0;
    $("sk-next").disabled = offset + limit >= total;
    const page = Math.floor(offset / limit) + 1;
    const pages = Math.max(1, Math.ceil(total / limit));
    $("sk-page-label").textContent = `${page} / ${pages}`;
  }

  function setDb(next) {
    db = next;
    $("db-local").classList.toggle("active", db === "local");
    $("db-bundled").classList.toggle("active", db === "bundled");
    offset = 0;
    load();
  }

  function bootFromUrl() {
    const p = qs();
    if (p.get("db") === "bundled" || p.get("db") === "local") {
      db = p.get("db");
    }
    $("db-local").classList.toggle("active", db === "local");
    $("db-bundled").classList.toggle("active", db === "bundled");
    if (p.get("q")) $("sk-q").value = p.get("q");
    if (p.get("character_id")) $("sk-character").value = p.get("character_id");
    if (p.get("filter")) $("sk-filter").value = p.get("filter");
    const off = parseInt(p.get("offset") || "0", 10);
    offset = Number.isFinite(off) && off >= 0 ? off : 0;
  }

  document.addEventListener("DOMContentLoaded", () => {
    if (window.HanShell) HanShell.mount({ active: "skins" });
    bootFromUrl();
    $("db-local").addEventListener("click", () => setDb("local"));
    $("db-bundled").addEventListener("click", () => setDb("bundled"));
    $("sk-apply").addEventListener("click", () => {
      offset = 0;
      load();
    });
    $("sk-q").addEventListener("input", () => {
      clearTimeout(searchTimer);
      searchTimer = setTimeout(() => {
        offset = 0;
        load();
      }, 280);
    });
    $("sk-prev").addEventListener("click", () => {
      offset = Math.max(0, offset - limit);
      load();
    });
    $("sk-next").addEventListener("click", () => {
      offset += limit;
      load();
    });
    load();
  });
})();
