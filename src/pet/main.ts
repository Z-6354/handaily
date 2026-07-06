import { getCurrentWindow } from "@tauri-apps/api/window";

import { LogicalSize, PhysicalPosition } from "@tauri-apps/api/dpi";

import { listen } from "@tauri-apps/api/event";

import { invoke } from "@tauri-apps/api/core";

import "./pet.css";

import { SpinePet, type PetAssetConfig } from "./spinePet";
import { createPetAssetResolver, preloadModelAssets, type PetAssetResolver } from "./petAssetResolver";

interface PetRemarkLine {
  text: string;
  animation?: string | null;
}

interface PetAnimationMeta {
  animations: string[];
  idle_animation?: string | null;
  click_animation?: string | null;
  boot_animation?: string | null;
  return_idle_animation?: string | null;
  drag_animation?: string | null;
  random_animations: string[];
  random_min_sec: number;
  random_max_sec: number;
  lines: PetRemarkLine[];
}



interface PetRemarkPayload {

  text: string;

  source: string;

  animation?: string | null;

}



interface PetConfigPayload {
  model_id: string;
  model_name: string;

  asset_base: string;

  config_file?: string | null;

  skel_file: string;

  atlas_file: string;

  png_file: string;

  use_file_src: boolean;

  power_mode: string;

  scale: number;

  animations: string[];

  idle_animation?: string | null;
  click_animation?: string | null;
  boot_animation?: string | null;
 return_idle_animation?: string | null;
 drag_animation?: string | null;
 random_animations: string[];
 random_min_sec: number;
 random_max_sec: number;
 lines: PetRemarkLine[];
 window_width: number;
  window_height: number;
  offset_x: number;
  offset_y: number;
}



interface PetScreenBounds {
  left: number;
  top: number;
  right: number;
  bottom: number;
}

interface PetPoint {
  x: number;
  y: number;
}

interface PetModelInfo {
  id: string;
  name: string;
  builtin: boolean;
}

interface PersonaInfo {
  id: string;
  name: string;
  active: boolean;
}

const SCREEN_MARGIN = 8;
let screenBounds: PetScreenBounds | null = null;

async function refreshScreenBounds() {
  screenBounds = await invoke<PetScreenBounds>("pet_get_screen_bounds");
}

function clampWindowPosition(x: number, y: number, w: number, h: number): PetPoint {
  const b = screenBounds;
  if (!b) return { x: Math.round(x), y: Math.round(y) };
  const minX = b.left + SCREEN_MARGIN;
  const minY = b.top + SCREEN_MARGIN;
  const maxX = Math.max(minX, b.right - w - SCREEN_MARGIN);
  const maxY = Math.max(minY, b.bottom - h - SCREEN_MARGIN);
  return {
    x: Math.round(Math.max(minX, Math.min(maxX, x))),
    y: Math.round(Math.max(minY, Math.min(maxY, y))),
  };
}

function clampWindowBounds(
  w: number,
  h: number,
  x: number,
  y: number,
): { w: number; h: number; x: number; y: number } {
  const size = clampWindowSize(w, h);
  const pos = clampWindowPosition(x, y, size.w, size.h);
  return { w: size.w, h: size.h, x: pos.x, y: pos.y };
}



const rootMaybe = document.getElementById("pet-root");
if (!rootMaybe) throw new Error("pet-root missing");
const root: HTMLElement = rootMaybe;

const bootHint = document.createElement("div");
bootHint.className = "pet-boot-hint";
bootHint.textContent = "桌宠加载中…";
root.appendChild(bootHint);

const dragPreview = document.createElement("div");
dragPreview.className = "pet-drag-preview";
dragPreview.innerHTML = `<span class="pet-drag-preview-hint">拖动中 · 松开保存</span>`;
root.appendChild(dragPreview);

function clearBootHint() {
  bootHint.remove();
}
const stage = document.createElement("div");

stage.className = "pet-stage";



const canvasWrap = document.createElement("div");

canvasWrap.className = "pet-canvas-wrap";



const canvas = document.createElement("canvas");

canvas.className = "pet-canvas";

canvas.width = 220;

canvas.height = 280;

canvasWrap.append(canvas);



const fallback = document.createElement("img");

fallback.className = "pet-fallback";

fallback.alt = "小寒桌宠";

fallback.style.display = "none";



const bubble = document.createElement("div");

bubble.className = "pet-bubble";



const editOverlay = document.createElement("div");

editOverlay.className = "pet-edit-bounds";

editOverlay.innerHTML = `
  <div class="pet-edit-bounds-handle" data-edge="n"></div>
  <div class="pet-edit-bounds-handle" data-edge="s"></div>
  <div class="pet-edit-bounds-handle" data-edge="e"></div>
  <div class="pet-edit-bounds-handle" data-edge="w"></div>
  <div class="pet-edit-bounds-handle" data-edge="ne"></div>
  <div class="pet-edit-bounds-handle" data-edge="nw"></div>
  <div class="pet-edit-bounds-handle" data-edge="se"></div>
  <div class="pet-edit-bounds-handle" data-edge="sw"></div>
`;

const resizeHandles = editOverlay.querySelectorAll(".pet-edit-bounds-handle");



const menu = document.createElement("div");

