const state = {
  devices: [],
  selected: null,
  history: [],
  native: false,
  trusted: false,
  trustPollTimer: null,
  knownPushIds: new Set(),
  receiving: false,
  receivingPushId: null,
  pendingReceiveItems: [],
  autoReceive: true,
  nativeTrustPoll: null,
  downloadStartedIds: new Set(),
  browsePath: "",
  browseSelected: new Set(),
  browseLoading: false,
  activeTab: "devices",
  azSelected: new Set(),
  azFiles: [],
  azLoading: false,
  azSending: false,
  trustHint: "",
  updateCheckTimer: null,
  upload: {
    active: false,
    paused: false,
    xhr: null,
    uiKey: null,
    pickUiKey: null,
  },
};

const $ = (id) => document.getElementById(id);

function isNativeApp() {
  return typeof window.Hantransfer !== "undefined";
}

let nativeStartupReadyDone = false;
let receiveRefreshInFlight = false;
let receiveRefreshQueued = false;
let lastEnsureTrustAt = 0;
let lastDevicesJson = "";

window.__markNativeApp = function () {
  if (!isNativeApp()) return;
  if (!state.native) {
    state.native = true;
    initMode();
  } else {
    refreshAppInfo();
  }
  if (!nativeStartupReadyDone) {
    nativeStartupReadyDone = true;
    try { window.onStartupReady?.(); } catch (_) {}
  }
};

function refreshAppInfo() {
  // 标题栏仅保留 hantransfer，不显示版本/模式小字
}

function esc(s) {
  return String(s).replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c]));
}

function setStatusBanner(el, text, type = "info") {
  if (!el) return;
  el.className = `status-banner ${type}`;
  el.textContent = text;
}

const toastTimers = new Map();

function ensureToastStack() {
  let stack = $("toast-stack");
  if (!stack) {
    stack = document.createElement("div");
    stack.id = "toast-stack";
    stack.className = "toast-stack";
    stack.setAttribute("aria-live", "polite");
    document.body.appendChild(stack);
  }
  return stack;
}

function toastIconHtml(type) {
  if (type === "loading") return '<div class="toast-spinner"></div>';
  const icons = { info: "i", success: "✓", warn: "!", error: "×" };
  return esc(icons[type] || "i");
}

function clearToastTimer(id) {
  const timer = toastTimers.get(id);
  if (timer) {
    clearTimeout(timer);
    toastTimers.delete(id);
  }
}

function scheduleToastDismiss(el, id, duration) {
  clearToastTimer(id);
  toastTimers.set(id, setTimeout(() => dismissToast(id), duration));
}

function dismissToast(id) {
  if (!id) return;
  clearToastTimer(id);
  const stack = $("toast-stack");
  const el = stack?.querySelector(`[data-toast-id="${id}"]`);
  if (!el) return;
  el.classList.add("leaving");
  setTimeout(() => el.remove(), 240);
}

/** Drop named toasts (esp. sticky loading with duration 0 / no close). */
function dismissStickyToasts(...ids) {
  ids.forEach((id) => dismissToast(id));
}

function bindToastDismiss(el, id, type) {
  const close = (e) => {
    e?.preventDefault?.();
    e?.stopPropagation?.();
    dismissToast(id);
  };
  const closeBtn = el.querySelector(".toast-close");
  if (closeBtn) {
    closeBtn.addEventListener("click", close);
    closeBtn.addEventListener("pointerup", close);
  }
  // Non-loading toasts: tap body to dismiss (WebView click can be flaky).
  if (type !== "loading") {
    el.addEventListener("click", (e) => {
      if (e.target.closest(".toast-close")) return;
      dismissToast(id);
    });
  }
}

function armLoadingToastWatchdog(id, message) {
  clearToastTimer(id);
  // Sticky loading with no close can freeze UX if a native callback is dropped.
  toastTimers.set(id, setTimeout(() => {
    showToast(`${message}（超时，可关闭）`, "warn", { id, duration: 10000 });
  }, 45000));
}

function showToast(message, type = "info", opts = {}) {
  const stack = ensureToastStack();
  const id = opts.id || `toast-${Date.now()}-${Math.random().toString(36).slice(2, 5)}`;
  const duration = opts.duration ?? (type === "loading" ? 0 : type === "error" ? 4200 : 3000);
  let el = opts.id ? stack.querySelector(`[data-toast-id="${opts.id}"]`) : null;

  if (el) {
    el.className = `toast toast-${type}`;
    el.classList.remove("leaving");
    const msgNode = el.querySelector(".toast-msg");
    if (msgNode) msgNode.textContent = message;
    const iconNode = el.querySelector(".toast-icon");
    if (iconNode) iconNode.innerHTML = toastIconHtml(type);
    const closeBtn = el.querySelector(".toast-close");
    if (type === "loading") closeBtn?.remove();
    else if (!closeBtn) {
      const btn = document.createElement("button");
      btn.type = "button";
      btn.className = "toast-close";
      btn.setAttribute("aria-label", "关闭");
      btn.textContent = "×";
      el.appendChild(btn);
      bindToastDismiss(el, id, type);
    }
    if (duration > 0) scheduleToastDismiss(el, id, duration);
    else if (type === "loading") armLoadingToastWatchdog(id, message);
    else clearToastTimer(id);
    return id;
  }

  el = document.createElement("div");
  el.className = `toast toast-${type}`;
  el.dataset.toastId = id;
  el.innerHTML = `
    <div class="toast-icon">${toastIconHtml(type)}</div>
    <span class="toast-msg"></span>
    ${type !== "loading" ? '<button type="button" class="toast-close" aria-label="关闭">×</button>' : ""}
  `;
  el.querySelector(".toast-msg").textContent = message;
  bindToastDismiss(el, id, type);
  stack.appendChild(el);
  while (stack.children.length > 3) {
    const oldest = stack.firstElementChild;
    if (oldest?.dataset.toastId) dismissToast(oldest.dataset.toastId);
    else oldest?.remove();
  }
  if (duration > 0) scheduleToastDismiss(el, id, duration);
  else if (type === "loading") armLoadingToastWatchdog(id, message);
  return id;
}

function toastInfo(msg, id) { return showToast(msg, "info", id ? { id } : {}); }
function toastOk(msg, id) { return showToast(msg, "success", id ? { id } : {}); }
function toastWarn(msg, id) { return showToast(msg, "warn", id ? { id } : {}); }
function toastErr(msg, id) { return showToast(msg, "error", id ? { id } : {}); }
function toastLoading(msg, id) { return showToast(msg, "loading", { id: id || "loading", duration: 0 }); }

function finishToast(id, message, type = "success") {
  if (id) showToast(message, type, { id });
  else showToast(message, type);
}

function initTapEffects() {
  const SEL = "button, .browse-item, .device, .file-drop, .tabs button, .native-top-nav button, .native-shortcuts button, .az-select-all";
  const clear = () => document.querySelectorAll(".tap-active").forEach((el) => el.classList.remove("tap-active"));
  document.addEventListener("pointerdown", (e) => {
    const el = e.target.closest(SEL);
    if (!el || el.disabled) return;
    clear();
    el.classList.add("tap-active");
  }, { passive: true });
  document.addEventListener("pointerup", clear, { passive: true });
  document.addEventListener("pointercancel", clear, { passive: true });
  document.addEventListener("blur", clear, true);
}

function fileExt(name) {
  const i = String(name).lastIndexOf(".");
  return i > 0 ? String(name).slice(i + 1, i + 5) : "file";
}

function formatTimeAgo(ts) {
  const diff = Date.now() - Number(ts);
  if (!Number.isFinite(diff) || diff < 0) return "";
  if (diff < 60000) return "刚刚";
  if (diff < 3600000) return `${Math.floor(diff / 60000)} 分钟前`;
  if (diff < 86400000) return `${Math.floor(diff / 3600000)} 小时前`;
  return new Date(ts).toLocaleDateString("zh-CN");
}

function historyBadge(type) {
  if (type === "push") return '<span class="badge badge-in">接收</span>';
  if (type === "azurlane_asset") return '<span class="badge badge-az">碧蓝</span>';
  return '<span class="badge badge-out">发送</span>';
}

function setProgressVisible(wrapId, pctId, visible) {
  const wrap = $(wrapId);
  if (wrap) wrap.classList.toggle("hidden", !visible);
  if (!visible && pctId) {
    const pct = $(pctId);
    if (pct) pct.textContent = "";
  }
}

function updateProgressBar(progEl, wrapId, pctId, value) {
  const v = Math.max(0, Math.min(100, Math.round(value)));
  if (progEl) progEl.value = v;
  const pct = pctId ? $(pctId) : null;
  if (pct) pct.textContent = `${v}%`;
  if (wrapId) setProgressVisible(wrapId, null, true);
}

const UPLOAD_UI = {
  send: {
    wrapId: "upload-progress-wrap",
    batchProg: "upload-batch-progress",
    batchPct: "upload-batch-pct",
    fileProg: "upload-progress",
    filePct: "upload-progress-pct",
    fileLabel: "upload-file-label",
    pauseBtn: "btn-upload-pause",
    msgEl: "send-msg",
  },
  quick: {
    wrapId: "quick-progress-wrap",
    batchProg: "quick-batch-progress",
    batchPct: "quick-batch-pct",
    fileProg: "quick-upload-progress",
    filePct: "quick-progress-pct",
    fileLabel: "quick-file-label",
    pauseBtn: "btn-quick-upload-pause",
    msgEl: "quick-send-msg",
  },
  browse: {
    wrapId: "upload-progress-wrap",
    batchProg: "upload-batch-progress",
    batchPct: "upload-batch-pct",
    fileProg: "upload-progress",
    filePct: "upload-progress-pct",
    fileLabel: "upload-file-label",
    pauseBtn: "btn-upload-pause",
    msgEl: "browse-msg",
  },
  az: {
    wrapId: "az-progress-wrap",
    batchProg: "az-batch-progress",
    batchPct: "az-batch-pct",
    fileProg: "az-progress",
    filePct: "az-progress-pct",
    fileLabel: "az-file-label",
    pauseBtn: "btn-az-upload-pause",
    msgEl: "az-status",
  },
};

function resolveUploadUi(key) {
  const cfg = UPLOAD_UI[key] || UPLOAD_UI.send;
  return {
    key,
    wrapId: cfg.wrapId,
    batchProg: $(cfg.batchProg),
    batchPct: cfg.batchPct,
    fileProg: $(cfg.fileProg),
    filePct: cfg.filePct,
    fileLabel: $(cfg.fileLabel),
    pauseBtn: $(cfg.pauseBtn),
    msgEl: $(cfg.msgEl),
  };
}

function activeUploadUiKey() {
  if (state.upload.uiKey) return state.upload.uiKey;
  if (state.upload.pickUiKey) return state.upload.pickUiKey;
  if (state.activeTab === "az" || state.azSending) return "az";
  if (state.activeTab === "devices") return "quick";
  if (state.activeTab === "send" && state.browseSelected?.size > 0) return "browse";
  return "send";
}

function uploadCompletedCount(p) {
  if (!p) return 0;
  if (p.index > 0) return p.index;
  if (p.done && !p.error && !p.paused) return p.batch_total || 1;
  return 0;
}

