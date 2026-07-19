import "./menu.css";
import { tauriInvoke as invoke, waitForTauriInternals } from "../lib/tauriInvoke";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { listen } from "@tauri-apps/api/event";
import { filterSkinsByKind } from "../lib/skinKindFilter";
import type { CharacterSkinInfo } from "../lib/xiaohan";

interface CharacterBrief {
  id: string;
  name: string;
  active: boolean;
}

interface PetMenuSkinsPayload {
  character_id: string;
  character_name: string;
  model_id: string;
  skins: CharacterSkinInfo[];
}

const MENU_AUTO_CLOSE_MS = 8000;
const FAVORITES_SETTING_KEY = "character_favorites";

const rootMaybe = document.getElementById("pet-menu-root");
if (!rootMaybe) throw new Error("pet-menu-root missing");
const root: HTMLElement = rootMaybe;

root.innerHTML = `
  <div class="pet-menu-view" data-menu-view="main">
    <div class="pet-menu-head">桌宠菜单<span class="pet-menu-countdown" data-menu-countdown></span></div>
    <div class="pet-menu-body">
      <button type="button" class="pet-menu-item" data-action="main">
        <span class="pet-menu-icon" aria-hidden="true">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round">
            <rect x="3" y="4" width="18" height="14" rx="2" />
            <path d="M8 20h8" />
          </svg>
        </span>
        <span class="pet-menu-text">打开小寒桌宠</span>
      </button>
      <button type="button" class="pet-menu-item" data-action="edit-bounds">
        <span class="pet-menu-icon" aria-hidden="true">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round">
            <path d="M4 8V4h4M20 8V4h-4M4 16v4h4M20 16v4h-4" />
          </svg>
        </span>
        <span class="pet-menu-text">编辑范围</span>
      </button>
      <div class="pet-menu-row" data-row="hit-areas" hidden>
        <span class="pet-menu-row-label">显示点击区域</span>
        <button type="button" class="pet-menu-switch" data-action="toggle-hit-areas" aria-pressed="false" aria-label="点击区域开关">
          <span class="pet-menu-switch-track"><span class="pet-menu-switch-thumb"></span></span>
        </button>
      </div>
      <div class="pet-menu-divider" role="separator"></div>
      <div class="pet-menu-row">
        <span class="pet-menu-row-label">气泡台词</span>
        <button type="button" class="pet-menu-switch" data-action="toggle-bubble" aria-pressed="true" aria-label="气泡台词开关">
          <span class="pet-menu-switch-track"><span class="pet-menu-switch-thumb"></span></span>
        </button>
      </div>
      <div class="pet-menu-divider" role="separator"></div>
      <button type="button" class="pet-menu-item" data-action="open-skins-menu">
        <span class="pet-menu-icon" aria-hidden="true">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round">
            <path d="M12 3v3M8 6l2 2M16 6l-2 2" />
            <circle cx="12" cy="13" r="5" />
            <path d="M9 21h6" />
          </svg>
        </span>
        <span class="pet-menu-text">切换模型</span>
        <span class="pet-menu-chevron" aria-hidden="true">›</span>
      </button>
      <button type="button" class="pet-menu-item" data-action="open-characters-menu">
        <span class="pet-menu-icon" aria-hidden="true">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round">
            <circle cx="12" cy="8" r="4" />
            <path d="M6 20c0-3.3 2.7-6 6-6s6 2.7 6 6" />
          </svg>
        </span>
        <span class="pet-menu-text">切换人物</span>
        <span class="pet-menu-chevron" aria-hidden="true">›</span>
      </button>
      <div class="pet-menu-divider" role="separator"></div>
      <button type="button" class="pet-menu-item pet-menu-item--danger" data-action="hide">
        <span class="pet-menu-icon" aria-hidden="true">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round">
            <path d="M3 3l18 18" />
            <path d="M10.6 10.6a2 2 0 0 0 2.8 2.8" />
            <path d="M9.9 5.1A9 9 0 0 1 12 5c4 0 7.5 2.7 8.8 6.5" />
            <path d="M6.2 6.2C4.6 7.8 3.5 9.8 3 12c1.3 3.8 4.8 6.5 8.8 6.5 1.1 0 2.1-.2 3-.5" />
          </svg>
        </span>
        <span class="pet-menu-text">隐藏桌宠</span>
      </button>
      <button type="button" class="pet-menu-item pet-menu-item--danger" data-action="quit">
        <span class="pet-menu-icon" aria-hidden="true">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round">
            <path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4" />
            <polyline points="16 17 21 12 16 7" />
            <line x1="21" y1="12" x2="9" y2="12" />
          </svg>
        </span>
        <span class="pet-menu-text">退出</span>
      </button>
    </div>
  </div>
  <div class="pet-menu-view" data-menu-view="characters" hidden>
    <div class="pet-menu-head pet-menu-head--with-back">
      <button type="button" class="pet-menu-back" data-action="menu-back" aria-label="返回">‹</button>
      <span class="pet-menu-head-title">选择人物</span>
    </div>
    <div class="pet-menu-body">
      <div class="pet-menu-sublist" data-menu-list="characters"></div>
    </div>
  </div>
  <div class="pet-menu-view" data-menu-view="skins" hidden>
    <div class="pet-menu-head pet-menu-head--with-back">
      <button type="button" class="pet-menu-back" data-action="menu-back" aria-label="返回">‹</button>
      <span class="pet-menu-head-title" data-menu-skins-title>选择模型</span>
    </div>
    <div class="pet-menu-body pet-menu-body--dual">
      <div class="pet-menu-dual-col">
        <div class="pet-menu-col-title">桌宠</div>
        <div class="pet-menu-sublist" data-menu-list="skins-spine"></div>
      </div>
      <div class="pet-menu-dual-col">
        <div class="pet-menu-col-title">舰娘</div>
        <div class="pet-menu-sublist" data-menu-list="skins-kanmusu"></div>
      </div>
    </div>
  </div>
`;