menu.className = "pet-menu";
menu.innerHTML = `
  <div class="pet-menu-head">桌宠菜单</div>
  <div class="pet-menu-body">
    <button type="button" class="pet-menu-item" data-action="main">
      <span class="pet-menu-icon" aria-hidden="true">
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round">
          <rect x="3" y="4" width="18" height="14" rx="2" />
          <path d="M8 20h8" />
        </svg>
      </span>
      <span class="pet-menu-text">打开小寒日报</span>
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
    <div class="pet-menu-submenu" data-submenu="models">
      <button type="button" class="pet-menu-item pet-menu-item--sub" data-action="submenu" data-submenu="models">
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
      <div class="pet-menu-flyout" data-flyout="models" hidden>
        <div class="pet-menu-flyout-title">选择模型</div>
        <div class="pet-menu-sublist" data-menu-list="models"></div>
      </div>
    </div>
    <div class="pet-menu-submenu" data-submenu="personas">
      <button type="button" class="pet-menu-item pet-menu-item--sub" data-action="submenu" data-submenu="personas">
        <span class="pet-menu-icon" aria-hidden="true">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round">
            <circle cx="12" cy="8" r="4" />
            <path d="M6 20c0-3.3 2.7-6 6-6s6 2.7 6 6" />
          </svg>
        </span>
        <span class="pet-menu-text">切换性格</span>
        <span class="pet-menu-chevron" aria-hidden="true">›</span>
      </button>
      <div class="pet-menu-flyout" data-flyout="personas" hidden>
        <div class="pet-menu-flyout-title">选择性格</div>
        <div class="pet-menu-sublist" data-menu-list="personas"></div>
      </div>
    </div>
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
`;



stage.append(canvasWrap, fallback);

root.append(stage, bubble, editOverlay, menu);



let bubbleTimer: ReturnType<typeof setTimeout> | null = null;
let pendingBubble: { text: string; animation?: string | null } | null = null;
let bubbleEnabled = true;

let pet: SpinePet | null = null;
let petAssetResolver: PetAssetResolver | null = null;
let lastFallbackSrc = "/assets/pet/chaijun/chaijun.png";

let pointerDown = false;

let windowDragStarted = false;

let windowDragAnchorReady = false;

let pointerStart = { x: 0, y: 0, screenX: 0, screenY: 0, time: 0 };

let dragAnchor = { winX: 0, winY: 0, screenX: 0, screenY: 0 };

let pendingDragPos: PetPoint | null = null;

let dragPositionRaf = 0;

const CLICK_MAX_MS = 280;

const DRAG_THRESHOLD = 10;

let menuAutoCloseTimer: ReturnType<typeof setTimeout> | null = null;

let menuModelsEl: HTMLElement | null = menu.querySelector('[data-menu-list="models"]');
let menuPersonasEl: HTMLElement | null = menu.querySelector('[data-menu-list="personas"]');
let menuBubbleSwitchEl: HTMLButtonElement | null = menu.querySelector('[data-action="toggle-bubble"]');
let menuSwitchBusy = false;
let openSubmenuId: string | null = null;

const MENU_AUTO_CLOSE_MS = 8000;

let editBoundsMode = false;

let editBoundsSuppressUntil = 0;

let canvasDisplayW = 220;

let canvasDisplayH = 280;

let stageScale = 0.8;

let stageOffsetX = 0;

let stageOffsetY = 0;

let offsetDragging = false;

let offsetDragStart = { x: 0, y: 0, ox: 0, oy: 0 };

let resizeDragging = false;

type ResizeEdge = "n" | "s" | "e" | "w" | "ne" | "nw" | "se" | "sw";

let resizeEdge: ResizeEdge = "se";

let resizeStart = { x: 0, y: 0, w: 0, h: 0, posX: 0, posY: 0 };

let pendingResizeBounds: { w: number; h: number; x?: number; y?: number } | null = null;

let resizeApplySerial: Promise<void> = Promise.resolve();

let resizeRafId = 0;

let lastResizeKey = "";

const MIN_W = 160;

const MAX_W = 480;

const MIN_H = 200;

const MAX_H = 600;



function isInsideEditArea(target: EventTarget | null): boolean {

  if (!(target instanceof Node)) return false;

  return editOverlay.contains(target) || stage.contains(target);

}



function modelAssetFilenames(cfg: PetConfigPayload): string[] {
  const files = [cfg.skel_file, cfg.atlas_file, cfg.png_file];
  if (cfg.config_file) files.push(cfg.config_file);
  return files.filter(Boolean);
}

function assetConfigFromPayload(cfg: PetConfigPayload): PetAssetConfig {
  const base = cfg.asset_base.endsWith("/") ? cfg.asset_base : `${cfg.asset_base}/`;
  return {
    pathPrefix: base,
    configFile: cfg.config_file ?? null,
    skelFile: cfg.skel_file,
    atlasFile: cfg.atlas_file,
    pngFile: cfg.png_file,
  };
}



function positionBubble() {
  if (!root) return;
  const pad = 6;
  const gap = 8;
  const rootRect = root.getBoundingClientRect();
  const stageRect = stage.getBoundingClientRect();
  const bubbleH = bubble.offsetHeight;
  const bubbleW = bubble.offsetWidth;

  // 角色在底部居中，气泡优先放在角色可视区域上方
  const charTop = stageRect.top - rootRect.top + stageRect.height * 0.22;
  let top = Math.max(pad, charTop - bubbleH - gap);
  let left = pad;

  // 避免超出窗口右侧
  const maxLeft = rootRect.width - bubbleW - pad;
  if (left > maxLeft) left = Math.max(pad, maxLeft);

  // 仍与角色重叠时改到右上角
  const charCenterX = stageRect.left - rootRect.left + stageRect.width / 2;
  const bubbleRight = left + bubbleW;
  const bubbleBottom = top + bubbleH;
  const overlapsChar =
    bubbleBottom > charTop &&
    bubbleRight > charCenterX - stageRect.width * 0.35 &&
    left < charCenterX + stageRect.width * 0.35;

  if (overlapsChar) {
    top = pad;
    left = Math.max(pad, rootRect.width - bubbleW - pad);
  }

  bubble.style.top = `${top}px`;
  bubble.style.left = `${left}px`;
}