function uploadDoneMessage(p) {
  const total = p?.batch_total || 1;
  const done = uploadCompletedCount(p);
  if (p?.paused) {
    return p.error || `已暂停，已完成 ${done}/${total} 个文件`;
  }
  if (p?.error) return p.error;
  // Native/batch summary already includes skip/fail counts.
  if (p?.name && /跳过|已发送|失败|重复/.test(String(p.name))) return p.name;
  if (total > 1) return `已发送 ${done} 个文件`;
  return `已发送 ${p?.name || "文件"}`;
}

function resetUploadProgressBars(ui) {
  updateProgressBar(ui.batchProg, ui.wrapId, ui.batchPct, 0);
  updateProgressBar(ui.fileProg, ui.wrapId, ui.filePct, 0);
  if (ui.fileLabel) ui.fileLabel.textContent = "当前文件";
}

function batchOverallPercent(payload) {
  const idx = payload?.index || 1;
  const count = payload?.batch_total || 1;
  const fileRatio = payload?.total > 0 ? payload.sent / payload.total : 0;
  if (count <= 1) return fileRatio * 100;
  return ((idx - 1) + fileRatio) / count * 100;
}

function filePercent(payload) {
  return payload?.total > 0 ? (payload.sent / payload.total) * 100 : 0;
}

function beginUploadSession(uiKey) {
  state.upload.active = true;
  state.upload.paused = false;
  state.upload.xhr = null;
  state.upload.uiKey = uiKey;
  state.upload.pickUiKey = null;
  const ui = resolveUploadUi(uiKey);
  resetUploadProgressBars(ui);
  ui.pauseBtn?.classList.remove("hidden");
  if (ui.pauseBtn) {
    ui.pauseBtn.disabled = false;
    ui.pauseBtn.textContent = "暂停发送";
  }
  setProgressVisible(ui.wrapId, null, true);
}

function endUploadSession() {
  state.upload.active = false;
  state.upload.paused = false;
  state.upload.xhr = null;
  state.upload.uiKey = null;
  state.upload.pickUiKey = null;
  Object.values(UPLOAD_UI).forEach((cfg) => {
    const btn = $(cfg.pauseBtn);
    btn?.classList.add("hidden");
    if (btn) {
      btn.disabled = false;
      btn.textContent = "暂停发送";
    }
  });
}

function pauseUpload() {
  if (!state.upload.active || state.upload.paused) return;
  state.upload.paused = true;
  const ui = resolveUploadUi(state.upload.uiKey || activeUploadUiKey());
  if (ui.pauseBtn) {
    ui.pauseBtn.disabled = true;
    ui.pauseBtn.textContent = "暂停中…";
  }
  try { state.upload.xhr?.abort(); } catch (_) {}
  if (state.native) {
    try { window.Hantransfer.pauseSendAsync?.(); } catch (_) {}
  }
}

function applyUploadProgress(payload, uiKey) {
  const ui = resolveUploadUi(uiKey || activeUploadUiKey());
  const batchPct = batchOverallPercent(payload);
  const filePct = filePercent(payload);
  const prefix = payload?.batch_total > 1 && payload?.index
    ? `(${payload.index}/${payload.batch_total}) `
    : "";
  const fileName = payload?.name || "文件";

  updateProgressBar(ui.batchProg, ui.wrapId, ui.batchPct, batchPct);
  updateProgressBar(ui.fileProg, ui.wrapId, ui.filePct, filePct);
  if (ui.fileLabel) {
    ui.fileLabel.textContent = payload?.done
      ? (payload?.error || payload?.paused ? "已停止" : "完成")
      : `${prefix}${fileName}`;
  }

  if (!ui.msgEl) return;
  if (payload?.done) {
    ui.pauseBtn?.classList.add("hidden");
    if (payload.error || payload.paused) {
      ui.msgEl.textContent = payload.error || "已暂停";
      setProgressVisible(ui.wrapId, null, false);
    } else {
      const doneMsg = uploadDoneMessage(payload);
      ui.msgEl.textContent = doneMsg;
      setProgressVisible(ui.wrapId, null, false);
    }
    return;
  }
  if (payload?.total > 0) {
    ui.msgEl.textContent = payload.batch_total > 1
      ? `上传 ${payload.index}/${payload.batch_total}: ${fileName}`
      : `上传中: ${fileName}`;
  }
}

function sameDevice(a, b) {
  if (!a || !b) return false;
  if (a.id && b.id && a.id === b.id) return true;
  return a.host === b.host && Number(a.port) === Number(b.port);
}

function deviceFromSaved(raw) {
  if (!raw?.host) return null;
  return {
    id: raw.id || raw.host,
    name: raw.name || raw.host,
    platform: raw.platform || "windows",
    host: raw.host,
    port: Number(raw.port) || 7822,
  };
}

function mergeSavedDeviceIntoList() {
  if (!state.native || !state.selected) return;
  if (!state.devices.some((d) => sameDevice(d, state.selected))) {
    state.devices = [state.selected, ...state.devices];
  }
}

function formatHttpHost(host) {
  const h = String(host || "");
  if (h.includes(":") && !h.startsWith("[")) return `[${h}]`;
  return h;
}

function isBadLanHost(host) {
  if (!host) return false;
  if (/^192\.168\.|^10\.|^172\.(1[6-9]|2\d|3[0-1])\./.test(host)) return false;
  if (/^fe80:/i.test(host)) return false;
  return host.includes(":");
}

function tab(name) {
  state.activeTab = name;
  document.querySelectorAll("[data-tab]").forEach((b) => {
    b.classList.toggle("active", b.dataset.tab === name);
  });
  document.querySelectorAll(".panel").forEach((p) => {
    p.classList.toggle("active", p.id === `tab-${name}`);
  });
  if (name === "receive") refreshReceiveQueue().catch(() => {});
  if (name === "az" || name === "send") {
    syncNativeDevice();
    if (state.selected && !state.trusted) ensureNativeTrust();
  }
  if (name === "az") loadAzLive2d();
  if (name === "settings") {
    syncNativeDevice();
    refreshSettings();
  }
}

function updateReceiveBadge(count) {
  const btn = document.querySelector('.tabs button[data-tab="receive"]');
  if (!btn) return;
  let badge = btn.querySelector(".tab-badge");
  if (count > 0) {
    if (!badge) {
      badge = document.createElement("span");
      badge.className = "tab-badge";
      btn.appendChild(badge);
    }
    badge.textContent = String(count);
  } else if (badge) {
    badge.remove();
  }
}

document.querySelectorAll("[data-tab]").forEach((btn) => {
  btn.addEventListener("click", () => tab(btn.dataset.tab));
});

function renderDevices() {
  const list = $("device-list");
  list.innerHTML = "";
  if (state.devices.length === 0) {
    list.innerHTML = `<li class="empty-state"><div class="empty-icon">📡</div>未发现设备<br><span class="hint">请确认与电脑在同一 WiFi，PC 已运行 hantransfer</span><br><button type="button" class="outline" style="margin-top:0.75rem" onclick="document.getElementById('btn-rescan')?.click()">重新扫描</button></li>`;
    return;
  }
  state.devices.forEach((d) => {
    const li = document.createElement("li");
    li.className = "device" + (sameDevice(state.selected, d) ? " selected" : "");
    li.innerHTML = `
      <div class="device-icon">💻</div>
      <div class="device-body">
        <strong>${esc(d.name)}</strong>
        <span class="hint">${esc(d.platform)} · ${esc(d.host)}:${d.port}${sameDevice(state.selected, d) ? (state.trusted ? " · 已信任" : " · 连接中…") : ""}</span>
      </div>
      <div class="device-dot"></div>`;
    li.onclick = () => selectDevice(d);
    list.appendChild(li);
  });
}

function selectDevice(d, opts = {}) {
  const ensureTrust = opts.ensureTrust !== false;
  const wasSelected = sameDevice(state.selected, d);
  state.selected = d;
  $("selected-name").textContent = `${d.name} (${d.host})`;
  renderDevices();
  updateQuickSendVisibility();
  updateAzDeviceHint();
  if (!wasSelected && opts.toastSelect !== false) toastInfo(`已选择 ${d.name}`);
  if (state.native) {
    try {
      window.Hantransfer.setSelectedDevice(d.host, Number(d.port) || 7822, d.id || null);
    } catch (_) {}
    // Already trusted for this device → skip; pending autoConnect will handle first connect.
    if (ensureTrust && !(wasSelected && state.trusted)) {
      ensureNativeTrust();
    }
  }
  localStorage.setItem("hantransfer-device", JSON.stringify(d));
}

function clearNativeTrustPoll() {
  if (state.nativeTrustPoll) {
    clearInterval(state.nativeTrustPoll);
    state.nativeTrustPoll = null;
  }
}

function ensureNativeTrust() {
  if (!state.native || !state.selected || state.trusted) return;
  const now = Date.now();
  if (now - lastEnsureTrustAt < 2000) return;
  lastEnsureTrustAt = now;
  try { window.Hantransfer.ensureTrustedAsync?.(); } catch (_) {}
}

/** Apply trusted state once; only toast when newly connected. */
function notifyConnected(opts = {}) {
  const wasTrusted = state.trusted;
  clearNativeTrustPoll();
  // Always clear sticky "正在连接…" loading toast — trust may arrive via
  // ensureTrustedAsync without onNativeConnectResult, leaving it undismissable.
  dismissStickyToasts("connect");
  state.trustHint = "";
  setTrusted(true);
  setStatusBanner($("scan-status"), opts.message || "已连接并已信任", "ok");
  setStatusBanner($("receive-status"), "已连接，可接收电脑推送", "ok");
  if ($("send-msg") && !$("send-msg").textContent) {
    $("send-msg").textContent = "已连接，请选择文件后发送";
  }
  if (!wasTrusted && opts.toast !== false) {
    toastOk("已连接电脑，可以传输文件", "connected");
    refreshReceiveQueue().catch(() => {});
  }
}

window.onNativeTrustPayload = function (payload) {
  onNativeTrustStatus(payload?.status, payload?.message);
};

window.onNativeTrustStatus = function (status, message) {
  if (status === "trusted") {
    notifyConnected({ toast: true });
    return;
  }
  setTrusted(false);
  if (status === "pending") {
    const msg = message || "等待电脑确认信任…（请在 PC 打开 http://电脑IP:7822/ 点允许）";
    state.trustHint = msg;
    setStatusBanner($("scan-status"), msg, "warn");
    setStatusBanner($("receive-status"), msg, "warn");
    updateAzDeviceHint();
    if (!state.nativeTrustPoll) {
      state.nativeTrustPoll = setInterval(ensureNativeTrust, 2500);
    }
    return;
  }
  clearNativeTrustPoll();
  state.trustHint = message
    || (status === "rejected"
      ? "电脑已拒绝此设备，请在 PC 管理页「已拒绝设备」中解除"
      : "连接失败");
  setStatusBanner($("scan-status"), state.trustHint, "err");
  setStatusBanner($("receive-status"), state.trustHint, "err");
  updateAzDeviceHint();
};