const menuSkinsSpineEl = root.querySelector('[data-menu-list="skins-spine"]');
const menuSkinsKanmusuEl = root.querySelector('[data-menu-list="skins-kanmusu"]');
const menuCharactersEl = root.querySelector('[data-menu-list="characters"]');
const menuMainViewEl = root.querySelector('[data-menu-view="main"]');
const menuCharactersViewEl = root.querySelector('[data-menu-view="characters"]');
const menuSkinsViewEl = root.querySelector('[data-menu-view="skins"]');
const menuSkinsTitleEl = root.querySelector("[data-menu-skins-title]");
const menuBubbleSwitchEl = root.querySelector<HTMLButtonElement>('[data-action="toggle-bubble"]');
const menuHitAreasRowEl = root.querySelector('[data-row="hit-areas"]');
const menuHitAreasSwitchEl = root.querySelector('[data-action="toggle-hit-areas"]');
const menuCountdownEl = root.querySelector<HTMLElement>("[data-menu-countdown]");

let menuAutoCloseTimer: ReturnType<typeof setTimeout> | null = null;
let menuCountdownTimer: ReturnType<typeof setInterval> | null = null;
let menuAutoCloseDeadline = 0;
let menuSwitchBusy = false;
let menuSwitchBusyTimer: ReturnType<typeof setTimeout> | null = null;
let menuSkinCharacterId: string | null = null;
/** 从「切换人物」进入皮肤页时浏览的角色；null 表示当前桌宠角色 */
let menuBrowsingCharacterId: string | null = null;
let menuSuppressBlurUntil = 0;
let bubbleEnabled = true;
let menuPickerRefreshTimer: ReturnType<typeof setTimeout> | null = null;
/** spine | kanmusu — 当前上桌引擎 */
let companionEngine: "spine" | "kanmusu" = "spine";
/** 本次菜单会话：显示点击区域（默认关，不写 settings） */
let hitAreasVisible = false;

async function refreshCompanionEngine() {
  try {
    const eng = await invoke<string>("pet_get_companion_engine");
    companionEngine = eng === "kanmusu" ? "kanmusu" : "spine";
  } catch {
    companionEngine = "spine";
  }
  syncKanmusuOnlyRows();
  syncHitAreasToggleUI();
}