let petLines: PetRemarkLine[] = [];
let pendingPreview: { animation: string; loop: boolean } | null = null;

function runPreviewAnimation(animation: string, loopAnim: boolean) {
  if (!pet || !animation) {
    pendingPreview = { animation, loop: loopAnim };
    return;
  }
  pendingPreview = null;
  pet.previewPlay(animation, loopAnim);
}

function pickLineForAnimation(lines: PetRemarkLine[], animation?: string | null): string | null {
  if (lines.length === 0) return null;
  const pool = lines.filter((line) => {
    if (!animation) return !line.animation;
    return line.animation === animation || !line.animation;
  });
  const use = pool.length > 0 ? pool : lines;
  return use[Math.floor(Math.random() * use.length)]?.text ?? null;
}

function clearBubble() {
  bubble.classList.remove("visible");
  if (bubbleTimer) {
    clearTimeout(bubbleTimer);
    bubbleTimer = null;
  }
  pendingBubble = null;
}

function showBubble(text: string, animation?: string | null) {
  if (!bubbleEnabled) return;
  if (document.hidden) {
    pendingBubble = { text, animation };
    return;
  }

  bubble.textContent = text;
  bubble.classList.remove("visible");
  void bubble.offsetWidth;
  bubble.classList.add("visible");
  positionBubble();

  if (bubbleTimer) clearTimeout(bubbleTimer);
  bubbleTimer = setTimeout(() => bubble.classList.remove("visible"), 8000);

  if (animation && pet) {
    pet.playAnimation(animation, false);
  }
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
  bubbleEnabled = enabled;
  try {
    await invoke("pet_set_bubble_enabled", { enabled });
  } catch (e) {
    console.error("保存气泡开关失败", e);
    showPetLoadError(e);
    return;
  }
  syncBubbleToggleUI();
  if (!enabled) clearBubble();
}

function closeAllSubmenus() {
  openSubmenuId = null;
  menu.querySelectorAll(".pet-menu-submenu.is-open").forEach((el) => {
    el.classList.remove("is-open");
  });
  menu.querySelectorAll(".pet-menu-flyout").forEach((el) => {
    (el as HTMLElement).hidden = true;
  });
}

function positionSubmenuFlyout(submenuId: string) {
  const wrap = menu.querySelector(`.pet-menu-submenu[data-submenu="${submenuId}"]`);
  const flyout = wrap?.querySelector(".pet-menu-flyout") as HTMLElement | null;
  if (!wrap || !flyout) return;
  flyout.classList.remove("pet-menu-flyout--left");
  const menuRect = menu.getBoundingClientRect();
  const flyoutW = flyout.offsetWidth || 168;
  if (menuRect.right + flyoutW > window.innerWidth - 8) {
    flyout.classList.add("pet-menu-flyout--left");
  }
}

function toggleSubmenu(submenuId: string | null) {
  if (openSubmenuId === submenuId) {
    closeAllSubmenus();
    return;
  }
  closeAllSubmenus();
  if (!submenuId) return;
  const wrap = menu.querySelector(`.pet-menu-submenu[data-submenu="${submenuId}"]`);
  const flyout = wrap?.querySelector(".pet-menu-flyout") as HTMLElement | null;
  if (!wrap || !flyout) return;
  openSubmenuId = submenuId;
  wrap.classList.add("is-open");
  flyout.hidden = false;
  positionSubmenuFlyout(submenuId);
}

async function loadConfig(): Promise<PetConfigPayload> {

  return invoke<PetConfigPayload>("pet_get_config");

}



async function setWindowSizeOnly(w: number, h: number) {

  await getCurrentWindow().setSize(new LogicalSize(w, h));

}



async function setWindowBoundsOnly(w: number, h: number, x: number, y: number) {
  const win = getCurrentWindow();
  const b = clampWindowBounds(w, h, x, y);
  await win.setSize(new LogicalSize(b.w, b.h));
  await win.setPosition(new PhysicalPosition(b.x, b.y));
}



async function applyWindowSize(w: number, h: number, refitCanvas = false) {

  await setWindowSizeOnly(w, h);

  syncCanvasToWindow(w, h, refitCanvas);

}



function syncCanvasToWindow(w: number, h: number, refitCanvas = false) {

  canvasDisplayW = Math.round(w);

  canvasDisplayH = Math.round(h);

  applyCanvasDisplaySize();

  if (pet) {

    pet.resizeCanvas(w, h, refitCanvas);

  } else {

    ensureCanvasAttached();

    canvas.width = canvasDisplayW;

    canvas.height = canvasDisplayH;

  }

}



function applyCanvasDisplaySize() {

  canvasWrap.style.width = `${canvasDisplayW}px`;

  canvasWrap.style.height = `${canvasDisplayH}px`;

}



function suppressEditBoundsExit(ms = 400) {

  editBoundsSuppressUntil = Date.now() + ms;

}



function lockCanvasDisplaySize() {

  applyCanvasDisplaySize();

}



function unlockCanvasDisplaySize() {

  applyCanvasDisplaySize();

}



function resetEditOverlayLayout() {

  editOverlay.style.inset = "";

  editOverlay.style.width = "";

  editOverlay.style.height = "";

  editOverlay.style.left = "";

  editOverlay.style.right = "";

  editOverlay.style.top = "";

  editOverlay.style.bottom = "";

}



function resizeBoundsKey(bounds: { w: number; h: number; x?: number; y?: number }) {

  return `${Math.round(bounds.w)}x${Math.round(bounds.h)}@${bounds.x ?? ""},${bounds.y ?? ""}`;

}