function updateQuickSendVisibility() {
  const quick = $("quick-send");
  if (!quick) return;
  quick.classList.toggle("hidden", !(state.trusted && state.selected));
}

function clearTrustPoll() {
  if (state.trustPollTimer) {
    clearInterval(state.trustPollTimer);
    state.trustPollTimer = null;
  }
}

function setTrusted(trusted) {
  state.trusted = trusted;
  updateQuickSendVisibility();
  updateDeviceActions();
  updateAzDeviceHint();
  updateAzSendButtons();
  updateSettingsConnectionUi();
}

function updateDeviceActions() {
  const el = $("device-actions");
  if (!el) return;
  el.classList.toggle("hidden", !(state.selected && state.trusted));
}

function onConnectedTrusted() {
  notifyConnected({ toast: false });
}

async function fetchHandshakeStatus(base) {
  const deviceId = await phoneDeviceId();
  const hs = await fetch(`${base}/api/v1/handshake`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      device_id: deviceId,
      name: "HAN-PHONE-WEB",
      platform: "android",
      version: "0.1.0",
    }),
  });
  const body = await hs.json().catch(() => ({}));
  let status;
  try {
    status = parseApiBody(body).status;
  } catch (_) {
    status = body?.data?.status;
  }
  return { hs, body, status };
}

function pollTrustUntilReady(base) {
  clearTrustPoll();
  let tries = 0;
  state.trustPollTimer = setInterval(async () => {
    tries += 1;
    if (tries > 90) {
      clearTrustPoll();
      setTrusted(false);
      const msg = "等待信任超时（约 3 分钟）。请在 PC 打开 http://电脑IP:7822/ 点「允许」，或重新连接";
      setStatusBanner($("scan-status"), msg, "err");
      toastWarn(msg);
      return;
    }
    try {
      const { hs, status } = await fetchHandshakeStatus(base);
      if (hs.status === 403 || status === "rejected") {
        clearTrustPoll();
        setTrusted(false);
        setStatusBanner(
          $("scan-status"),
          "电脑已拒绝此设备，请在 PC 管理页「已拒绝设备」中点「解除拒绝」后重试",
          "err",
        );
        return;
      }
      if (hs.status === 200 || status === "trusted") {
        clearTrustPoll();
        setStatusBanner($("scan-status"), "已连接并已信任", "ok");
        onConnectedTrusted();
      }
    } catch (_) {}
  }, 2000);
}

function loadHistory() {
  if (state.native) {
    try {
      state.history = JSON.parse(window.Hantransfer.getHistoryJson() || "[]");
    } catch (_) {
      state.history = [];
    }
  } else {
    state.history = JSON.parse(localStorage.getItem("hantransfer-history") || "[]");
  }
  const list = $("history-list");
  list.innerHTML = "";
  if (state.history.length === 0) {
    list.innerHTML = `<li class="empty-state"><div class="empty-icon">📋</div>暂无传输记录</li>`;
    return;
  }
  state.history.forEach((h) => {
    const li = document.createElement("li");
    li.className = "file-row";
    const when = formatTimeAgo(h.at);
    li.innerHTML = `
      <div class="file-icon">${esc(fileExt(h.filename))}</div>
      <div class="file-body">
        <strong>${esc(h.filename)}</strong>
        <span class="hint">${historyBadge(h.type)}${esc(h.deviceName)}${when ? ` · ${when}` : ""}</span>
      </div>`;
    list.appendChild(li);
  });
}

function pushHistory(item) {
  state.history.unshift(item);
  state.history = state.history.slice(0, 100);
  if (state.native) {
    try { window.Hantransfer.appendHistory(JSON.stringify(item)); } catch (_) {}
  } else {
    localStorage.setItem("hantransfer-history", JSON.stringify(state.history));
  }
  loadHistory();
}

function parseDevices(data) {
  if (typeof data === "string") return JSON.parse(data);
  return data;
}

window.onDevicesUpdated = function (data) {
  try {
    state.devices = parseDevices(data);
    if (state.native && state.selected && !state.devices.some((d) => sameDevice(d, state.selected))) {
      state.devices.unshift(state.selected);
    }
    if (state.native && !state.selected && state.devices.length > 0) {
      selectDevice(state.devices[0]);
    }
    renderDevices();
    const badIp = state.devices.some((d) => isBadLanHost(d.host));
    if (state.native && badIp) {
      setStatusBanner($("scan-status"), "发现设备但 IP 异常，请点「手动输入 IP」或重启 PC 端后重新扫描", "warn");
      $("btn-manual-ip")?.classList.remove("hidden");
      return;
    }
    $("btn-manual-ip")?.classList.toggle("hidden", state.devices.length > 0);
    if (!(state.trusted && state.selected)) {
      setStatusBanner($("scan-status"), `已发现 ${state.devices.length} 台设备`, state.devices.length ? "ok" : "info");
    }
    // Trust poll owns handshake retries — avoid stacking on every mDNS refresh.
    if (state.native && state.selected && !state.trusted && !state.nativeTrustPoll) {
      ensureNativeTrust();
    }
  } catch (_) {
    setStatusBanner($("scan-status"), "设备列表解析失败", "err");
  }
};

window.onUploadProgressPayload = function (payload) {
  let p = payload;
  if (typeof p === "string") {
    try { p = JSON.parse(p); } catch (_) { return; }
  }
  const isAz = state.activeTab === "az" || state.azSending;
  const uiKey = isAz ? "az" : activeUploadUiKey();
  if (!state.upload.active && !p?.done) beginUploadSession(uiKey);
  applyUploadProgress(p, uiKey);

  if (isAz) {
    if (p?.done) {
      state.azSending = false;
      updateAzSendButtons();
      const doneMsg = uploadDoneMessage(p);
      if (p.error || p.paused) {
        setStatusBanner($("az-status"), doneMsg, p.paused ? "warn" : "err");
        finishToast("az-send", doneMsg, p.paused ? "warn" : "error");
      } else {
        setStatusBanner($("az-status"), doneMsg, "ok");
        finishToast("az-send", doneMsg, "success");
      }
      endUploadSession();
    } else if (p?.total > 0) {
      state.azSending = true;
      updateAzSendButtons();
      toastLoading(`正在发送 ${p.name || "文件"}…`, "az-send");
    }
    return;
  }

  if (p?.done) {
    const doneMsg = uploadDoneMessage(p);
    endUploadSession();
    if (p.error || p.paused) {
      finishToast("upload", doneMsg, p.paused ? "warn" : "error");
    } else {
      finishToast("upload", doneMsg, "success");
    }
  } else if (p?.total > 0) {
    toastLoading(`正在发送 ${p.name || "文件"}…`, "upload");
  }
};

window.onUploadProgress = function (filename, sent, total, done, error) {
  const prog = $("upload-progress");
  prog.classList.remove("hidden");
  if (total > 0) prog.value = Math.round((sent / total) * 100);
  if (done) {
    $("send-msg").textContent = error ? `失败: ${error}` : `已发送: ${filename}`;
    if (!error) prog.classList.add("hidden");
  }
};

window.onAzStatusPayload = function (payload) {
  const el = $("az-status");
  if (!el) return;
  const isErr = !!payload?.error;
  const msg = payload?.message || "";
  setStatusBanner(el, msg, isErr ? "err" : "ok");
  state.azSending = false;
  updateAzSendButtons();
  endUploadSession();
  setProgressVisible("az-progress-wrap", null, false);
  if (msg) {
    finishToast("az-send", msg, isErr ? "error" : "success");
  } else {
    dismissToast("az-send");
  }
};

function deviceBase() {
  const d = state.selected;
  return d ? `http://${formatHttpHost(d.host)}:${d.port}` : null;
}

function formatSize(bytes) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

async function fetchPushPendingBrowser() {
  const base = deviceBase();
  if (!base || !state.trusted) return [];
  const deviceId = await phoneDeviceId();
  const resp = await fetch(`${base}/api/v1/push/pending`, {
    headers: { "X-Hantransfer-Device-ID": deviceId },
  });
  const body = await resp.json();
  return parseApiBody(body);
}

function renderReceiveList(items) {
  state.pendingReceiveItems = items;
  const list = $("receive-list");
  if (!list) return;
  list.innerHTML = "";
  if (!items.length) {
    list.innerHTML = `<li class="empty-state"><div class="empty-icon">📥</div>暂无待接收文件<br><span class="hint">电脑推送后会显示在这里</span></li>`;
    return;
  }
  items.forEach((item) => {
    const li = document.createElement("li");
    const isActive = state.receiving && state.receivingPushId === item.push_id;
    li.className = "file-row receive-item" + (isActive ? " receiving" : "");
    li.innerHTML = `
      <div class="file-icon">${esc(fileExt(item.filename))}</div>
      <div class="file-body">
        <strong>${esc(item.filename)}</strong>
        <span class="hint">${esc(item.source || "电脑")} · ${formatSize(item.size || 0)}</span>
      </div>`;
    const started = state.downloadStartedIds.has(item.push_id);
    const btn = document.createElement("button");
    btn.type = "button";
    btn.textContent = isActive ? "…" : (started ? "确认已接收" : "下载");
    btn.disabled = state.receiving && !started;
    btn.className = started ? "outline" : "";
    btn.onclick = () => (started ? ackPushItem(item) : downloadPushItem(item));
    li.appendChild(btn);
    list.appendChild(li);
  });
  const allBtn = $("btn-receive-all");
  const refreshBtn = $("btn-receive-refresh");
  if (allBtn) allBtn.disabled = state.receiving || !items.length;
  if (refreshBtn) refreshBtn.disabled = state.receiving;
}

function pushFileUrl(base, pushId, deviceId) {
  return `${base}/api/v1/push/${encodeURIComponent(pushId)}/file?device_id=${encodeURIComponent(deviceId)}`;
}

async function verifyPushAvailable(base, pushId, deviceId) {
  const url = pushFileUrl(base, pushId, deviceId);
  const resp = await fetch(url, { method: "HEAD" });
  if (resp.status === 404) {
    throw new Error("文件不存在或已被接收，请让电脑重新推送");
  }
  if (!resp.ok) {
    const hint = resp.status === 401 ? "未信任，请在电脑管理页允许连接"
      : resp.status === 403 ? "该文件不是发给本机的"
      : `HTTP ${resp.status}`;
    throw new Error(`无法下载：${hint}`);
  }
  return url;
}

function triggerBrowserFileDownload(fileUrl) {
  const mobile = /Android|iPhone|iPad|iPod/i.test(navigator.userAgent || "");
  if (mobile) {
    window.open(fileUrl, "_blank", "noopener");
    return;
  }
  const a = document.createElement("a");
  a.href = fileUrl;
  a.rel = "noopener";
  document.body.appendChild(a);
  a.click();
  a.remove();
}