/** 仅舰娘桌宠时显示「点击区域」开关 */
function syncKanmusuOnlyRows() {
  menuHitAreasRowEl?.toggleAttribute("hidden", companionEngine !== "kanmusu");
}

function syncHitAreasToggleUI() {
  if (!(menuHitAreasSwitchEl instanceof HTMLElement)) return;
  menuHitAreasSwitchEl.classList.toggle("is-on", hitAreasVisible);
  menuHitAreasSwitchEl.setAttribute("aria-pressed", hitAreasVisible ? "true" : "false");
}

async function setHitAreasVisible(enabled: boolean) {
  hitAreasVisible = enabled;
  syncHitAreasToggleUI();
  menuSuppressBlurUntil = Date.now() + 1200;
  try {
    await invoke("pet_set_hit_areas_visible", { visible: enabled });
  } catch (e) {
    showMenuError(e);
  }
}

function scheduleRefreshPetMenuPickers() {
  if (menuPickerRefreshTimer) clearTimeout(menuPickerRefreshTimer);
  menuPickerRefreshTimer = setTimeout(() => {
    menuPickerRefreshTimer = null;
    void refreshPetMenuPickers();
  }, 120);
}

function parseFavoriteIds(raw: string | null | undefined): string[] {
  if (!raw?.trim()) return [];
  try {
    const parsed = JSON.parse(raw) as unknown;
    if (!Array.isArray(parsed)) return [];
    return parsed.filter((id): id is string => typeof id === "string" && id.length > 0);
  } catch {
    return [];
  }
}

function showMenuError(err: unknown) {
  let banner = root.querySelector(".pet-menu-error");
  if (!banner) {
    banner = document.createElement("div");
    banner.className = "pet-menu-error";
    root.appendChild(banner);
  }
  const msg = err instanceof Error ? err.message : String(err);
  banner.textContent = msg;
  window.setTimeout(() => banner?.remove(), 4000);
}

function syncBubbleToggleUI() {
  if (!menuBubbleSwitchEl) return;
  menuBubbleSwitchEl.classList.toggle("is-on", bubbleEnabled);
  menuBubbleSwitchEl.setAttribute("aria-pressed", bubbleEnabled ? "true" : "false");
}

async function loadBubbleEnabled() {
  if (bubbleSetPending) return;
  try {
    bubbleEnabled = await invoke<boolean>("pet_get_bubble_enabled");
  } catch {
    bubbleEnabled = true;
  }
  syncBubbleToggleUI();
}

let bubbleSetChain: Promise<void> = Promise.resolve();
let bubbleSetPending = false;

async function setBubbleEnabled(enabled: boolean) {
  const prev = bubbleEnabled;
  if (prev === enabled) return;
  menuSuppressBlurUntil = Date.now() + 1500;
  bubbleEnabled = enabled;
  syncBubbleToggleUI();
  bubbleSetPending = true;
  bubbleSetChain = bubbleSetChain.then(async () => {
    try {
      await invoke("pet_set_bubble_enabled", { enabled });
    } catch (e) {
      if (bubbleEnabled === enabled) {
        bubbleEnabled = prev;
        syncBubbleToggleUI();
      }
      showMenuError(e);
    } finally {
      bubbleSetPending = false;
    }
  });
  await bubbleSetChain;
}

function setMenuView(view: "main" | "characters" | "skins") {
  menuMainViewEl?.toggleAttribute("hidden", view !== "main");
  menuCharactersViewEl?.toggleAttribute("hidden", view !== "characters");
  menuSkinsViewEl?.toggleAttribute("hidden", view !== "skins");
}

async function openMenuView(view: "characters" | "skins") {
  setMenuView(view);
  await refreshPetMenuPickers();
}

function resetMenuView() {
  menuBrowsingCharacterId = null;
  setMenuView("main");
}