function scheduleEditResize(bounds: { w: number; h: number; x?: number; y?: number }) {

  pendingResizeBounds = bounds;

  if (resizeRafId) return;

  resizeRafId = requestAnimationFrame(() => {

    resizeRafId = 0;

    const next = pendingResizeBounds;

    if (!next || !resizeDragging) return;

    const key = resizeBoundsKey(next);

    if (key === lastResizeKey) return;

    lastResizeKey = key;

    resizeApplySerial = resizeApplySerial

      .then(async () => {

        if (next.x !== undefined && next.y !== undefined) {

          await setWindowBoundsOnly(next.w, next.h, next.x, next.y);

        } else {

          await setWindowSizeOnly(next.w, next.h);

        }

      })

      .catch(() => {});

  });

}



async function commitEditResize(bounds: { w: number; h: number; x?: number; y?: number }) {

  await resizeApplySerial;

  if (bounds.x !== undefined && bounds.y !== undefined) {

    await setWindowBoundsOnly(bounds.w, bounds.h, bounds.x, bounds.y);

    await savePosition();

  } else {

    await setWindowSizeOnly(bounds.w, bounds.h);

  }

  unlockCanvasDisplaySize();

  applyStageTransform();

}



function clampWindowSize(w: number, h: number) {

  return {

    w: Math.max(MIN_W, Math.min(MAX_W, w)),

    h: Math.max(MIN_H, Math.min(MAX_H, h)),

  };

}



function computeResizeBounds(mx: number, my: number) {

  const dx = mx - resizeStart.x;

  const dy = my - resizeStart.y;

  let w = resizeStart.w;

  let h = resizeStart.h;

  let x = resizeStart.posX;

  let y = resizeStart.posY;

  const edge = resizeEdge;

  if (edge.includes("e")) {

    w = resizeStart.w + dx;

  }

  if (edge.includes("w")) {

    w = resizeStart.w - dx;

    x = resizeStart.posX + dx;

  }

  if (edge.includes("s")) {

    h = resizeStart.h + dy;

  }

  if (edge.includes("n")) {

    h = resizeStart.h - dy;

    y = resizeStart.posY + dy;

  }

  const clamped = clampWindowSize(w, h);

  if (edge.includes("w")) {

    x = resizeStart.posX + (resizeStart.w - clamped.w);

  }

  if (edge.includes("n")) {

    y = resizeStart.posY + (resizeStart.h - clamped.h);

  }

  return clampWindowBounds(clamped.w, clamped.h, x, y);

}



async function syncAnimations(
  modelId: string,
  names: string[],
  idle?: string | null,
): Promise<PetAnimationMeta | null> {
  if (names.length === 0) return null;
  try {
    return await invoke<PetAnimationMeta>("pet_sync_animations", {
      payload: {
        model_id: modelId,
        animations: names,
        idle_animation: idle ?? null,
      },
    });
  } catch {
    return null;
  }
}



function clampStageOffset() {

  const maxX = Math.max(60, Math.round(canvasDisplayW * 0.45));

  const maxY = Math.max(48, Math.round(canvasDisplayH * 0.45));

  stageOffsetX = Math.max(-maxX, Math.min(maxX, stageOffsetX));

  stageOffsetY = Math.max(-maxY, Math.min(maxY, stageOffsetY));

}



function ensureCanvasAttached() {

  if (!canvasWrap.contains(canvas)) {

    canvasWrap.append(canvas);

  }

}



function applyStageTransform() {

  stage.style.transformOrigin = "bottom center";

  stage.style.transform = `translate(${stageOffsetX}px, ${stageOffsetY}px) scale(${stageScale})`;

}



async function waitUntilVisibleForLoad(): Promise<void> {
  if (!document.hidden) {
    await new Promise<void>((resolve) => {
      requestAnimationFrame(() => requestAnimationFrame(() => resolve()));
    });
    return;
  }
  await new Promise<void>((resolve) => {
    const done = () => {
      document.removeEventListener("visibilitychange", onVis);
      resolve();
    };
    const onVis = () => {
      if (!document.hidden) {
        void new Promise<void>((r) => {
          requestAnimationFrame(() => requestAnimationFrame(() => r()));
        }).then(done);
      }
    };
    document.addEventListener("visibilitychange", onVis);
  });
}

function awaitAnimationFrames(count: number): Promise<void> {
  return new Promise((resolve) => {
    const step = (left: number) => {
      if (left <= 0) {
        resolve();
        return;
      }
      requestAnimationFrame(() => step(left - 1));
    };
    step(count);
  });
}

async function applyStageScale(scale: number) {

  stageScale = Math.max(0.4, Math.min(1.5, scale));

  applyStageTransform();

}



function applyStageOffset(x: number, y: number) {

  stageOffsetX = Math.round(x);

  stageOffsetY = Math.round(y);

  applyStageTransform();

}