async function ackPushItem(item) {
  const base = deviceBase();
  if (!base) return;
  const msgEl = $("receive-msg");
  const deviceId = await phoneDeviceId();
  try {
    const resp = await fetch(`${base}/api/v1/push/${item.push_id}/ack?device_id=${encodeURIComponent(deviceId)}`, {
      method: "POST",
      headers: { "X-Hantransfer-Device-ID": deviceId },
    });
    if (!resp.ok) {
      const body = await resp.json().catch(() => ({}));
      throw new Error(body.error?.message || `确认失败 (${resp.status})`);
    }
    state.downloadStartedIds.delete(item.push_id);
    state.knownPushIds.add(item.push_id);
    pushHistory({
      filename: item.filename,
      deviceName: item.source || state.selected?.name || "PC",
      path: "",
      at: Date.now(),
      type: "push",
    });
    msgEl.textContent = `已确认接收: ${item.filename}`;
    await refreshReceiveQueue();
    loadHistory();
  } catch (e) {
    msgEl.textContent = `${item.filename}: ${e.message || e}`;
  }
}

async function downloadPushItem(item, opts = {}) {
  const { batch = false, index = 1, total = 1, onProgress } = opts;
  const base = deviceBase();
  if (!base) return;
  const msgEl = $("receive-msg");
  const progEl = $("receive-progress");
  const deviceId = await phoneDeviceId();
  if (!batch) {
    state.receiving = true;
    state.receivingPushId = item.push_id;
    renderReceiveList(state.pendingReceiveItems);
  }
  msgEl.textContent = total > 1 ? `下载 ${index}/${total}: ${item.filename}` : `下载中: ${item.filename}`;
  updateProgressBar(progEl, "receive-progress-wrap", "receive-progress-pct", 0);
  try {
    if (state.native) {
      window.Hantransfer.downloadPush(item.push_id);
      return;
    }
    const fileUrl = await verifyPushAvailable(base, item.push_id, deviceId);
    triggerBrowserFileDownload(fileUrl);
    // Browser cannot detect download completion — auto-ack so PC outbox clears.
    try {
      const resp = await fetch(`${base}/api/v1/push/${item.push_id}/ack?device_id=${encodeURIComponent(deviceId)}`, {
        method: "POST",
        headers: { "X-Hantransfer-Device-ID": deviceId },
      });
      if (!resp.ok) {
        const body = await resp.json().catch(() => ({}));
        throw new Error(body.error?.message || `确认失败 (${resp.status})`);
      }
      state.knownPushIds.add(item.push_id);
      state.downloadStartedIds.delete(item.push_id);
      pushHistory({
        filename: item.filename,
        deviceName: item.source || state.selected?.name || "PC",
        path: "",
        at: Date.now(),
        type: "push",
      });
      msgEl.textContent = total > 1
        ? `已接收 ${index}/${total}: ${item.filename}`
        : `已接收: ${item.filename}`;
    } catch (_) {
      // Download already started — keep confirm button as fallback.
      state.downloadStartedIds.add(item.push_id);
      msgEl.textContent = `已开始下载: ${item.filename}（请点「确认已接收」）`;
    }
    renderReceiveList(state.pendingReceiveItems);
  } catch (e) {
    msgEl.textContent = `${item.filename}: ${e.message || e}`;
    throw e;
  } finally {
    if (!batch && !state.native) {
      state.receiving = false;
      state.receivingPushId = null;
      setProgressVisible("receive-progress-wrap", "receive-progress-pct", false);
      renderReceiveList(state.pendingReceiveItems);
    }
  }
}

async function downloadPushBatchBrowser(items) {
  if (!items.length || state.receiving) return;
  state.receiving = true;
  let ok = 0;
  let needAck = 0;
  try {
    for (let i = 0; i < items.length; i++) {
      state.receivingPushId = items[i].push_id;
      renderReceiveList(items);
      try {
        await downloadPushItem(items[i], {
          batch: true,
          index: i + 1,
          total: items.length,
        });
        if (state.downloadStartedIds.has(items[i].push_id)) needAck++;
        else ok++;
        if (i < items.length - 1) await new Promise((r) => setTimeout(r, 800));
      } catch (_) {
        /* keep going */
      }
    }
    if (ok && !needAck) {
      $("receive-msg").textContent = `已自动接收 ${ok} 个文件`;
    } else if (ok || needAck) {
      $("receive-msg").textContent = needAck
        ? `已下载 ${ok + needAck} 个，其中 ${needAck} 个请点「确认已接收」`
        : `已自动接收 ${ok} 个文件`;
    } else {
      $("receive-msg").textContent = "下载失败，请刷新后重试";
    }
  } finally {
    state.receiving = false;
    state.receivingPushId = null;
    setProgressVisible("receive-progress-wrap", "receive-progress-pct", false);
    await refreshReceiveQueue();
    loadHistory();
  }
}

async function autoReceiveNew(items) {
  if (!state.autoReceive || state.receiving || !items.length) return;
  const fresh = items.filter((i) => !state.knownPushIds.has(i.push_id));
  if (!fresh.length) return;
  state.receiving = true;
  if (state.native) {
    try {
      window.Hantransfer.downloadPushBatch(JSON.stringify(fresh.map((i) => i.push_id)));
    } catch (e) {
      $("receive-msg").textContent = String(e);
      state.receiving = false;
    }
    return;
  }
  try {
    await downloadPushBatchBrowser(fresh);
  } catch (_) {
    state.receiving = false;
  }
}

function onNewPushDetected(count) {
  if (count <= 0) return;
  const msg = `电脑发来 ${count} 个文件`;
  if (document.querySelector("#tab-receive.active")) return;
  tab("receive");
  $("receive-msg").textContent = msg;
}

async function refreshReceiveQueue(opts = {}) {
  const statusEl = $("receive-status");
  if (opts.toast) toastLoading("正在查询待接收文件…", "receive-refresh");
  if (!state.selected) {
    setStatusBanner(statusEl, "请先选择电脑", "warn");
    renderReceiveList([]);
    if (opts.toast) finishToast("receive-refresh", "请先选择电脑", "warn");
    return;
  }
  if (!state.native && !state.trusted) {
    setStatusBanner(statusEl, "请先连接并信任电脑", "warn");
    renderReceiveList([]);
    if (opts.toast) finishToast("receive-refresh", "请先连接并信任电脑", "warn");
    return;
  }
  if (receiveRefreshInFlight) {
    receiveRefreshQueued = true;
    return;
  }
  receiveRefreshInFlight = true;
  try {
    let items = [];
    if (state.native) {
      items = await fetchPushPendingNative();
    } else {
      items = await fetchPushPendingBrowser();
    }
    if (statusEl) {
      const totalSize = items.reduce((s, i) => s + (i.size || 0), 0);
      setStatusBanner(
        statusEl,
        items.length
          ? `有 ${items.length} 个文件待接收（共 ${formatSize(totalSize)}）`
          : "暂无待接收文件",
        items.length ? "info" : "ok",
      );
    }
    updateReceiveBadge(items.length);
    const freshCount = items.filter((i) => !state.knownPushIds.has(i.push_id)).length;
    if (freshCount > 0) onNewPushDetected(freshCount);
    renderReceiveList(items);
    if (!state.receiving) await autoReceiveNew(items);
    if (opts.toast) {
      finishToast(
        "receive-refresh",
        items.length ? `有 ${items.length} 个文件待接收` : "暂无待接收文件",
        items.length ? "info" : "success",
      );
    }
  } catch (e) {
    setStatusBanner(statusEl, String(e), "err");
    if (opts.toast) finishToast("receive-refresh", String(e), "error");
  } finally {
    receiveRefreshInFlight = false;
    if (receiveRefreshQueued) {
      receiveRefreshQueued = false;
      refreshReceiveQueue().catch(() => {});
    }
  }
}

function fetchPushPendingNative() {
  return new Promise((resolve) => {
    let settled = false;
    const finish = (items) => {
      if (settled) return;
      settled = true;
      window.onPushPendingResult = null;
      resolve(Array.isArray(items) ? items : []);
    };
    const prev = window.onPushPendingResult;
    window.onPushPendingResult = function (payload) {
      let items = payload;
      if (typeof items === "string") {
        try { items = JSON.parse(items); } catch (_) { items = []; }
      }
      finish(items);
      if (typeof prev === "function") {
        try { prev(payload); } catch (_) {}
      }
    };
    try {
      if (window.Hantransfer.pollPushPendingAsync) {
        window.Hantransfer.pollPushPendingAsync();
        setTimeout(() => finish([]), 8000);
        return;
      }
      const raw = window.Hantransfer.pollPushPendingJson?.() || "[]";
      finish(JSON.parse(raw));
    } catch (_) {
      finish([]);
    }
  });
}

window.onReceiveProgressPayload = function (payload) {
  const progEl = $("receive-progress");
  const msgEl = $("receive-msg");
  if (payload?.push_id && payload?.batch_total > 1) {
    state.receivingPushId = payload.push_id;
    renderReceiveList(state.pendingReceiveItems);
  }
  if (payload?.batch_total > 0 && payload?.index > 0) {
    const prefix = payload.batch_total > 1 ? `(${payload.index}/${payload.batch_total}) ` : "";
    msgEl.textContent = payload.error
      ? `${prefix}${payload.name} 失败`
      : `${prefix}${payload.name}`;
  }
  if (payload?.total > 0 && progEl) {
    updateProgressBar(progEl, "receive-progress-wrap", "receive-progress-pct", (payload.sent / payload.total) * 100);
    if (payload.batch_total > 1 && payload.index > 0) {
      const pct = Math.round((payload.sent / payload.total) * 100);
      msgEl.textContent = `(${payload.index}/${payload.batch_total}) ${payload.name} · ${pct}%`;
    }
  }
  if (payload?.done && !payload?.batch_done) {
    if (!payload.error && payload.push_id) state.knownPushIds.add(payload.push_id);
    return;
  }
  if (payload?.batch_done) {
    msgEl.textContent = payload.error && !payload.success_count
      ? (payload.error || payload.name || "失败")
      : (payload.name || "完成");
    setProgressVisible("receive-progress-wrap", "receive-progress-pct", false);
    state.receiving = false;
    state.receivingPushId = null;
    renderReceiveList(state.pendingReceiveItems);
    refreshReceiveQueue();
    loadHistory();
    if (payload.error && !payload.success_count) {
      finishToast("receive-dl", payload.error || "下载失败", "error");
    } else {
      finishToast("receive-dl", payload.name || "下载完成", "success");
    }
  }
};

function parseApiBody(body) {
  if (body && body.ok === false) {
    throw new Error(body.error?.message || "request failed");
  }
  return body?.data ?? body;
}

function randomUuid() {
  if (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function") {
    return crypto.randomUUID();
  }
  if (typeof crypto !== "undefined" && typeof crypto.getRandomValues === "function") {
    const bytes = new Uint8Array(16);
    crypto.getRandomValues(bytes);
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    const hex = Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("");
    return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
  }
  return "xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx".replace(/[xy]/g, (c) => {
    const r = (Math.random() * 16) | 0;
    const v = c === "x" ? r : (r & 0x3) | 0x8;
    return v.toString(16);
  });
}

async function phoneDeviceId() {
  let id = localStorage.getItem("hantransfer-device-id");
  if (!id) {
    id = randomUuid();
    localStorage.setItem("hantransfer-device-id", id);
  }
  return id;
}