function renderMenuSublist(
  container: Element | null,
  items: {
    id: string;
    label: string;
    active: boolean;
    disabled?: boolean;
    modelId?: string;
    preferEngine?: "spine" | "kanmusu";
  }[],
  kind: "character" | "skin",
  emptyText?: string,
) {
  if (!container) return;
  container.innerHTML = "";
  if (items.length === 0) {
    const empty = document.createElement("div");
    empty.className = "pet-menu-subempty";
    empty.textContent =
      emptyText ??
      (kind === "character"
        ? companionEngine === "kanmusu"
          ? "当前没有舰娘角色，请先从 Wiki 同步模型"
          : "当前没有收藏角色，请先在人物页收藏"
        : "暂无模型");
    container.appendChild(empty);
    return;
  }
  for (const item of items) {
    const btn = document.createElement("button");
    btn.type = "button";
    btn.className = `pet-menu-subitem${item.active ? " is-active" : ""}`;
    btn.dataset.action = kind === "skin" ? "switch-skin" : "pick-character";
    btn.dataset.id = item.id;
    if (item.modelId) btn.dataset.modelId = item.modelId;
    if (item.preferEngine) btn.dataset.preferEngine = item.preferEngine;
    btn.textContent = item.label;
    if (kind === "skin") {
      btn.disabled = item.active || item.disabled === true || menuSwitchBusy;
    } else {
      btn.disabled = item.disabled === true || menuSwitchBusy;
    }
    container.appendChild(btn);
  }
}

function renderDualSkinLists(skins: CharacterSkinInfo[]) {
  const spineItems = filterSkinsByKind(skins, "spine").map((s) => ({
    id: s.id,
    label: s.model_name && s.model_name !== s.model_id ? s.model_name : s.name,
    active: companionEngine === "spine" && s.active,
    disabled: !s.model_ready,
    modelId: s.model_id,
    preferEngine: "spine" as const,
  }));
  const kanmusuItems = filterSkinsByKind(skins, "kanmusu").map((s) => ({
    id: s.id,
    label: s.name || s.model_name,
    active: companionEngine === "kanmusu" && s.active,
    disabled: !s.kanmusu_ready,
    modelId: s.model_id,
    preferEngine: "kanmusu" as const,
  }));
  renderMenuSublist(menuSkinsSpineEl, spineItems, "skin", "无可切换桌宠模型");
  renderMenuSublist(menuSkinsKanmusuEl, kanmusuItems, "skin", "无可切换舰娘皮肤");
}

async function loadSkinsForCharacter(characterId: string) {
  await refreshCompanionEngine();
  const skinMenu = await invoke<PetMenuSkinsPayload>("characters_pet_menu_skins_for", {
    characterId,
  });
  menuSkinCharacterId = skinMenu.character_id;
  menuBrowsingCharacterId = skinMenu.character_id;
  if (menuSkinsTitleEl) {
    menuSkinsTitleEl.textContent = `选择模型 · ${skinMenu.character_name}`;
  }
  renderDualSkinLists(skinMenu.skins);
}

async function refreshPetMenuPickers() {
  try {
    await refreshCompanionEngine();
    const [favorites, favoritesRaw, skinMenu] = await Promise.all([
      companionEngine === "kanmusu"
        ? invoke<CharacterBrief[]>("characters_pet_menu_kanmusu").catch(
            () => [] as CharacterBrief[],
          )
        : invoke<CharacterBrief[]>("characters_pet_menu_favorites").catch(
            () => [] as CharacterBrief[],
          ),
      invoke<string | null>("settings_get", { key: FAVORITES_SETTING_KEY }),
      (async () => {
        const characterId = menuBrowsingCharacterId;
        if (characterId) {
          return invoke<PetMenuSkinsPayload>("characters_pet_menu_skins_for", {
            characterId,
          });
        }
        return invoke<PetMenuSkinsPayload>("characters_pet_menu_skins");
      })(),
    ]);
    await loadBubbleEnabled();
    menuSkinCharacterId = skinMenu.character_id;
    if (!menuBrowsingCharacterId) {
      menuSkinCharacterId = skinMenu.character_id;
    }
    if (menuSkinsTitleEl) {
      menuSkinsTitleEl.textContent = `选择模型 · ${skinMenu.character_name}`;
    }
    renderDualSkinLists(skinMenu.skins);
    const favoriteIds = new Set(parseFavoriteIds(favoritesRaw));
    let characterItems: { id: string; label: string; active: boolean }[];
    if (companionEngine === "kanmusu") {
      const activeId = skinMenu.character_id;
      characterItems = favorites
        .map((c) => ({
          id: c.id,
          label: c.name,
          active: c.id === activeId,
        }))
        .sort((a, b) => {
          if (a.active !== b.active) return a.active ? -1 : 1;
          return a.label.localeCompare(b.label, "zh-CN");
        });
    } else {
      characterItems = favorites
        .filter((c) => favoriteIds.has(c.id))
        .sort((a, b) => {
          if (a.active !== b.active) return a.active ? -1 : 1;
          return a.name.localeCompare(b.name, "zh-CN");
        })
        .map((c) => ({
          id: c.id,
          label: `★ ${c.name}`,
          active: c.active,
        }));
    }
    renderMenuSublist(menuCharactersEl, characterItems, "character");
  } catch (e) {
    console.error("桌宠菜单加载选项失败", e);
    menuSkinCharacterId = null;
    renderDualSkinLists([]);
    renderMenuSublist(menuCharactersEl, [], "character");
  }
}