async function initSpine(cfg: PetConfigPayload, opts?: { skipBoot?: boolean }): Promise<boolean> {

  const skipBoot = opts?.skipBoot ?? false;

  stageScale = cfg.scale || 0.8;

  stageOffsetX = cfg.offset_x ?? 0;

  stageOffsetY = cfg.offset_y ?? 0;

  clampStageOffset();

  await preloadModelAssets(cfg.model_id, modelAssetFilenames(cfg), cfg.use_file_src);

  petAssetResolver?.dispose();
  petAssetResolver = createPetAssetResolver(cfg);
  const assets = assetConfigFromPayload(cfg);
  lastFallbackSrc = await petAssetResolver.urlFor(cfg.png_file);
  fallback.src = lastFallbackSrc;

  const nextW = cfg.window_width || 240;
  const nextH = cfg.window_height || 320;
  if (nextW !== canvasDisplayW || nextH !== canvasDisplayH) {
    await applyWindowSize(nextW, nextH);
  }

  canvasWrap.style.visibility = "hidden";
  pet?.dispose();
  pet = null;
  await awaitAnimationFrames(3);
  ensureCanvasAttached();

  await waitUntilVisibleForLoad();

  const animOptions = {
    idleAnimation: cfg.idle_animation,
    clickAnimation: cfg.click_animation,
    bootAnimation: cfg.boot_animation,
   returnIdleAnimation: cfg.return_idle_animation,
   dragAnimation: cfg.drag_animation,
   randomAnimations: cfg.random_animations ?? [],
    randomMinSec: cfg.random_min_sec ?? 30,
    randomMaxSec: cfg.random_max_sec ?? 120,
    onRandomAction: (name: string) => {
      const text = pickLineForAnimation(petLines, name);
      if (text) showBubble(text, name);
    },
  };
  petLines = cfg.lines ?? [];



  try {

    canvasWrap.style.display = "block";

    fallback.style.display = "none";

    pet = new SpinePet(canvas, assets, {
      resolveAssetUrl: petAssetResolver.urlFor,
      skipBootAnimation: skipBoot,
      ...animOptions,
      onTap: (animation) => {
        if (!animation) return;
        const text = pickLineForAnimation(petLines, animation);
        if (text) showBubble(text, animation);
      },
    });

    const names = await pet.start();

   pet.resizeCanvas(canvasDisplayW, canvasDisplayH, true);

    const meta = await syncAnimations(cfg.model_id, names, cfg.idle_animation);

    pet.configureAnimations({
      idleAnimation: meta?.idle_animation ?? cfg.idle_animation,
      clickAnimation: meta?.click_animation ?? cfg.click_animation,
      bootAnimation: meta?.boot_animation ?? cfg.boot_animation,
     returnIdleAnimation: meta?.return_idle_animation ?? cfg.return_idle_animation,
     dragAnimation: meta?.drag_animation ?? cfg.drag_animation,
     randomAnimations: meta?.random_animations ?? cfg.random_animations ?? [],
     randomMinSec: meta?.random_min_sec ?? cfg.random_min_sec ?? 30,
     randomMaxSec: meta?.random_max_sec ?? cfg.random_max_sec ?? 120,
  }, { soft: true });
   petLines = meta?.lines ?? cfg.lines ?? [];

    await applyStageScale(stageScale);

    clampStageOffset();

    applyStageTransform();

    applyCanvasDisplaySize();

    if (pendingPreview) {
      const p = pendingPreview;
      pendingPreview = null;
      runPreviewAnimation(p.animation, p.loop);
    }

    canvasWrap.style.visibility = "visible";
    return true;

  } catch {
    // Spine 不可用时由静态图兜底
    canvasWrap.style.visibility = "visible";
    canvasWrap.style.display = "none";

    fallback.style.display = "block";

    applyStageTransform();
    return false;

  }

}



let reloadSerial: Promise<void> = Promise.resolve();
let reloadInProgress = false;

function showPetLoadError(err: unknown) {
  let banner = document.getElementById("pet-load-error");
  if (!banner) {
    banner = document.createElement("div");
    banner.id = "pet-load-error";
    banner.className = "pet-load-error";
    root.appendChild(banner);
  }
  const msg = err instanceof Error ? err.message : String(err);
  banner.innerHTML = "";
  const text = document.createElement("span");
  text.className = "pet-load-error-text";
  text.textContent = `桌宠提示：${msg}`;
  const closeBtn = document.createElement("button");
  closeBtn.type = "button";
  closeBtn.className = "pet-load-error-close";
  closeBtn.setAttribute("aria-label", "关闭");
  closeBtn.textContent = "×";
  closeBtn.addEventListener("click", (e) => {
    e.stopPropagation();
    banner?.remove();
  });
  banner.append(text, closeBtn);
}

async function resumePetFromHidden() {
  if (reloadInProgress) return;
  await waitUntilVisibleForLoad();
  if (!pet) {
    void reloadPet();
    return;
  }
  pet.setRenderPaused(false);
  pet.resizeCanvas(canvasDisplayW, canvasDisplayH, true);
  applyStageTransform();
}

async function reloadPet() {

  reloadSerial = reloadSerial.then(async () => {
    reloadInProgress = true;
    const skipBoot = pet !== null;
    try {
      await invoke("pet_clear_spine_ready");

      await waitUntilVisibleForLoad();

      const cfg = await loadConfig();

      await refreshScreenBounds();

      await initSpine(cfg, { skipBoot });

      clearBootHint();
      if (pet) {
        await invoke("pet_mark_spine_ready");
      }
    } catch (e) {

      console.error("桌宠配置加载失败", e);

      fallback.src = lastFallbackSrc;

      canvasWrap.style.display = "none";

      fallback.style.display = "block";

      applyStageTransform();

      showPetLoadError(e);

      clearBootHint();
    } finally {
      reloadInProgress = false;
    }

  });

  await reloadSerial;

}

// 尽早注册，避免 Rust on_page_load 发出的 pet-reload 在监听器就绪前丢失
const petReloadUnlistenPromise = listen("pet-reload", () => {
  void reloadPet();
});

const petResumeUnlistenPromise = listen("pet-resume", () => {
  void resumePetFromHidden();
});

async function refreshPetAnimations() {
  if (!pet) return;
  try {
    const cfg = await loadConfig();
    pet.configureAnimations({
      idleAnimation: cfg.idle_animation,
      clickAnimation: cfg.click_animation,
      bootAnimation: cfg.boot_animation,
     returnIdleAnimation: cfg.return_idle_animation,
     dragAnimation: cfg.drag_animation,
     randomAnimations: cfg.random_animations ?? [],
     randomMinSec: cfg.random_min_sec ?? 30,
     randomMaxSec: cfg.random_max_sec ?? 120,
 }, { soft: true });
   petLines = cfg.lines ?? [];
 } catch {
   // 刷新失败时保留当前配置
  }
}