async function handshakePc(base) {
  const { hs, body, status } = await fetchHandshakeStatus(base);
  if (hs.status === 202 || status === "pending") {
    return { ok: true, message: "等待电脑确认信任…（请在 PC 浏览器打开管理页点「允许连接」）", pending: true };
  }
  if (hs.status === 403 || status === "rejected") {
    return { ok: false, message: "电脑已拒绝此设备，请在 PC 管理页「已拒绝设备」中点「解除拒绝」后重试" };
  }
  if (!hs.ok && !body?.data) {
    throw new Error(body.error?.message || `握手失败: ${hs.status}`);
  }
  return { ok: true, message: "已连接并已信任", pending: false };
}

async function connectToPc(host, port) {
  toastLoading("正在连接电脑…", "connect");
  if (state.native) {
    try {
      window.Hantransfer.connectToPcAsync?.(host, port);
    } catch (err) {
      dismissToast("connect");
      throw err;
    }
    return;
  }
  const probeBase = `http://${formatHttpHost(host)}:${port}`;
  const resp = await fetch(`${probeBase}/api/v1/status`);
  if (!resp.ok) throw new Error(`status ${resp.status}`);
  const info = parseApiBody(await resp.json());
  const connectHost = info.lan_ip || host;
  const connectPort = info.port || port;
  const base = `http://${formatHttpHost(connectHost)}:${connectPort}`;
  const d = {
    id: connectHost,
    name: info.name || connectHost,
    platform: info.platform || "windows",
    host: connectHost,
    port: connectPort,
  };
  state.devices = [d];
  selectDevice(d);
  $("manual-ip").classList.add("hidden");

  setStatusBanner($("scan-status"), "握手中…", "info");
  const hs = await handshakePc(base);
  setStatusBanner($("scan-status"), hs.message, hs.pending ? "warn" : (hs.ok ? "ok" : "err"));
  if (hs.pending) {
    dismissToast("connect");
    toastWarn("请在电脑上点「允许」完成信任");
    setTrusted(false);
    pollTrustUntilReady(base);
  } else if (hs.ok) {
    dismissToast("connect");
    onConnectedTrusted();
  } else {
    dismissToast("connect");
    setTrusted(false);
    toastErr(hs.message || "连接失败");
  }
  renderDevices();
  return d;
}

window.onNativeConnectResult = function (payload) {
  let p = payload;
  if (typeof p === "string") {
    try { p = JSON.parse(p); } catch (_) { return; }
  }
  // Concurrent connect while one is in-flight — keep loading toast for the real result.
  if (!p?.ok && p?.error === "正在连接中，请稍候") {
    return;
  }
  if (p?.device) {
    state.devices = [p.device];
    selectDevice(p.device, { ensureTrust: false, toastSelect: false });
    $("manual-ip")?.classList.add("hidden");
  }
  renderDevices();
  if (p?.ok && p?.pending) {
    dismissToast("connect");
    toastWarn("请在电脑上点「允许」完成信任");
    setStatusBanner($("scan-status"), p.message || "等待电脑确认信任…", "warn");
    setTrusted(false);
    ensureNativeTrust();
    return;
  }
  if (p?.ok) {
    dismissToast("connect");
    notifyConnected({ message: p.message || "已连接并已信任", toast: true });
    return;
  }
  dismissToast("connect");
  setTrusted(false);
  const err = p?.error || "连接失败";
  setStatusBanner($("scan-status"), err, "err");
  toastErr(err);
};

function initMode() {
  state.native = isNativeApp();
  document.body.classList.toggle("native-app", state.native);
  refreshAppInfo();
  if (state.native) {
    $("native-top-nav")?.classList.remove("hidden");
    $("native-shortcuts")?.classList.remove("hidden");
    $("manual-ip")?.classList.remove("hidden");
    $("btn-manual-ip")?.classList.add("hidden");
    $("send-browser-only")?.classList.add("hidden");
    $("send-native-hint")?.classList.remove("hidden");
    $("az-browser-only")?.classList.add("hidden");
    $("az-native-only")?.classList.remove("hidden");
    updateAzDeviceHint();
    $("tabs-scroll-hint")?.classList.remove("hidden");
    document.querySelectorAll(".file-drop").forEach((el) => el.classList.add("hidden"));
    $("btn-send-file").textContent = "浏览内部存储";
    $("native-browse")?.classList.remove("hidden");
    $("btn-quick-send").textContent = "选择文件并发送";
    $("receive-auto-hint")?.classList.remove("hidden");
    $("receive-auto-toggle")?.classList.remove("hidden");
    if ($("receive-auto-hint")) {
      $("receive-auto-hint").textContent = "自动保存至 Downloads/hantransfer";
    }
    state.autoReceive = true;
    $("chk-auto-receive")?.addEventListener("change", (e) => {
      state.autoReceive = e.target.checked;
    });

    try {
      const raw = window.Hantransfer.getSavedDeviceJson?.();
      if (raw) {
        const saved = JSON.parse(raw);
        if (saved?.host && $("pc-host")) $("pc-host").value = saved.host;
        if (saved?.port && $("pc-port")) $("pc-port").value = saved.port;
      }
    } catch (_) {}

    setStatusBanner($("scan-status"), "正在连接电脑…", "info");
    try { window.Hantransfer.startDiscoveryFromJs?.(); } catch (_) {}
    // Single auto-connect path for saved PC (do not also call ensureTrusted here).
    try { window.Hantransfer.autoConnectSavedAsync?.(); } catch (_) {}
  } else {
    $("native-top-nav")?.classList.add("hidden");
    $("native-shortcuts")?.classList.add("hidden");
    $("send-native-hint")?.classList.add("hidden");
    $("tabs-scroll-hint")?.classList.add("hidden");
    $("send-browser-only")?.classList.remove("hidden");
    $("receive-auto-hint")?.classList.remove("hidden");
    $("receive-auto-toggle")?.classList.remove("hidden");
    if ($("receive-auto-hint")) {
      $("receive-auto-hint").textContent = "自动下载并确认接收（浏览器默认开启）";
    }
    state.autoReceive = true;
    $("chk-auto-receive")?.addEventListener("change", (e) => {
      state.autoReceive = e.target.checked;
    });
    $("az-browser-only")?.classList.remove("hidden");
    $("az-native-only")?.classList.add("hidden");

    const host = location.hostname;
    const port = parseInt(location.port, 10) || 7822;
    const onPcPage = host && host !== "localhost" && host !== "127.0.0.1";

    if (onPcPage) {
      $("pc-host").value = host;
      $("pc-port").value = port;
      connectToPc(host, port).catch((err) => {
        if (state.devices.length === 0) {
          showConnectHelp(host, port, err);
          $("pc-host").value = host;
          $("manual-ip").classList.remove("hidden");
        } else {
          setStatusBanner($("scan-status"), `已发现 PC，握手异常：${err.message || err}`, "warn");
        }
      });
    } else {
      $("manual-ip").classList.remove("hidden");
      const saved = localStorage.getItem("hantransfer-device");
      if (saved) {
        try { selectDevice(JSON.parse(saved)); } catch (_) {}
      }
      setStatusBanner($("scan-status"), "请输入电脑 IP，或访问 http://<电脑IP>:7822/m/", "info");
    }
  }
}

function showConnectHelp(host, port, err) {
  const el = $("scan-status");
  if (el) {
    el.className = "status-banner err";
    el.innerHTML =
      `无法连接 ${esc(host)}:${port}<br>` +
      "<span class='hint'>请确认：① 同一 WiFi ② PC 已运行 hantransfer ③ 防火墙已放行</span>";
  }
  if (err) console.warn("connect failed", err);
}

$("btn-save-ip").onclick = () => {
  const host = $("pc-host").value.trim();
  const port = parseInt($("pc-port").value, 10) || 7822;
  if (!host) return;
  connectToPc(host, port).catch(() => {
    dismissToast("connect");
    setStatusBanner($("scan-status"), "无法连接，请确认 PC 端 hantransfer 已启动", "err");
    toastErr("无法连接电脑");
  });
};

$("btn-rescan").onclick = () => {
  toastInfo("正在扫描局域网设备…");
  if (state.native) {
    try {
      window.Hantransfer.rescanDiscoveryFromJs?.()
        ?? window.Hantransfer.startDiscoveryFromJs?.()
        ?? window.Hantransfer.startDiscovery?.();
    } catch (_) {}
    return;
  }
  const host = location.hostname;
  const port = parseInt(location.port, 10) || 7822;
  if (host && host !== "localhost" && host !== "127.0.0.1") {
    connectToPc(host, port).catch((err) => {
      setStatusBanner($("scan-status"), `重连失败: ${err.message || err}`, "err");
    });
  }
};

async function uploadBrowserBatch(files, ui = {}) {
  const msgEl = ui.msgEl || $("send-msg");
  const uiKey = ui.uiKey || "send";
  const uploadUi = resolveUploadUi(uiKey);
  if (!files.length) { msgEl.textContent = "请选择文件"; toastWarn("请选择文件"); return; }
  beginUploadSession(uiKey);
  toastLoading(`正在发送 ${files.length} 个文件…`, "upload");
  let ok = 0;
  let skipped = 0;
  let fail = 0;
  for (let i = 0; i < files.length; i++) {
    if (state.upload.paused) break;
    msgEl.textContent = `上传 ${i + 1}/${files.length}: ${files[i].name}`;
    try {
      const outcome = await uploadBrowser(files[i], { ...ui, uiKey, batch: true, index: i + 1, total: files.length });
      if (outcome?.skipped) skipped++;
      else ok++;
      applyUploadProgress({
        name: files[i].name,
        sent: files[i].size || 1,
        total: files[i].size || 1,
        index: i + 1,
        batch_total: files.length,
        done: false,
      }, uiKey);
    } catch (e) {
      if (state.upload.paused) break;
      fail++;
      msgEl.textContent = `${files[i].name}: ${e.message || e}`;
      // Keep going — only drop the failed/duplicate item, not the whole batch.
    }
  }

  const paused = state.upload.paused;
  if (paused) {
    msgEl.textContent = `已暂停，已完成 ${ok + skipped}/${files.length}`;
    applyUploadProgress({
      name: "已暂停",
      sent: 0,
      total: 1,
      index: ok + skipped,
      batch_total: files.length,
      done: true,
      paused: true,
      error: `已暂停，已完成 ${ok + skipped}/${files.length} 个文件`,
    }, uiKey);
    endUploadSession();
    finishToast("upload", `已暂停，已完成 ${ok + skipped}/${files.length} 个文件`, "warn");
    return;
  }

  const parts = [];
  if (ok) parts.push(`已发送 ${ok}`);
  if (skipped) parts.push(`跳过 ${skipped} 个重复`);
  if (fail) parts.push(`失败 ${fail}`);
  const summary = parts.length ? `${parts.join("，")}（共 ${files.length}）` : "没有可发送的文件";
  msgEl.textContent = summary;
  applyUploadProgress({
    name: summary,
    sent: 1,
    total: 1,
    index: ok + skipped,
    batch_total: files.length,
    done: true,
    error: ok + skipped === 0 ? summary : null,
  }, uiKey);
  endUploadSession();
  if (ok + skipped === 0) {
    setProgressVisible(uploadUi.wrapId, null, false);
    finishToast("upload", summary, "error");
  } else if (fail) {
    finishToast("upload", summary, "warn");
  } else {
    setProgressVisible(uploadUi.wrapId, null, false);
    finishToast("upload", summary, "success");
  }
}