function updateMenuCountdown() {
  if (!menuCountdownEl) return;
  const leftMs = menuAutoCloseDeadline - Date.now();
  if (leftMs <= 0) {
    menuCountdownEl.textContent = "";
    return;
  }
  const sec = Math.ceil(leftMs / 1000);
  menuCountdownEl.textContent = ` ${sec}s`;
}

function clearMenuCountdown() {
  if (menuCountdownTimer) {
    clearInterval(menuCountdownTimer);
    menuCountdownTimer = null;
  }
  menuAutoCloseDeadline = 0;
  if (menuCountdownEl) menuCountdownEl.textContent = "";
}

function scheduleAutoClose() {
  if (menuAutoCloseTimer) clearTimeout(menuAutoCloseTimer);
  clearMenuCountdown();
  menuAutoCloseDeadline = Date.now() + MENU_AUTO_CLOSE_MS;
  updateMenuCountdown();
  menuCountdownTimer = setInterval(updateMenuCountdown, 500);
  menuAutoCloseTimer = setTimeout(() => {
    menuAutoCloseTimer = null;
    clearMenuCountdown();
    void invoke("pet_menu_hide");
  }, MENU_AUTO_CLOSE_MS);
}

function cancelAutoClose() {
  if (menuAutoCloseTimer) {
    clearTimeout(menuAutoCloseTimer);
    menuAutoCloseTimer = null;
  }
  clearMenuCountdown();
}

function setMenuSwitchBusy(busy: boolean) {
  menuSwitchBusy = busy;
  if (menuSwitchBusyTimer) {
    clearTimeout(menuSwitchBusyTimer);
    menuSwitchBusyTimer = null;
  }
  if (busy) {
    menuSwitchBusyTimer = setTimeout(() => {
      menuSwitchBusy = false;
      menuSwitchBusyTimer = null;
      void refreshPetMenuPickers();
    }, 15000);
  }
}

async function hideMenu() {
  cancelAutoClose();
  await invoke("pet_menu_hide");
}

root.addEventListener("mouseenter", () => cancelAutoClose());
root.addEventListener("mouseleave", () => scheduleAutoClose());