async function savePosition() {
  try {
    const win = getCurrentWindow();
    const pos = await win.outerPosition();
    const saved = await invoke<PetPoint>("pet_save_position", { x: pos.x, y: pos.y });
    if (saved.x !== pos.x || saved.y !== pos.y) {
      await win.setPosition(new PhysicalPosition(saved.x, saved.y));
    }
  } catch {
    // ignore
  }
}

function setWindowDragPreview(active: boolean) {
  root.classList.toggle("pet-window-dragging", active);
  dragPreview.classList.toggle("visible", active);
}

async function beginWindowDrag(screenX: number, screenY: number) {
  if (windowDragStarted) return;
  windowDragStarted = true;
  try {
    const pos = await getCurrentWindow().outerPosition();
    dragAnchor = { winX: pos.x, winY: pos.y, screenX, screenY };
    windowDragAnchorReady = true;
   setWindowDragPreview(true);
   pet?.playDrag();
 } catch {
   windowDragStarted = false;
   windowDragAnchorReady = false;
  }
}

function scheduleDragPosition(x: number, y: number) {
  pendingDragPos = { x, y };
  if (dragPositionRaf) return;
  dragPositionRaf = requestAnimationFrame(() => {
    dragPositionRaf = 0;
    const next = pendingDragPos;
    if (!next || !windowDragStarted) return;
    pendingDragPos = null;
    void getCurrentWindow().setPosition(new PhysicalPosition(next.x, next.y));
  });
}

function endWindowDrag() {
  if (dragPositionRaf) {
    cancelAnimationFrame(dragPositionRaf);
    dragPositionRaf = 0;
  }
  pendingDragPos = null;
  windowDragAnchorReady = false;
  pet?.stopDrag();
  setWindowDragPreview(false);
}



function positionMenu(clientX: number, clientY: number) {
  menu.style.left = `${clientX}px`;
  menu.style.top = `${clientY}px`;
  requestAnimationFrame(() => {
    const rect = menu.getBoundingClientRect();
    const pad = 8;
    let x = clientX;
    let y = clientY;
    if (rect.right > window.innerWidth - pad) {
      x -= rect.right - window.innerWidth + pad;
    }
    if (rect.bottom > window.innerHeight - pad) {
      y -= rect.bottom - window.innerHeight + pad;
    }
    if (x < pad) x = pad;
    if (y < pad) y = pad;
    menu.style.left = `${x}px`;
    menu.style.top = `${y}px`;
  });
}

function renderMenuSublist(
  container: HTMLElement | null,
  items: { id: string; label: string; active: boolean }[],
  kind: "model" | "persona",
) {
  if (!container) return;
  container.innerHTML = "";
  if (items.length === 0) {
    const empty = document.createElement("div");
    empty.className = "pet-menu-subempty";
    empty.textContent = "暂无选项";
    container.appendChild(empty);
    return;
  }
  for (const item of items) {
    const btn = document.createElement("button");
    btn.type = "button";
    btn.className = `pet-menu-subitem${item.active ? " is-active" : ""}`;
    btn.dataset.action = kind === "model" ? "switch-model" : "switch-persona";
    btn.dataset.id = item.id;
    btn.textContent = item.label;
    btn.disabled = item.active || menuSwitchBusy;
    container.appendChild(btn);
  }
}

async function refreshPetMenuPickers() {
  try {
    const [models, personas, cfg] = await Promise.all([
      invoke<PetModelInfo[]>("pet_list_models"),
      invoke<PersonaInfo[]>("persona_list"),
      loadConfig(),
    ]);
    await loadBubbleEnabled();
    renderMenuSublist(
      menuModelsEl,
      models.map((m) => ({
        id: m.id,
        label: m.name,
        active: m.id === cfg.model_id,
      })),
      "model",
    );
    renderMenuSublist(
      menuPersonasEl,
      personas.map((p) => ({
        id: p.id,
        label: p.name,
        active: p.active,
      })),
      "persona",
    );
  } catch (e) {
    console.error("桌宠菜单加载选项失败", e);
    renderMenuSublist(menuModelsEl, [], "model");
    renderMenuSublist(menuPersonasEl, [], "persona");
  }
}

function toggleMenu(open?: boolean, clientX?: number, clientY?: number) {
  const next = open ?? !menu.classList.contains("open");
  menu.classList.toggle("open", next);
  if (!next) closeAllSubmenus();
  if (menuAutoCloseTimer) {
    clearTimeout(menuAutoCloseTimer);
    menuAutoCloseTimer = null;
  }
  if (next) {
    positionMenu(
      clientX ?? Math.round(window.innerWidth * 0.5),
      clientY ?? Math.round(window.innerHeight * 0.4),
    );
    void refreshPetMenuPickers();
    menuAutoCloseTimer = setTimeout(() => {
      menu.classList.remove("open");
      menuAutoCloseTimer = null;
    }, MENU_AUTO_CLOSE_MS);
  }
}

function setEditBoundsMode(on: boolean) {

  editBoundsMode = on;

  editOverlay.classList.toggle("active", on);

  stage.classList.toggle("edit-bounds-active", on);

  const editBtn = menu.querySelector('[data-action="edit-bounds"]');

  if (editBtn) {
    editBtn.classList.toggle("pet-menu-item--active", on);
    const label = editBtn.querySelector(".pet-menu-text");
    if (label) {
      label.textContent = on ? "完成编辑" : "编辑范围";
    }
  }

  if (!on) {

    offsetDragging = false;

    resizeDragging = false;

    pendingResizeBounds = null;

    lastResizeKey = "";

    if (resizeRafId) {

      cancelAnimationFrame(resizeRafId);

      resizeRafId = 0;

    }

    unlockCanvasDisplaySize();

    resetEditOverlayLayout();

  }

}