async function uploadBrowser(file, ui = {}) {
  const msgEl = ui.msgEl || $("send-msg");
  const uiKey = ui.uiKey || "send";
  const uploadUi = resolveUploadUi(uiKey);
  const { batch = false, index = 1, total = 1 } = ui;
  const d = state.selected;
  if (!d) { msgEl.textContent = "请先选择电脑"; throw new Error("请先选择电脑"); }
  const base = `http://${d.host}:${d.port}`;
  const deviceId = await phoneDeviceId();

  if (index === 1 && !batch) beginUploadSession(uiKey);

  if (index === 1) {
    msgEl.textContent = "握手中…";
    const { hs, body: hsBody, status: hsStatus } = await fetchHandshakeStatus(base);
    if (hs.status === 202 || hsStatus === "pending") {
      msgEl.textContent = "请在电脑浏览器打开管理页确认信任，然后重试";
      throw new Error("等待信任");
    }
    if (hs.status === 403 || hsStatus === "rejected") {
      msgEl.textContent = "电脑已拒绝此设备";
      throw new Error("已拒绝");
    }
    if (!hs.ok) {
      msgEl.textContent = hsBody.error?.message || `握手失败: ${hs.status}`;
      throw new Error(hsBody.error?.message || `握手失败: ${hs.status}`);
    }
  }

  const transferId = randomUuid();
  const metadataObj = {
    filename: file.name,
    size: file.size,
    hash: "sha256:server",
    type: "file",
    source: "HAN-PHONE-WEB",
  };
  const metadata = JSON.stringify(metadataObj);

  // Same name+size on PC → skip body transfer (only this file).
  try {
    const checkResp = await fetch(`${base}/api/v1/files/check`, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        "X-Hantransfer-Device-ID": deviceId,
      },
      body: metadata,
    });
    if (checkResp.ok) {
      const checkBody = await checkResp.json();
      const data = checkBody.data || checkBody;
      if (data.exists || data.skipped) {
        msgEl.textContent = total > 1
          ? `跳过重复 ${index}/${total}: ${file.name}`
          : `已存在，跳过: ${file.name}`;
        applyUploadProgress({
          name: file.name,
          sent: file.size || 1,
          total: file.size || 1,
          index,
          batch_total: total,
          done: false,
        }, uiKey);
        if (!batch) {
          applyUploadProgress({
            name: file.name,
            sent: file.size || 1,
            total: file.size || 1,
            index: 1,
            batch_total: 1,
            done: true,
          }, uiKey);
          endUploadSession();
          finishToast("upload", `已存在，跳过: ${file.name}`, "success");
        }
        return { skipped: true, path: data.path || "" };
      }
    }
  } catch (_) {
    /* check optional — fall through to upload */
  }

  const form = new FormData();
  form.append("metadata", new Blob([metadata], { type: "application/json" }), "metadata.json");
  form.append("file", file, file.name);

  msgEl.textContent = total > 1 ? `上传 ${index}/${total}: ${file.name}` : "上传中…";
  applyUploadProgress({
    name: file.name,
    sent: 0,
    total: file.size || 1,
    index,
    batch_total: total,
    done: false,
  }, uiKey);

  const body = await new Promise((resolve, reject) => {
    const xhr = new XMLHttpRequest();
    state.upload.xhr = xhr;
    xhr.upload.onprogress = (e) => {
      if (!e.lengthComputable) return;
      applyUploadProgress({
        name: file.name,
        sent: e.loaded,
        total: e.total,
        index,
        batch_total: total,
        done: false,
      }, uiKey);
    };
    xhr.onload = () => {
      state.upload.xhr = null;
      let parsed = {};
      try { parsed = JSON.parse(xhr.responseText || "{}"); } catch (_) {}
      resolve({ ok: xhr.status >= 200 && xhr.status < 300, status: xhr.status, body: parsed });
    };
    xhr.onerror = () => {
      state.upload.xhr = null;
      reject(new Error("网络错误"));
    };
    xhr.onabort = () => {
      state.upload.xhr = null;
      reject(new Error("已暂停"));
    };
    xhr.open("POST", `${base}/api/v1/files`);
    xhr.setRequestHeader("X-Hantransfer-Device-ID", deviceId);
    xhr.setRequestHeader("X-Hantransfer-Transfer-ID", transferId);
    xhr.send(form);
  });

  if (state.upload.paused) throw new Error("已暂停");

  if (!body.ok) {
    msgEl.textContent = body.body.error?.message || `上传失败 ${body.status}`;
    throw new Error(body.body.error?.message || `上传失败 ${body.status}`);
  }
  const result = parseApiBody(body.body);
  const pending = result.status === "pending_approval" || String(result.path || "").startsWith("pending");
  const skipped = result.status === "skipped";
  if (!batch) {
    msgEl.textContent = pending
      ? `已送达 PC，等待确认: ${file.name}`
      : skipped
        ? `已存在，跳过: ${file.name}`
        : `已发送: ${file.name}`;
    applyUploadProgress({
      name: file.name,
      sent: file.size || 1,
      total: file.size || 1,
      index: 1,
      batch_total: 1,
      done: true,
    }, uiKey);
    endUploadSession();
    finishToast("upload", pending ? `已送达 PC，等待确认: ${file.name}` : (skipped ? `已存在，跳过: ${file.name}` : `已发送: ${file.name}`), "success");
  }
  pushHistory({
    filename: file.name,
    deviceName: d.name,
    path: pending ? "（待 PC 确认）" : (result.path || ""),
    at: Date.now(),
    type: "file",
  });
  return { skipped, path: result.path || "" };
}

function updateSettingsConnectionUi() {
  const updEl = $("settings-update-status");
  if (!updEl || !state.native) return;
  if (!state.selected) {
    updEl.textContent = "更新：请先在「设备」页选择电脑";
    return;
  }
  const host = state.selected.host;
  if (state.trusted) {
    updEl.textContent = `更新：已连接 ${host}，可检查更新`;
  } else {
    updEl.textContent = `更新：已选 ${host} · ${state.trustHint || "等待 PC 信任确认…"}`;
  }
}

function syncConnectionStateFromNative() {
  if (!state.native) return;
  if (!state.selected) restoreSelectedDeviceFromNative();
  // Do not call probeTrustJson for live handshake — it is cache-only and must not
  // trigger extra connect toasts. Trust comes from autoConnect / ensureTrustedAsync.
  updateSettingsConnectionUi();
}

function refreshSettings() {
  if (!state.native) return;
  refreshAppInfo();
  if (!state.selected) restoreSelectedDeviceFromNative();
  const verEl = $("settings-version");
  const permEl = $("settings-perm-status");
  try {
    const app = JSON.parse(window.Hantransfer.getAppInfoJson?.() || "{}");
    if (verEl) setStatusBanner(verEl, `当前版本：${app.display || app.version || "?"}`, "info");
    const startup = JSON.parse(window.Hantransfer.getStartupStatusJson?.() || "{}");
    if (permEl) {
      const ok = startup.files_access && startup.install_allowed;
      setStatusBanner(
        permEl,
        ok ? "权限：文件访问与安装权限已就绪" : "权限：请在系统设置中开启文件访问与安装未知应用",
        ok ? "ok" : "warn",
      );
    }
  } catch (_) {}
  updateSettingsConnectionUi();
}

window.onStartupReady = function () {
  if (!state.native) return;
  refreshSettings();
  if (!state.selected) restoreSelectedDeviceFromNative();
  else mergeSavedDeviceIntoList();
  syncNativeDevice();
  // Trust / connect is owned by initMode → autoConnectSavedAsync.
  // Only poll trust if we already have a selected device but autoConnect missed it.
  if (state.selected && !state.trusted) {
    // Mild delay so we don't race autoConnect's handshake.
    setTimeout(() => {
      if (state.selected && !state.trusted) ensureNativeTrust();
    }, 1500);
  } else if (!state.selected && state.devices.length === 0) {
    setStatusBanner($("scan-status"), "正在扫描局域网…可下方手动输入 PC IP", "info");
  }
};

function loadAzLive2d(force = false) {
  if (!state.native || (state.azLoading && !force)) return;
  state.azLoading = true;
  toastLoading("正在扫描 live2d 文件…", "az-load");
  $("az-error-actions")?.classList.add("hidden");
  $("az-empty")?.classList.add("hidden");
  $("az-toolbar")?.classList.add("hidden");
  $("az-loading")?.classList.remove("hidden");
  $("az-file-list")?.replaceChildren();
  state.azSelected.clear();
  state.azFiles = [];
  updateAzSendButtons();
  updateAzDeviceHint();
  setStatusBanner($("az-status"), "正在加载国服 live2d…", "info");
  try {
    window.Hantransfer.loadAzLive2dAsync?.();
  } catch (e) {
    state.azLoading = false;
    dismissToast("az-load");
    $("az-loading")?.classList.add("hidden");
    showAzError(String(e), false);
    toastErr(String(e));
  }
}

const AZ_VIA_LABELS = {
  file: "直读",
  document: "系统接口",
  zero_width: "内核绕过",
  mt_provider: "MT 存储",
  known: "已知路径",
};

function showAzError(message, needPerm) {
  setStatusBanner($("az-status"), message, "err");
  $("az-error-actions")?.classList.remove("hidden");
  $("btn-az-perm")?.classList.toggle("hidden", !needPerm);
}

function updateAzDeviceHint() {
  const el = $("az-device-hint");
  if (!el || !state.native) return;
  if (!state.selected) {
    el.textContent = "请先在「设备」页选择并信任电脑";
    el.classList.remove("hidden", "ok");
    el.classList.add("warn");
    return;
  }
  if (!state.trusted) {
    el.textContent = state.trustHint || `正在连接 ${state.selected.name}…`;
    el.classList.remove("hidden", "ok");
    el.classList.add("warn");
    return;
  }
  el.textContent = `目标：${state.selected.name}`;
  el.classList.remove("hidden", "warn");
  el.classList.add("ok");
}

function updateAzSummary() {
  const el = $("az-summary");
  if (!el) return;
  const n = state.azFiles.length;
  const sel = state.azSelected.size;
  const total = state.azFiles.reduce((s, f) => s + (f.size || 0), 0);
  if (!n) {
    el.textContent = "";
    return;
  }
  el.textContent = sel ? `已选 ${sel}/${n} · ${formatBytes(total)}` : `${n} 个 · ${formatBytes(total)}`;
}

