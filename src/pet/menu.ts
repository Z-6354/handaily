import "./menu.css";
import { tauriInvoke as invoke, waitForTauriInternals } from "../lib/tauriInvoke";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { listen } from "@tauri-apps/api/event";

interface CharacterSkinInfo {
  id: string;
  name: string;
  model_id: string;
  model_name: string;
  active: boolean;
  model_ready: boolean;
}

interface CharacterInfo {
  id: string;
  name: string;
  active: boolean;
  skins: CharacterSkinInfo[];
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
    <div class="pet-menu-head">桌宠菜单</div>
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
    <div class="pet-menu-body">
      <div class="pet-menu-sublist" data-menu-list="skins"></div>
    </div>
  </div>
`;

const menuSkinsEl = root.querySelector('[data-menu-list="skins"]');
const menuCharactersEl = root.querySelector('[data-menu-list="characters"]');
const menuMainViewEl = root.querySelector('[data-menu-view="main"]');
const menuCharactersViewEl = root.querySelector('[data-menu-view="characters"]');
const menuSkinsViewEl = root.querySelector('[data-menu-view="skins"]');
const menuSkinsTitleEl = root.querySelector("[data-menu-skins-title]");
const menuBubbleSwitchEl = root.querySelector<HTMLButtonElement>('[data-action="toggle-bubble"]');

let menuAutoCloseTimer: ReturnType<typeof setTimeout> | null = null;
let menuSwitchBusy = false;
let menuSkinCharacterId: string | null = null;
let menuSuppressBlurUntil = 0;
let bubbleEnabled = true;
let menuPickerRefreshTimer: ReturnType<typeof setTimeout> | null = null;

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
  try {
    bubbleEnabled = await invoke<boolean>("pet_get_bubble_enabled");
  } catch {
    bubbleEnabled = true;
  }
  syncBubbleToggleUI();
}

async function setBubbleEnabled(enabled: boolean) {
  const prev = bubbleEnabled;
  bubbleEnabled = enabled;
  syncBubbleToggleUI();
  try {
    await invoke("pet_set_bubble_enabled", { enabled });
  } catch (e) {
    bubbleEnabled = prev;
    syncBubbleToggleUI();
    showMenuError(e);
  }
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
  setMenuView("main");
}

function renderMenuSublist(
  container: Element | null,
  items: { id: string; label: string; active: boolean; disabled?: boolean }[],
  kind: "character" | "skin",
) {
  if (!container) return;
  container.innerHTML = "";
  if (items.length === 0) {
    const empty = document.createElement("div");
    empty.className = "pet-menu-subempty";
    empty.textContent =
      kind === "character" ? "当前没有可选人物" : "当前角色暂无可切换模型";
    container.appendChild(empty);
    return;
  }
  for (const item of items) {
    const btn = document.createElement("button");
    btn.type = "button";
    btn.className = `pet-menu-subitem${item.active ? " is-active" : ""}`;
    btn.dataset.action = kind === "skin" ? "switch-skin" : "switch-character";
    btn.dataset.id = item.id;
    btn.textContent = item.label;
    btn.disabled = item.active || item.disabled === true || menuSwitchBusy;
    container.appendChild(btn);
  }
}

async function refreshPetMenuPickers() {
  try {
    const [characters, favoritesRaw, skinMenu] = await Promise.all([
      invoke<CharacterInfo[]>("characters_list"),
      invoke<string | null>("settings_get", { key: FAVORITES_SETTING_KEY }),
      invoke<PetMenuSkinsPayload>("characters_pet_menu_skins"),
    ]);
    await loadBubbleEnabled();
    menuSkinCharacterId = skinMenu.character_id;
    if (menuSkinsTitleEl) {
      menuSkinsTitleEl.textContent = `选择模型 · ${skinMenu.character_name}`;
    }
    renderMenuSublist(
      menuSkinsEl,
      skinMenu.skins.map((s) => ({
        id: s.id,
        label: s.model_name || s.name,
        active: s.active,
        disabled: !s.model_ready,
      })),
      "skin",
    );
    const favoriteIds = new Set(parseFavoriteIds(favoritesRaw));
    const characterItems = [...characters].sort((a, b) => {
      const af = favoriteIds.has(a.id) ? 0 : 1;
      const bf = favoriteIds.has(b.id) ? 0 : 1;
      if (af !== bf) return af - bf;
      if (a.active !== b.active) return a.active ? -1 : 1;
      return a.name.localeCompare(b.name, "zh-CN");
    });
    renderMenuSublist(
      menuCharactersEl,
      characterItems.map((c) => ({
        id: c.id,
        label: favoriteIds.has(c.id) ? `★ ${c.name}` : c.name,
        active: c.active,
      })),
      "character",
    );
  } catch (e) {
    console.error("桌宠菜单加载选项失败", e);
    menuSkinCharacterId = null;
    renderMenuSublist(menuSkinsEl, [], "skin");
    renderMenuSublist(menuCharactersEl, [], "character");
  }
}

function scheduleAutoClose() {
  if (menuAutoCloseTimer) clearTimeout(menuAutoCloseTimer);
  menuAutoCloseTimer = setTimeout(() => {
    menuAutoCloseTimer = null;
    void invoke("pet_menu_hide");
  }, MENU_AUTO_CLOSE_MS);
}

function cancelAutoClose() {
  if (menuAutoCloseTimer) {
    clearTimeout(menuAutoCloseTimer);
    menuAutoCloseTimer = null;
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
    void setBubbleEnabled(!bubbleEnabled);
    return;
  }
  if (action === "open-characters-menu") {
    void openMenuView("characters");
    return;
  }
  if (action === "open-skins-menu") {
    void openMenuView("skins");
    return;
  }
  if (action === "menu-back") {
    setMenuView("main");
    return;
  }
  if (action === "switch-skin" || action === "switch-character") {
    const id = btn.getAttribute("data-id");
    if (!id || menuSwitchBusy || btn.disabled) return;
    menuSwitchBusy = true;
    const prevLabel = btn.textContent;
    btn.textContent = "切换中…";
    try {
      if (action === "switch-character") {
        await invoke("characters_set_active", { characterId: id });
      } else {
        const characterId = menuSkinCharacterId;
        if (!characterId) throw new Error("当前没有可切换模型的角色");
        await invoke("characters_set_skin", { characterId, skinId: id });
      }
      await hideMenu();
    } catch (err) {
      console.error("桌宠菜单切换失败", err);
      showMenuError(err);
      if (prevLabel) btn.textContent = prevLabel;
    } finally {
      menuSwitchBusy = false;
    }
    return;
  }
  if (action === "edit-bounds") {
    cancelAutoClose();
    await hideMenu();
    await invoke("pet_enter_edit_bounds");
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
      await hideMenu();
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
  resetMenuView();
  void refreshPetMenuPickers();

  void listen<PetMenuShownPayload>("pet-menu-shown", (ev) => {
    const ms = ev.payload?.suppress_blur_ms ?? 1200;
    menuSuppressBlurUntil = Date.now() + ms;
    resetMenuView();
    scheduleRefreshPetMenuPickers();
    cancelAutoClose();
    scheduleAutoClose();
    window.setTimeout(() => {
      void invoke("pet_menu_sync_z_order");
    }, 50);
  });

  void listen("pet-app-exiting", () => {
    cancelAutoClose();
  });

  void getCurrentWindow().listen("tauri://blur", () => {
    if (Date.now() < menuSuppressBlurUntil) return;
    void hideMenu();
  });
});