async function enterEditBounds() {

  suppressEditBoundsExit();

  menu.classList.remove("open");

  resetEditOverlayLayout();

  setEditBoundsMode(true);

  pet?.setRenderPaused(false);

  applyCanvasDisplaySize();

}



async function exitEditBounds() {

  if (!editBoundsMode) return;

  suppressEditBoundsExit();

  setEditBoundsMode(false);

  menu.classList.remove("open");

  try {

    const win = getCurrentWindow();

    const size = await win.innerSize();

    const scale = await win.scaleFactor();

    const w = size.width / scale;

    const h = size.height / scale;

    await invoke("pet_save_layout", {

      width: w,

      height: h,

      scale: stageScale,

      offsetX: stageOffsetX,

      offsetY: stageOffsetY,

    });

    resetEditOverlayLayout();

    applyStageTransform();

    applyCanvasDisplaySize();

    await savePosition();

  } catch {
    // 保存失败时忽略，下次拖动会重试
  }

}



resizeHandles.forEach((handle) => {

  handle.addEventListener("mousedown", async (e: Event) => {

    const me = e as MouseEvent;

    if (!editBoundsMode || me.button !== 0) return;

    e.preventDefault();

    e.stopPropagation();

    const edge = handle.getAttribute("data-edge") as ResizeEdge | null;

    if (!edge) return;

    resizeEdge = edge;

    resizeDragging = true;

    pendingResizeBounds = null;

    lastResizeKey = "";

    lockCanvasDisplaySize();

    pet?.setRenderPaused(true);

    const win = getCurrentWindow();

    const [size, sf, pos] = await Promise.all([

      win.innerSize(),

      win.scaleFactor(),

      win.outerPosition(),

    ]);

    resizeStart = {

      x: me.screenX,

      y: me.screenY,

      w: size.width / sf,

      h: size.height / sf,

      posX: pos.x,

      posY: pos.y,

    };

  });

});



window.addEventListener("mousemove", (e: Event) => {
  const me = e as MouseEvent;

  if (pointerDown && !windowDragStarted && !editBoundsMode) {

    const dx = me.clientX - pointerStart.x;

    const dy = me.clientY - pointerStart.y;

    if (Math.hypot(dx, dy) >= DRAG_THRESHOLD) {

      void beginWindowDrag(pointerStart.screenX, pointerStart.screenY);

    }

  }

  if (windowDragStarted && windowDragAnchorReady && !editBoundsMode) {

    const dx = me.screenX - dragAnchor.screenX;

    const dy = me.screenY - dragAnchor.screenY;

    const clamped = clampWindowPosition(
      dragAnchor.winX + dx,
      dragAnchor.winY + dy,
      canvasDisplayW,
      canvasDisplayH,
    );

    scheduleDragPosition(clamped.x, clamped.y);

  }

  if (resizeDragging) {

    const bounds = computeResizeBounds(me.screenX, me.screenY);

    pendingResizeBounds =

      resizeEdge.includes("n") || resizeEdge.includes("w")

        ? { w: bounds.w, h: bounds.h, x: bounds.x, y: bounds.y }

        : { w: bounds.w, h: bounds.h };

    scheduleEditResize(pendingResizeBounds);

    return;

  }

  if (offsetDragging) {

    const dx = me.clientX - offsetDragStart.x;

    const dy = me.clientY - offsetDragStart.y;

    applyStageOffset(

      offsetDragStart.ox + dx,

      offsetDragStart.oy + dy,

    );

  }

});



window.addEventListener("mouseup", (e: Event) => {
  const me = e as MouseEvent;

  if (pointerDown && !editBoundsMode && me.button === 0) {

    pointerDown = false;

    const elapsed = Date.now() - pointerStart.time;

    const dx = me.clientX - pointerStart.x;

    const dy = me.clientY - pointerStart.y;

    const dist = Math.hypot(dx, dy);

    const isClick = elapsed <= CLICK_MAX_MS && dist < DRAG_THRESHOLD;

    if (isClick) {

      if (windowDragStarted) {

        endWindowDrag();

      }

      pet?.handleClick();

    } else if (windowDragStarted) {

      endWindowDrag();

      void savePosition();

    }

    windowDragStarted = false;

    windowDragAnchorReady = false;

  }

  if (resizeDragging && me.button === 0) {

    resizeDragging = false;

    pet?.setRenderPaused(false);

    const bounds = pendingResizeBounds;

    pendingResizeBounds = null;

    if (bounds) {

      void commitEditResize(bounds);

    } else {

      unlockCanvasDisplaySize();

      resetEditOverlayLayout();

    }

  }

  offsetDragging = false;

});



window.addEventListener(

  "wheel",

  (e) => {

    if (!editBoundsMode) return;

    e.preventDefault();

    const delta = e.deltaY > 0 ? -0.05 : 0.05;

    void applyStageScale(stageScale + delta);

  },

  { passive: false },

);



stage.addEventListener("mousedown", (e) => {

  if (e.button !== 0) return;

  if (menu.classList.contains("open") && !menu.contains(e.target as Node)) {
    toggleMenu(false);
    return;
  }

  if (editBoundsMode) {

    e.preventDefault();

    offsetDragging = true;

    offsetDragStart = {

      x: e.clientX,

      y: e.clientY,

      ox: stageOffsetX,

      oy: stageOffsetY,

    };

    return;

  }

  pointerDown = true;

  windowDragStarted = false;

  windowDragAnchorReady = false;

  pointerStart = {

    x: e.clientX,

    y: e.clientY,

    screenX: e.screenX,

    screenY: e.screenY,

    time: Date.now(),

  };

});