window.onAzLive2dReady = function (data) {
  state.azLoading = false;
  dismissToast("az-load");
  $("az-loading")?.classList.add("hidden");
  if (data?.need_files_access) {
    showAzError("请开启「所有文件访问」后返回", true);
    toastWarn("请开启「所有文件访问」权限");
    return;
  }
  if (!data?.ok) {
    showAzError(data?.error || "无法读取 live2d 目录", !!data?.need_files_access);
    toastErr(data?.error || "无法读取 live2d 目录");
    return;
  }
  $("az-error-actions")?.classList.add("hidden");
  const files = (data.entries || []).filter((e) => !e.is_dir);
  state.azPath = data.path;
  state.azFiles = files.map((e) => ({ ...e, relative: e.relative || e.path }));
  state.azSelected.clear();
  const via = AZ_VIA_LABELS[data.via] || "";
  if (!files.length) {
    $("az-empty")?.classList.remove("hidden");
    $("az-toolbar")?.classList.add("hidden");
    setStatusBanner($("az-status"), via ? `已连接（${via}）· 目录为空` : "目录为空", "warn");
    toastWarn("live2d 目录为空");
    updateAzSendButtons();
    return;
  }
  $("az-empty")?.classList.add("hidden");
  $("az-toolbar")?.classList.remove("hidden");
  renderAzFileList(files);
  const label = via ? `已加载 ${files.length} 个文件（${via}）` : `已加载 ${files.length} 个文件`;
  setStatusBanner($("az-status"), label, "ok");
  toastOk(`已加载 ${files.length} 个 live2d 文件`);
  syncAzSelectAllCheckbox();
};

function renderAzFileList(entries) {
  const list = $("az-file-list");
  if (!list) return;
  list.innerHTML = "";
  entries.forEach((entry) => {
    const key = entry.relative || entry.path;
    const li = document.createElement("li");
    li.className = "browse-item is-file";
    li.innerHTML = `
      <div class="file-meta">
        <span class="file-name">${esc(entry.name)}</span>
        <span class="file-size">${formatBytes(entry.size || 0)}</span>
      </div>`;
    li.onclick = () => {
      if (state.azSelected.has(key)) state.azSelected.delete(key);
      else state.azSelected.add(key);
      li.classList.toggle("selected", state.azSelected.has(key));
      updateAzSendButtons();
      syncAzSelectAllCheckbox();
    };
    list.appendChild(li);
  });
  updateAzSummary();
  updateAzSendButtons();
}

function syncAzSelectAllCheckbox() {
  const chk = $("chk-az-all");
  if (!chk) return;
  const n = state.azFiles.length;
  const sel = state.azSelected.size;
  chk.checked = n > 0 && sel === n;
  chk.indeterminate = sel > 0 && sel < n;
}

function toggleAzSelectAll(checked) {
  state.azSelected.clear();
  if (checked) {
    state.azFiles.forEach((f) => state.azSelected.add(f.relative || f.path));
  }
  document.querySelectorAll("#az-file-list .browse-item").forEach((li, i) => {
    const entry = state.azFiles[i];
    if (entry) li.classList.toggle("selected", state.azSelected.has(entry.relative || entry.path));
  });
  updateAzSendButtons();
  updateAzSummary();
  syncAzSelectAllCheckbox();
}

function updateAzSendButtons() {
  const selBtn = $("btn-az-send-selected");
  const allBtn = $("btn-az-send-all");
  const canSend = state.trusted && state.selected && !state.azSending;
  const n = state.azSelected.size;
  const total = state.azFiles.length;
  if (selBtn) {
    selBtn.textContent = n ? `发送选中 (${n})` : "发送选中";
    selBtn.disabled = !canSend || n === 0;
  }
  if (allBtn) {
    allBtn.disabled = !canSend || total === 0;
  }
  updateAzSummary();
}

function ensureAzCanSend() {
  if (!state.selected) {
    setStatusBanner($("az-status"), "请先在「设备」页选择电脑", "warn");
    toastWarn("请先在「设备」页选择电脑");
    tab("devices");
    return false;
  }
  if (!state.trusted) {
    setStatusBanner($("az-status"), "等待电脑确认信任…", "warn");
    toastWarn("等待电脑确认信任…");
    ensureNativeTrust();
    return false;
  }
  return true;
}

function ensureCanSend(msgEl) {
  if (!state.selected) {
    if (msgEl) msgEl.textContent = "请先在「设备」页选择电脑";
    toastWarn("请先在「设备」页选择电脑");
    tab("devices");
    return false;
  }
  if (!state.trusted) {
    const hint = state.trustHint || "等待电脑确认信任…请在 PC 打开 http://电脑IP:7822/ 点允许";
    if (msgEl) msgEl.textContent = hint;
    toastWarn("等待电脑确认信任…");
    ensureNativeTrust();
    return false;
  }
  return true;
}

function sendAzSelected() {
  if (!ensureAzCanSend()) return;
  const paths = Array.from(state.azSelected);
  if (!paths.length) {
    setStatusBanner($("az-status"), "请先勾选要发送的文件", "warn");
    toastWarn("请先勾选要发送的文件");
    return;
  }
  state.azSending = true;
  updateAzSendButtons();
  beginUploadSession("az");
  toastLoading(`正在发送 ${paths.length} 个文件…`, "az-send");
  try {
    window.Hantransfer.sendAzSelectedJson(JSON.stringify(paths));
  } catch (e) {
    state.azSending = false;
    updateAzSendButtons();
    endUploadSession();
    setStatusBanner($("az-status"), String(e), "err");
    finishToast("az-send", String(e), "error");
  }
}

function sendAzAll() {
  if (!ensureAzCanSend()) return;
  state.azSending = true;
  updateAzSendButtons();
  beginUploadSession("az");
  toastLoading("正在发送全部 live2d 文件…", "az-send");
  try {
    window.Hantransfer.sendAzBatch("live2d");
  } catch (e) {
    state.azSending = false;
    updateAzSendButtons();
    endUploadSession();
    setStatusBanner($("az-status"), String(e), "err");
    finishToast("az-send", String(e), "error");
  }
}

function clearCheckingMsg() {
  const msgEl = $("settings-msg");
  if (msgEl && msgEl.textContent === "检查中…") msgEl.textContent = "";
  $("btn-check-update")?.removeAttribute("disabled");
}

function restoreSelectedDeviceFromNative() {
  if (!state.native) return false;
  try {
    const raw = window.Hantransfer.getSavedDeviceJson?.();
    if (!raw) return false;
    const d = deviceFromSaved(JSON.parse(raw));
    if (!d) return false;
    state.selected = d;
    if ($("selected-name")) $("selected-name").textContent = `${d.name} (${d.host})`;
    syncNativeDevice();
    mergeSavedDeviceIntoList();
    renderDevices();
    return true;
  } catch (_) {
    return false;
  }
}

function syncNativeDevice() {
  if (!state.native || !state.selected) return;
  try {
    window.Hantransfer.syncSelectedDevice?.(
      state.selected.host,
      Number(state.selected.port) || 7822,
      state.selected.id || null,
    );
  } catch (_) {}
}

function applyUpdateInfo(info) {
  const updEl = $("settings-update-status");
  const msgEl = $("settings-msg");
  const installBtn = $("btn-install-update");
  clearCheckingMsg();
  dismissToast("update-check");
  if (!info?.ok) {
    const err = info?.error || "检查失败";
    if (updEl) updEl.textContent = `更新：${err}`;
    if (msgEl) msgEl.textContent = err;
    installBtn?.classList.add("hidden");
    toastErr(err);
    return;
  }
  if (info.update_available) {
    if (updEl) updEl.textContent = `发现新版本 ${info.remote_display || info.version_name || ""}（当前 ${info.local_display}）`;
    if (msgEl) msgEl.textContent = "可点「下载并安装」";
    installBtn?.classList.remove("hidden");
    toastOk(`发现新版本 ${info.remote_display || ""}`);
  } else {
    if (updEl) updEl.textContent = `已是最新版本（${info.local_display}）`;
    if (msgEl) msgEl.textContent = "";
    installBtn?.classList.add("hidden");
    toastOk("已是最新版本");
  }
}

function checkAppUpdate() {
  if (!state.native) {
    clearCheckingMsg();
    applyUpdateInfo({ ok: false, error: "非 App 环境" });
    return;
  }
  if (!state.selected) restoreSelectedDeviceFromNative();
  if (!state.selected) {
    clearCheckingMsg();
    applyUpdateInfo({ ok: false, error: "请先在「设备」页选择电脑，或手动输入 PC IP" });
    return;
  }
  toastLoading("正在检查更新…", "update-check");
  syncNativeDevice();
  if (state.updateCheckTimer) {
    clearTimeout(state.updateCheckTimer);
    state.updateCheckTimer = null;
  }
  state.updateCheckTimer = setTimeout(() => {
    state.updateCheckTimer = null;
    clearCheckingMsg();
    applyUpdateInfo({ ok: false, error: "检查更新超时。请确认 PC 已运行 hantransfer 且在同一 WiFi" });
  }, 12000);
  try {
    if (window.Hantransfer.checkAppUpdateAsync) {
      window.Hantransfer.checkAppUpdateAsync();
      return;
    }
    const raw = window.Hantransfer.checkAppUpdateJson?.();
    if (state.updateCheckTimer) {
      clearTimeout(state.updateCheckTimer);
      state.updateCheckTimer = null;
    }
    if (!raw) {
      clearCheckingMsg();
      applyUpdateInfo({ ok: false, error: "检查更新不可用" });
      return;
    }
    const info = JSON.parse(raw);
    applyUpdateInfo(info);
  } catch (e) {
    if (state.updateCheckTimer) {
      clearTimeout(state.updateCheckTimer);
      state.updateCheckTimer = null;
    }
    clearCheckingMsg();
    applyUpdateInfo({ ok: false, error: String(e) });
  }
}

window.onAppUpdateCheckResult = function (info) {
  if (state.updateCheckTimer) {
    clearTimeout(state.updateCheckTimer);
    state.updateCheckTimer = null;
  }
  applyUpdateInfo(info);
};

window.onClearCacheResult = function (payload) {
  const el = $("settings-cache-msg");
  const msg = payload?.message || (payload?.ok ? "已清理" : "清理失败");
  if (el) el.textContent = msg;
  if (payload?.ok) finishToast("clear-cache", "缓存已清理", "success");
  else finishToast("clear-cache", msg, "error");
};

window.onAppUpdatePayload = function (payload) {
  const msgEl = $("settings-msg");
  const prog = $("update-progress");
  const wrap = $("update-progress-wrap");
  const pct = $("update-progress-pct");
  if (payload.sent != null && payload.total != null && prog) {
    wrap?.classList.remove("hidden");
    const p = payload.total > 0 ? Math.round((payload.sent / payload.total) * 100) : 0;
    prog.value = p;
    if (pct) pct.textContent = `${p}%`;
    if (!payload.done) toastLoading(`正在下载更新 ${p}%…`, "app-update");
  }
  if (payload.error && msgEl) msgEl.textContent = payload.error;
  if (payload.message && msgEl) msgEl.textContent = payload.message;
  if (payload.done) {
    setProgressVisible("update-progress-wrap", "update-progress-pct", false);
    refreshSettings();
    if (payload.error) finishToast("app-update", payload.error, "error");
    else finishToast("app-update", payload.message || "更新包已下载", "success");
  }
};

function sendSelectedFile(inputId, ui) {
  const msgEl = ui?.msgEl || $("send-msg");
  if (state.native) {
    openNativeBrowser();
    tab("send");
    return;
  }
  const input = $(inputId);
  const files = Array.from(input.files || []);
  if (!files.length) { msgEl.textContent = "请选择文件"; return; }
  const uiKey = ui?.uiKey || (inputId === "quick-file-input" ? "quick" : "send");
  const uploadUi = { ...ui, uiKey, msgEl };
  if (files.length === 1) {
    uploadBrowser(files[0], uploadUi).catch((e) => {
      if (!state.upload.paused) msgEl.textContent = String(e);
    });
    return;
  }
  uploadBrowserBatch(files, uploadUi).catch((e) => {
    if (!state.upload.paused) msgEl.textContent = String(e);
  });
}