root.addEventListener("click", async (e) => {
  const btn = (e.target as HTMLElement).closest("button");
  if (!btn) return;

  const action = btn.getAttribute("data-action");
  if (action === "toggle-bubble") {
    e.preventDefault();
    e.stopPropagation();
    void setBubbleEnabled(!bubbleEnabled);
    return;
  }
  if (action === "toggle-hit-areas") {
    e.preventDefault();
    e.stopPropagation();
    void setHitAreasVisible(!hitAreasVisible);
    return;
  }
  if (action === "open-characters-menu") {
    void openMenuView("characters");
    return;
  }
  if (action === "open-skins-menu") {
    menuBrowsingCharacterId = null;
    void openMenuView("skins");
    return;
  }
  if (action === "menu-back") {
    if (menuSkinsViewEl && !menuSkinsViewEl.hasAttribute("hidden") && menuBrowsingCharacterId) {
      menuBrowsingCharacterId = null;
      setMenuView("characters");
      void refreshPetMenuPickers();
      return;
    }
    menuBrowsingCharacterId = null;
    setMenuView("main");
    return;
  }
  if (action === "pick-character") {
    const id = btn.getAttribute("data-id");
    if (!id || menuSwitchBusy) return;
    try {
      await loadSkinsForCharacter(id);
      setMenuView("skins");
    } catch (err) {
      console.error("加载角色皮肤失败", err);
      showMenuError(err);
    }
    return;
  }
  if (action === "switch-skin") {
    const id = btn.getAttribute("data-id");
    if (!id || menuSwitchBusy || btn.disabled) return;
    const preferEngine = btn.getAttribute("data-prefer-engine") || undefined;
    setMenuSwitchBusy(true);
    const prevLabel = btn.textContent;
    btn.textContent = "切换中…";
    try {
      const characterId = menuSkinCharacterId ?? menuBrowsingCharacterId;
      if (!characterId) throw new Error("当前没有可切换模型的角色");
      // 先关菜单，热切换期间不再挡交互
      void hideMenu();
      await invoke("pet_menu_switch_skin", {
        characterId,
        skinId: id,
        // 舰娘热切换几乎立即返回；Spine 仍需等待 ready
        timeoutMs: preferEngine === "kanmusu" ? 8000 : 25000,
        preferEngine: preferEngine ?? null,
      });
      menuBrowsingCharacterId = null;
      window.setTimeout(() => void refreshPetMenuPickers(), 400);
    } catch (err) {
      console.error("桌宠菜单切换失败", err);
      showMenuError(err);
      if (prevLabel) btn.textContent = prevLabel;
    } finally {
      setMenuSwitchBusy(false);
    }
    return;
  }
  if (action === "edit-bounds") {
    cancelAutoClose();
    void hideMenu();
    void invoke("pet_enter_edit_bounds");
    return;
  }
  if (action === "quit") {
    cancelAutoClose();
    // 延迟 invoke，让 click 处理完再退出，避免同步 teardown 菜单 WebView 导致 IPC 挂起
    window.setTimeout(() => {
      void invoke("app_exit").catch((err) => {
        console.error("退出失败", err);
      });
    }, 0);
    return;
  }
  if (action === "main") {
    try {
      menuSuppressBlurUntil = Date.now() + 2000;
      cancelAutoClose();
      await invoke("pet_open_main", { page: null });
    } catch (err) {
      console.error("桌宠菜单操作失败", err);
      showMenuError(err);
    }
    return;
  }
  if (action === "hide") {
    try {
      await invoke("pet_hide", { destroy: false });
    } catch (err) {
      console.error("桌宠菜单操作失败", err);
      showMenuError(err);
    }
    return;
  }
});

interface PetMenuShownPayload {
  suppress_blur_ms?: number;
}

void waitForTauriInternals().then(async () => {
  await loadBubbleEnabled();
  await refreshCompanionEngine();
  resetMenuView();
  void refreshPetMenuPickers();

  void listen<PetMenuShownPayload>("pet-menu-shown", (ev) => {
    const ms = ev.payload?.suppress_blur_ms ?? 1200;
    menuSuppressBlurUntil = Date.now() + ms;
    void loadBubbleEnabled();
    void refreshCompanionEngine().then(() => {
      syncHitAreasToggleUI();
      // 会话态：每次打开菜单把开关态再同步给桌宠前端
      if (companionEngine === "kanmusu") {
        void invoke("pet_set_hit_areas_visible", { visible: hitAreasVisible }).catch(
          () => undefined,
        );
      }
    });
    resetMenuView();
    scheduleRefreshPetMenuPickers();
    cancelAutoClose();
    scheduleAutoClose();
    window.setTimeout(() => {
      void invoke("pet_menu_sync_z_order");
    }, 50);
  });

  void listen("pet-menu-refresh-pickers", () => {
    void refreshPetMenuPickers();
  });

  void listen("pet-app-exiting", () => {
    cancelAutoClose();
  });

  void listen<boolean>("pet-bubble-enabled-changed", (ev) => {
    if (typeof ev.payload !== "boolean") return;
    bubbleEnabled = ev.payload;
    syncBubbleToggleUI();
  });

  void getCurrentWindow().listen("tauri://blur", () => {
    if (Date.now() < menuSuppressBlurUntil) return;
    void hideMenu();
  });
});