stage.addEventListener("dblclick", async (e) => {

  e.preventDefault();

  pointerDown = false;

  windowDragStarted = false;

  windowDragAnchorReady = false;

  endWindowDrag();

  await invoke("pet_open_main", { page: null });

});



stage.addEventListener("contextmenu", (e) => {

  e.preventDefault();

  openPetMenu(e.clientX, e.clientY);

});



root.addEventListener("contextmenu", (e) => {

  if (e.target === stage || stage.contains(e.target as Node)) return;

  e.preventDefault();

  openPetMenu(e.clientX, e.clientY);

});



function openPetMenu(clientX?: number, clientY?: number) {

  if (menu.classList.contains("open")) {

    toggleMenu(false);

    return;

  }

  toggleMenu(true, clientX, clientY);

}



document.addEventListener("mousedown", (e) => {

  if (Date.now() < editBoundsSuppressUntil) {

    return;

  }

  if (menu.classList.contains("open") && !menu.contains(e.target as Node)) {
    toggleMenu(false);
    return;
  }

  if (editBoundsMode) {

    if (isInsideEditArea(e.target)) {

      return;

    }

    void exitEditBounds();

    return;

  }

});



document.addEventListener("click", (e) => {

  if (Date.now() < editBoundsSuppressUntil) {

    return;

  }

  if (menu.contains(e.target as Node)) {

    return;

  }

  if (editBoundsMode) {

    if (isInsideEditArea(e.target)) {

      return;

    }

    void exitEditBounds();

    return;

  }

});



void getCurrentWindow().listen("tauri://focus", () => {
  void getCurrentWindow().setAlwaysOnTop(true);
});

void getCurrentWindow().listen("tauri://blur", () => {

  if (Date.now() < editBoundsSuppressUntil) {

    return;

  }

  toggleMenu(false);

  if (editBoundsMode) {

    void exitEditBounds();

  }

});



menu.addEventListener("mouseenter", () => {

  if (menuAutoCloseTimer) {

    clearTimeout(menuAutoCloseTimer);

    menuAutoCloseTimer = null;

  }

});



menu.addEventListener("mousedown", (e) => {
  e.stopPropagation();
});

menu.addEventListener("click", async (e) => {

  e.stopPropagation();

  const btn = (e.target as HTMLElement).closest("button");

  if (!btn) return;

  const action = btn.getAttribute("data-action");

  if (action === "toggle-bubble") {
    void setBubbleEnabled(!bubbleEnabled);
    return;
  }

  if (action === "submenu") {
    const submenuId = btn.getAttribute("data-submenu");
    toggleSubmenu(submenuId);
    return;
  }

  if (action === "switch-model" || action === "switch-persona") {
    const id = btn.getAttribute("data-id");
    if (!id || menuSwitchBusy) return;
    menuSwitchBusy = true;
    toggleMenu(false);
    try {
      if (action === "switch-model") {
        await invoke("pet_set_model", { modelId: id });
      } else {
        await invoke("persona_set_active", { personaId: id });
      }
    } catch (err) {
      console.error("桌宠菜单切换失败", err);
      showPetLoadError(err);
    } finally {
      menuSwitchBusy = false;
    }
    return;
  }

  if (action === "edit-bounds") {

    if (editBoundsMode) {

      await exitEditBounds();

    } else {

      await enterEditBounds();

    }

    return;

  }

  toggleMenu(false);

  try {
    if (action === "main") {
      await invoke("pet_open_main", { page: null });
    } else if (action === "hide") {
      await invoke("pet_hide", { destroy: false });
    } else if (action === "quit") {
      await invoke("app_exit");
    }
  } catch (err) {
    console.error("桌宠菜单操作失败", err);
    showPetLoadError(err);
  }

});



document.addEventListener("visibilitychange", () => {
  if (document.hidden) {
    pet?.setRenderPaused(true);
  } else {
    if (pendingBubble) {
      const next = pendingBubble;
      pendingBubble = null;
      showBubble(next.text, next.animation);
    }
    void resumePetFromHidden();
  }
});



let petEventUnlisten: (() => void) | null = null;

async function setupPetEvents() {
  if (petEventUnlisten) return;

  const unlistenReload = await petReloadUnlistenPromise;
  const unlistenResume = await petResumeUnlistenPromise;

  const unlistenRemark = await listen<PetRemarkPayload>("pet-remark", (ev) => {
    const payload = ev.payload;
    if (!payload?.text) return;
    showBubble(payload.text, payload.animation);
  });

  const unlistenAnimations = await listen("pet-animations-changed", () => {
    void refreshPetAnimations();
  });

  const unlistenPreview = await listen<{ animation: string; loop: boolean }>(
    "pet-preview-animation",
    (ev) => {
      const { animation, loop: loopAnim } = ev.payload ?? {};
      if (!animation) return;
      runPreviewAnimation(animation, loopAnim ?? false);
    },
  );

  const unlistenContext = await listen<string>("pet-context-changed", () => {
    // debounce 由 Rust 端处理，此处仅预留
  });

  petEventUnlisten = () => {
    unlistenRemark();
    unlistenReload();
    unlistenResume();
    unlistenAnimations();
    unlistenPreview();
    unlistenContext();
    petEventUnlisten = null;
  };

  if (import.meta.hot) {
    import.meta.hot.dispose(() => petEventUnlisten?.());
  }
}

async function bootPetWindow() {
  await setupPetEvents();
  await loadBubbleEnabled();
  // 首载由 show_pet 在 pet.html 就绪后 nudge；不再用定时兜底 reload，避免与 nudge 双重重载碎块
}

void bootPetWindow();