window.openNativeBrowser = function (path) {
  if (!state.native) return;
  tab("send");
  loadBrowseDir(path || state.browsePath || "");
};

function loadBrowseDir(path) {
  const msg = $("browse-msg");
  if (msg) msg.textContent = "加载中…";
  state.browseLoading = true;
  toastLoading("正在加载目录…", "browse-load");
  if (state.native && window.Hantransfer.browseDirAsync) {
    window.Hantransfer.browseDirAsync(path || "");
    return;
  }
  applyBrowseResult(path, msg);
}

function applyBrowseResult(path, msgEl) {
  const msg = msgEl || $("browse-msg");
  try {
    const raw = window.Hantransfer.browseDirJson?.(path || "");
    if (!raw) return;
    const data = JSON.parse(raw);
    renderBrowseResult(data, msg);
  } catch (e) {
    if (msg) msg.textContent = String(e);
  } finally {
    state.browseLoading = false;
  }
}

window.onBrowseDirResult = function (data) {
  if (data?.loading) return;
  state.browseLoading = false;
  renderBrowseResult(data, $("browse-msg"));
};

function renderBrowseResult(data, msg) {
  if (data?.loading) return;
  dismissToast("browse-load");
  if (!data?.ok) {
    if (msg) {
      msg.textContent = data.error || "无法浏览";
      msg.style.whiteSpace = "pre-wrap";
    }
    toastErr(data.error || "无法浏览此目录");
    if (data.need_files_access && msg) {
      msg.textContent = (data.error || "无法浏览") + " · 请到「设置」开启所有文件访问";
    }
    return;
  }
  state.browsePath = data.path;
  state.browseParent = data.parent || "";
  state.browseSelected.clear();
  if ($("browse-path")) $("browse-path").textContent = data.path;
  renderBrowseList(data.entries || []);
  const via = data.via === "document" ? "（系统文档接口）"
    : data.via === "known" ? "（已知路径）" : "";
  void via;
  if (msg) msg.textContent = "点击文件夹进入 · 点击文件选中后发送";
}

function renderBrowseList(entries) {
  const list = $("browse-list");
  if (!list) return;
  list.innerHTML = "";
  if (!entries.length) {
    list.innerHTML = `<li class="empty-state hint">空目录</li>`;
    return;
  }
  entries.forEach((entry) => {
    const li = document.createElement("li");
    li.className = "browse-item" + (entry.is_dir ? " is-dir" : " is-file");
    const icon = entry.is_dir ? "📁" : "📄";
    const size = entry.is_dir ? "" : ` · ${formatBytes(entry.size || 0)}`;
    li.innerHTML = `<span>${icon} ${esc(entry.name)}${size}</span>`;
    li.onclick = () => {
      if (entry.is_dir) {
        loadBrowseDir(entry.path);
      } else {
        if (state.browseSelected.has(entry.path)) state.browseSelected.delete(entry.path);
        else state.browseSelected.add(entry.path);
        li.classList.toggle("selected", state.browseSelected.has(entry.path));
        updateBrowseSendBtn();
      }
    };
    list.appendChild(li);
  });
  updateBrowseSendBtn();
}

function updateBrowseSendBtn() {
  const btn = $("btn-browse-send");
  if (!btn) return;
  const n = state.browseSelected.size;
  btn.textContent = n ? `发送选中的 ${n} 个文件` : "发送选中文件";
  btn.disabled = n === 0;
}

function sendBrowseSelected() {
  const msg = $("browse-msg");
  const paths = Array.from(state.browseSelected);
  if (!paths.length) {
    if (msg) msg.textContent = "请先点击文件选中（文件夹可点进入）";
    toastWarn("请先选中要发送的文件");
    return;
  }
  if (!ensureCanSend(msg)) return;
  syncNativeDevice();
  beginUploadSession("browse");
  toastLoading(`正在发送 ${paths.length} 个文件…`, "upload");
  try {
    window.Hantransfer.sendFilesByPathJson(JSON.stringify(paths));
    if (msg) msg.textContent = "上传中…";
  } catch (e) {
    endUploadSession();
    if (msg) msg.textContent = String(e);
    finishToast("upload", String(e), "error");
  }
}

function formatBytes(n) {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  return `${(n / 1024 / 1024).toFixed(1)} MB`;
}

function updateFileDropUi(inputId, dropId, defaultTitle, defaultHint) {
  const input = $(inputId);
  const drop = dropId ? $(dropId) : null;
  if (!input || !drop) return;
  const files = Array.from(input.files || []);
  const title = drop.querySelector(".file-drop-title");
  const hint = drop.querySelector(".file-drop-hint");
  if (files.length) {
    drop.classList.add("has-files");
    if (title) {
      title.textContent = files.length === 1
        ? files[0].name
        : `已选 ${files.length} 个文件`;
    }
    if (hint) {
      const total = files.reduce((s, f) => s + (f.size || 0), 0);
      hint.textContent = formatBytes(total);
    }
  } else {
    drop.classList.remove("has-files");
    if (title) title.textContent = defaultTitle;
    if (hint && defaultHint) hint.textContent = defaultHint;
  }
}

function bindFilePicker(btnId, inputId, dropId, ui, defaultTitle, defaultHint) {
  const btn = $(btnId);
  const input = $(inputId);
  if (!btn || !input) return;

  btn.addEventListener("click", () => {
    if (state.native) {
      sendSelectedFile(inputId, ui);
      return;
    }
    if (!input.files?.length) {
      input.click();
      return;
    }
    sendSelectedFile(inputId, ui);
  });

  input.addEventListener("change", () => {
    updateFileDropUi(inputId, dropId, defaultTitle, defaultHint);
    const n = input.files?.length || 0;
    btn.textContent = n ? `发送 ${n} 个文件` : "选择文件并发送";
  });
}

bindFilePicker(
  "btn-send-file",
  "file-input",
  "file-drop",
  { uiKey: "send", msgEl: $("send-msg") },
  "点击选择要发送的文件",
  "支持多选 · 单文件最大 512 MB",
);
bindFilePicker(
  "btn-quick-send",
  "quick-file-input",
  "quick-file-drop",
  { uiKey: "quick", msgEl: $("quick-send-msg") },
  "点击选择文件",
  "支持多选 · 相册 / 下载目录",
);

$("btn-upload-pause")?.addEventListener("click", () => pauseUpload());
$("btn-quick-upload-pause")?.addEventListener("click", () => pauseUpload());
$("btn-az-upload-pause")?.addEventListener("click", () => pauseUpload());

$("btn-go-send-tab")?.addEventListener("click", () => tab("send"));
$("btn-go-send")?.addEventListener("click", () => tab("send"));
$("btn-go-receive")?.addEventListener("click", () => tab("receive"));
$("btn-manual-ip")?.addEventListener("click", () => {
  $("manual-ip")?.classList.remove("hidden");
  tab("devices");
});

$("btn-browse-az")?.addEventListener("click", () => {
  loadBrowseDir("/storage/emulated/0/Android/data/com.bilibili.azurlane/files/AssetBundles/live2d");
});
$("btn-browse-send")?.addEventListener("click", () => sendBrowseSelected());

$("btn-check-update")?.addEventListener("click", () => {
  $("settings-msg").textContent = "检查中…";
  $("btn-check-update")?.setAttribute("disabled", "disabled");
  checkAppUpdate();
});
$("btn-install-update")?.addEventListener("click", () => {
  syncNativeDevice();
  toastLoading("正在下载更新包…", "app-update");
  try { window.Hantransfer.downloadAppUpdate(); } catch (e) {
    $("settings-msg").textContent = String(e);
    finishToast("app-update", String(e), "error");
  }
});
$("btn-clear-cache")?.addEventListener("click", () => {
  const el = $("settings-cache-msg");
  if (el) el.textContent = "清理中…";
  toastLoading("正在清理缓存…", "clear-cache");
  try { window.Hantransfer.clearAppCacheAsync?.(); } catch (e) {
    if (el) el.textContent = String(e);
    finishToast("clear-cache", String(e), "error");
  }
});

$("btn-go-az-tab")?.addEventListener("click", () => {
  toastInfo("正在打开碧蓝页…");
  tab("az");
});
$("btn-az-send-selected")?.addEventListener("click", () => sendAzSelected());
$("btn-az-send-all")?.addEventListener("click", () => sendAzAll());
$("btn-az-refresh")?.addEventListener("click", () => loadAzLive2d(true));
$("btn-az-retry")?.addEventListener("click", () => loadAzLive2d(true));
$("btn-az-perm")?.addEventListener("click", () => {
  try { window.Hantransfer.requestAllFilesAccess(); } catch (e) {
    setStatusBanner($("az-status"), String(e), "err");
  }
});
$("chk-az-all")?.addEventListener("change", (e) => toggleAzSelectAll(e.target.checked));

$("btn-browse-up")?.addEventListener("click", () => {
  if (state.browseParent) loadBrowseDir(state.browseParent);
  else loadBrowseDir("");
});
$("btn-browse-root")?.addEventListener("click", () => loadBrowseDir(""));

$("btn-receive-refresh")?.addEventListener("click", () => refreshReceiveQueue({ toast: true }));
$("btn-receive-all")?.addEventListener("click", () => {
  if (state.receiving) return;
  if (state.native) {
    state.receiving = true;
    toastLoading("正在下载全部文件…", "receive-dl");
    try { window.Hantransfer.downloadAllPush(); } catch (e) {
      $("receive-msg").textContent = String(e);
      state.receiving = false;
    }
  } else {
    refreshReceiveQueue().then(async () => {
      const items = await fetchPushPendingBrowser();
      await downloadPushBatchBrowser(items);
    }).catch((e) => { $("receive-msg").textContent = String(e); });
  }
});

initTapEffects();
if (!isNativeApp()) initMode();
loadHistory();
window.refreshSettings = refreshSettings;
window.loadAzLive2d = loadAzLive2d;
document.addEventListener("visibilitychange", () => {
  if (document.hidden) return;
  if (state.trusted && state.selected) {
    refreshReceiveQueue().catch(() => {});
  } else if (state.native && state.selected && !state.trusted) {
    ensureNativeTrust();
  }
  if (state.native && state.activeTab === "az" && !state.azLoading) loadAzLive2d(true);
});
setInterval(() => {
  if (document.hidden) return;
  if (state.native) {
    try {
      const json = window.Hantransfer.getDevicesJson();
      if (json && json !== lastDevicesJson) {
        lastDevicesJson = json;
        window.onDevicesUpdated(JSON.parse(json));
      }
    } catch (_) {}
  }
  if (state.trusted && state.selected && state.activeTab === "receive") {
    refreshReceiveQueue().catch(() => {});
  } else if (state.native && state.selected && !state.trusted && !state.nativeTrustPoll) {
    ensureNativeTrust();
  }
}, state.native ? 4000 : 5000);
