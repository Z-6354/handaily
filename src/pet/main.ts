import { cursorPosition, getCurrentWindow } from "@tauri-apps/api/window";

import { LogicalSize, PhysicalPosition } from "@tauri-apps/api/dpi";

import { listen } from "@tauri-apps/api/event";

import { tauriInvoke as invoke, waitForTauriInternals } from "../lib/tauriInvoke";

import "./pet.css";

import { SpinePet, type PetAssetConfig } from "./spinePet";
import {
  createPetAssetResolver,
  preloadModelAssets,
  warmModelBundleCache,
  type PetAssetResolver,
} from "./petAssetResolver";
import {
  convertStageOffsetForOrigin,
  isCursorOutsideWindow,
} from "./petLayout";
import {
  initPetMovementLog,
  petMovementLog,
  petMovementLogThrottled,
} from "./petMovementLog";

const petWindow = getCurrentWindow();

function movementLogFlags(): Record<string, unknown> {
  return {
    editBoundsMode,
    editBoundsEnterPending,
    resizeDragging,
    offsetDragging,
    resizeEdge,
    exitEditBoundsInFlight,
    ignoreCursorActive,
    petMenuOpen,
    stageOffsetX,
    stageOffsetY,
    stageScale,
    canvasDisplayW,
    canvasDisplayH,
    suppressUntil: editBoundsSuppressUntil,
  };
}

function movementCursor(me: MouseEvent): Record<string, number> {
  return {
    clientX: me.clientX,
    clientY: me.clientY,
    screenX: me.screenX,
    screenY: me.screenY,
  };
}

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
  bubble_enabled: boolean;
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

interface PetWindowBounds {
  x: number;
  y: number;
  width: number;
  height: number;
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

function showBootHint() {
  if (!bootHint.isConnected) root.appendChild(bootHint);
  bootHint.hidden = false;
}

function hideBootHint() {
  bootHint.hidden = true;
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



stage.append(canvasWrap, fallback);

root.append(stage, bubble, editOverlay);

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
let dragWindowPhysW = 220;
let dragWindowPhysH = 280;

const CLICK_MAX_MS = 280;

const DRAG_THRESHOLD = 10;

const DOUBLE_CLICK_MS = 500;

const DOUBLE_CLICK_DIST = 14;

let editBoundsMode = false;

let editBoundsEnterPending = false;

function shouldDeferClickThrough(): boolean {
  return editBoundsMode || editBoundsEnterPending;
}

function resetPointerGestureState() {
  pointerDown = false;
  windowDragStarted = false;
  windowDragAnchorReady = false;
  offsetDragging = false;
  resizeDragging = false;
  pollLeftWasDown = false;
  pollLeftDownAt = 0;
  pollLeftDownClient = { x: 0, y: 0 };
  prevRightMouseDown = false;
  prevLeftMouseDown = false;
  clickCaptureSuppressUntil = 0;
  pendingResizeBounds = null;
  endWindowDrag();
}

async function restoreNormalInteraction() {
  resetPointerGestureState();
  stopEditBoundsPoll();
  cancelEditBoundsBlurExit();
  stopClickThroughPoll();
  clickThroughApplySerial += 1;
  editBoundsEnterPending = false;
  if (document.hidden || editBoundsMode) return;
  petMovementLog("restore-interaction", movementLogFlags());
  try {
    ignoreCursorActive = true;
    await applyClickThrough(false, true);
    await syncClickThroughState();
    ensureClickThroughPoll();
    try {
      await petWindow.setFocus();
    } catch {
      // ignore
    }
  } catch (err) {
    console.error("restoreNormalInteraction failed", err);
    ignoreCursorActive = true;
    void applyClickThrough(false, true).then(() => {
      ensureClickThroughPoll();
      void syncClickThroughState();
    });
  }
}

let editBoundsSuppressUntil = 0;

let editBoundsBlurTimer: ReturnType<typeof setTimeout> | null = null;

let clickCaptureSuppressUntil = 0;

let lastStageClickAt = 0;

let lastStageClickX = 0;

let lastStageClickY = 0;

let lastMainOpenFromDoubleClickAt = 0;

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

type ResizeBoundsPayload = { w: number; h: number; x?: number; y?: number };

let pendingResizeBounds: ResizeBoundsPayload | null = null;

let resizeApplySerial: Promise<void> = Promise.resolve();

let exitEditBoundsInFlight = false;

let resizeRafId = 0;

let lastResizeKey = "";

let editResizeScaleFactor = 1;

const MIN_W = 160;

const MAX_W = 480;

const MIN_H = 200;

const MAX_H = 600;

let ignoreCursorActive = false;
let clickThroughPoll = 0;
let clickThroughInterval = 0;
const CLICK_THROUGH_POLL_MS = 80;

let prevRightMouseDown = false;
let prevLeftMouseDown = false;
let pollLeftWasDown = false;
let pollLeftDownAt = 0;
let pollLeftDownClient = { x: 0, y: 0 };
let rightClickMenuLock = false;
let editBoundsPollLeftDown = false;
/** 进入编辑后需先松开菜单点击，避免误触发 poll-outside */
let editBoundsAwaitMouseUp = false;
const EDIT_EDGE_HIT_PX = 14;
let petMenuOpen = false;
let menuCloseSuppressUntil = 0;
let cachedWinPos: { x: number; y: number } | null = null;
let lastDispatchedClickAt = 0;
let appExiting = false;

function collectInteractiveRects(): DOMRect[] {
  const rects: DOMRect[] = [];
  const add = (el: HTMLElement | null, pad = 0) => {
    if (!el) return;
    const style = getComputedStyle(el);
    if (style.display === "none" || style.visibility === "hidden") return;
    const r = el.getBoundingClientRect();
    if (r.width < 2 && r.height < 2) return;
    rects.push(new DOMRect(r.left - pad, r.top - pad, r.width + pad * 2, r.height + pad * 2));
  };
  const charRect = pet?.getCharacterScreenRect(12);
  if (charRect) {
    rects.push(charRect);
  } else {
    add(canvasWrap, 6);
    if (fallback.style.display === "block") add(fallback, 6);
  }
  if (bubble.classList.contains("visible")) add(bubble, 4);
  add(document.getElementById("pet-load-error"), 0);
  return rects;
}

function hitInteractive(clientX: number, clientY: number): boolean {
  return collectInteractiveRects().some(
    (r) => clientX >= r.left && clientX <= r.right && clientY >= r.top && clientY <= r.bottom,
  );
}

function mustCapturePointer(): boolean {
  return (
    pointerDown ||
    windowDragStarted ||
    resizeDragging ||
    offsetDragging ||
    exitEditBoundsInFlight ||
    editBoundsEnterPending ||
    shouldDeferClickThrough() ||
    Date.now() < clickCaptureSuppressUntil
  );
}

function suppressClickCapture(ms: number) {
  clickCaptureSuppressUntil = Math.max(clickCaptureSuppressUntil, Date.now() + ms);
}

function consumeStageDoubleClick(clientX: number, clientY: number): boolean {
  const now = Date.now();
  const isDouble =
    now - lastStageClickAt <= DOUBLE_CLICK_MS &&
    Math.hypot(clientX - lastStageClickX, clientY - lastStageClickY) <= DOUBLE_CLICK_DIST;
  lastStageClickAt = now;
  lastStageClickX = clientX;
  lastStageClickY = clientY;
  return isDouble;
}

async function openMainFromDoubleClick() {
  const now = Date.now();
  if (now - lastMainOpenFromDoubleClickAt < 400) return;
  lastMainOpenFromDoubleClickAt = now;
  lastDispatchedClickAt = now;
  suppressClickCapture(DOUBLE_CLICK_MS);
  pointerDown = false;
  windowDragStarted = false;
  windowDragAnchorReady = false;
  endWindowDrag();
  try {
    await invoke("pet_open_main", { page: null });
  } catch (err) {
    console.error("双击打开主窗口失败", err);
  }
}

function dispatchStageClick(clientX: number, clientY: number) {
  const now = Date.now();
  const completingDouble =
    now - lastStageClickAt <= DOUBLE_CLICK_MS &&
    Math.hypot(clientX - lastStageClickX, clientY - lastStageClickY) <= DOUBLE_CLICK_DIST;
  if (!completingDouble && now - lastDispatchedClickAt < 80) return;
  lastDispatchedClickAt = now;
  if (consumeStageDoubleClick(clientX, clientY)) {
    void openMainFromDoubleClick();
  } else {
    pet?.handleClick();
  }
}

async function trackPollLeftClickWhenIgnored(
  pointer: { x: number; y: number },
  screen: { x: number; y: number },
) {
  let down = false;
  try {
    down = await invoke<boolean>("pet_is_left_mouse_down");
  } catch {
    return;
  }

  if (!ignoreCursorActive || shouldDeferClickThrough() || petMenuOpen || rightClickMenuLock || mainWindowCovering) {
    pollLeftWasDown = down;
    return;
  }

  const edgeDown = down && !pollLeftWasDown;
  const edgeUp = !down && pollLeftWasDown;

  if (edgeDown && hitInteractive(pointer.x, pointer.y)) {
    suppressClickCapture(DOUBLE_CLICK_MS);
    pollLeftDownAt = Date.now();
    pollLeftDownClient = { x: pointer.x, y: pointer.y };
    pointerDown = true;
    windowDragStarted = false;
    windowDragAnchorReady = false;
    pointerStart = {
      x: pointer.x,
      y: pointer.y,
      screenX: screen.x,
      screenY: screen.y,
      time: pollLeftDownAt,
    };
    void applyClickThrough(false, true);
    void refreshDragWindowSize();
    void readWindowBoundsPhysical().then((bounds) => {
      cachedWinPos = { x: bounds.x, y: bounds.y };
    });
  }

  if (edgeUp && pollLeftWasDown) {
    const elapsed = Date.now() - pollLeftDownAt;
    const dist = Math.hypot(pointer.x - pollLeftDownClient.x, pointer.y - pollLeftDownClient.y);
    const isClick =
      pointerDown &&
      elapsed <= CLICK_MAX_MS &&
      dist < DRAG_THRESHOLD &&
      hitInteractive(pollLeftDownClient.x, pollLeftDownClient.y);

    if (isClick) {
      if (windowDragStarted) {
        endWindowDrag();
      } else {
        dispatchStageClick(pointer.x, pointer.y);
      }
      windowDragStarted = false;
      windowDragAnchorReady = false;
    } else if (windowDragStarted) {
      endWindowDrag();
      void savePosition();
      windowDragStarted = false;
      windowDragAnchorReady = false;
    }
    pointerDown = false;
  }

  pollLeftWasDown = down;
}

let clickThroughApplySerial = 0;

async function applyClickThrough(ignore: boolean, force = false) {
  if (ignore && mustCapturePointer()) return;
  if (!force && ignore === ignoreCursorActive) return;
  const token = ++clickThroughApplySerial;
  try {
    await petWindow.setIgnoreCursorEvents(ignore);
    if (token !== clickThroughApplySerial) return;
    ignoreCursorActive = ignore;
    petMovementLogThrottled("click-through", "click-through", {
      ignore,
      force,
      ...movementLogFlags(),
    });
  } catch (err) {
    console.error("applyClickThrough failed", err);
    petMovementLog("click-through", { ignore, force, error: String(err), ...movementLogFlags() });
  }
}

function stopClickThroughPoll() {
  if (clickThroughPoll) {
    cancelAnimationFrame(clickThroughPoll);
    clickThroughPoll = 0;
  }
  if (clickThroughInterval) {
    clearInterval(clickThroughInterval);
    clickThroughInterval = 0;
  }
}

function ensureClickThroughPoll() {
  if (document.hidden) return;
  stopClickThroughPoll();
  clickThroughInterval = window.setInterval(() => {
    if (document.hidden) return;
    void runClickThroughPollTick();
  }, CLICK_THROUGH_POLL_MS);
}

async function dismissMenuOnOutsideLeftClick() {
  if (!petMenuOpen || rightClickMenuLock) return;
  let down = false;
  let overMenu = false;
  try {
    const poll = await invoke<{ left_down: boolean; menu_contains_cursor: boolean }>(
      "pet_poll_menu_dismiss",
    );
    down = poll.left_down;
    overMenu = poll.menu_contains_cursor;
  } catch {
    return;
  }
  const edgeDown = down && !prevLeftMouseDown;
  const edgeUp = !down && prevLeftMouseDown;
  prevLeftMouseDown = down;
  // 按下时在菜单内：不处理（避免在 click 到达菜单 WebView 前误关菜单）
  if (edgeDown) return;
  // 仅在松开且光标不在菜单上时关闭
  if (!edgeUp || overMenu) return;
  try {
    await invoke("pet_menu_hide");
  } catch {
    // ignore
  }
}

async function runClickThroughPollTick() {
  if (editBoundsMode) {
    await runEditBoundsPollTick();
    return;
  }
  if (shouldDeferClickThrough() || mustCapturePointer()) return;

  if (petMenuOpen) {
    await dismissMenuOnOutsideLeftClick();
  } else {
    prevLeftMouseDown = false;
  }

  let pointer: { x: number; y: number } | undefined;
  let screen: { x: number; y: number } | undefined;
  const client = await cursorClientLogical();
  if (client) {
    pointer = { x: client.x, y: client.y };
    screen = { x: client.screen.x, y: client.screen.y };
  } else {
    return;
  }

  await syncClickThroughState(pointer);
  await trackPollLeftClickWhenIgnored(pointer, screen);

  if (petMenuOpen || rightClickMenuLock || shouldDeferClickThrough()) {
    prevRightMouseDown = false;
    return;
  }

  let rightDown = false;
  try {
    rightDown = await invoke<boolean>("pet_is_right_mouse_down");
  } catch {
    return;
  }
  const rightEdge = rightDown && !prevRightMouseDown;
  prevRightMouseDown = rightDown;
  if (!rightEdge || !hitInteractive(pointer.x, pointer.y)) return;
  await applyClickThrough(false, true);
  await openPetMenu();
}

async function syncClickThroughState(pointer?: { x: number; y: number }) {
  if (mustCapturePointer()) {
    await applyClickThrough(false, true);
    return;
  }
  let x: number;
  let y: number;
  if (pointer) {
    x = pointer.x;
    y = pointer.y;
  } else {
    const client = await cursorClientLogical();
    if (!client) return;
    x = client.x;
    y = client.y;
  }
  const ignore = !hitInteractive(x, y);
  await applyClickThrough(ignore);
}

function startClickThrough() {
  if (document.hidden || shouldDeferClickThrough()) {
    stopClickThroughPoll();
    return;
  }
  ensureClickThroughPoll();
  void syncClickThroughState();
}

function stopClickThrough() {
  stopClickThroughPoll();
  void applyClickThrough(false, true);
}

async function ensureClickThroughDisabled() {
  stopClickThroughPoll();
  clickThroughApplySerial += 1;
  ignoreCursorActive = true;
  await applyClickThrough(false, true);
}

function isPointOnEditHandle(target: EventTarget | null): boolean {
  if (!(target instanceof Element)) return false;
  return target.closest(".pet-edit-bounds-handle") != null;
}

function hitTestEditEdge(clientX: number, clientY: number): ResizeEdge | null {
  const w = root.clientWidth;
  const h = root.clientHeight;
  if (w < 1 || h < 1) return null;
  const nearL = clientX <= EDIT_EDGE_HIT_PX;
  const nearR = clientX >= w - EDIT_EDGE_HIT_PX;
  const nearT = clientY <= EDIT_EDGE_HIT_PX;
  const nearB = clientY >= h - EDIT_EDGE_HIT_PX;
  if (nearT && nearL) return "nw";
  if (nearT && nearR) return "ne";
  if (nearB && nearL) return "sw";
  if (nearB && nearR) return "se";
  if (nearT) return "n";
  if (nearB) return "s";
  if (nearL) return "w";
  if (nearR) return "e";
  return null;
}

function isInsideEditArea(target: EventTarget | null): boolean {
  if (!(target instanceof Node)) return false;
  if (isPointOnEditHandle(target)) return true;
  return editOverlay.contains(target) || stage.contains(target);
}



function modelAssetFilenames(cfg: PetConfigPayload): string[] {
  const files = [cfg.skel_file, cfg.atlas_file];
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
  if (editBoundsMode) {
    bubble.style.top = "6px";
    bubble.style.left = "6px";
    bubble.style.right = "auto";
    return;
  }
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
  bubbleTimer = setTimeout(() => {
    bubble.classList.remove("visible");
    if (!editBoundsMode) startClickThrough();
  }, 8000);

  if (animation && pet) {
    pet.playAnimation(animation, false);
  }
  if (!editBoundsMode) startClickThrough();
}



function applyBubbleEnabledFromConfig(enabled: boolean | undefined) {
  if (typeof enabled !== "boolean") return;
  bubbleEnabled = enabled;
}

async function loadBubbleEnabled() {
  try {
    bubbleEnabled = await invoke<boolean>("pet_get_bubble_enabled");
  } catch {
    bubbleEnabled = true;
  }
}

async function loadConfig(): Promise<PetConfigPayload> {

  return invoke<PetConfigPayload>("pet_get_config");

}



async function setWindowSizeOnly(w: number, h: number) {

  await getCurrentWindow().setSize(new LogicalSize(w, h));

}

function clampWindowSizePhysical(w: number, h: number, sf: number) {
  const minW = Math.round(MIN_W * sf);
  const maxW = Math.round(MAX_W * sf);
  const minH = Math.round(MIN_H * sf);
  const maxH = Math.round(MAX_H * sf);
  return {
    w: Math.max(minW, Math.min(maxW, Math.round(w))),
    h: Math.max(minH, Math.min(maxH, Math.round(h))),
  };
}

async function readWindowBoundsPhysical(): Promise<PetWindowBounds> {
  return invoke<PetWindowBounds>("pet_get_window_bounds");
}

async function readWindowBounds(): Promise<ResizeBoundsPayload> {
  const [bounds, sf] = await Promise.all([
    readWindowBoundsPhysical(),
    getCurrentWindow().scaleFactor(),
  ]);
  return {
    w: bounds.width / sf,
    h: bounds.height / sf,
    x: bounds.x,
    y: bounds.y,
  };
}

/** 屏幕光标 → 窗口内逻辑坐标（与 frame HWND 对齐，避免 outerPosition 偏差） */
async function cursorClientLogical(): Promise<{ x: number; y: number; screen: PetPoint } | null> {
  try {
    const [cursor, bounds, sf] = await Promise.all([
      cursorPosition(),
      readWindowBoundsPhysical(),
      petWindow.scaleFactor(),
    ]);
    return {
      x: (cursor.x - bounds.x) / sf,
      y: (cursor.y - bounds.y) / sf,
      screen: { x: cursor.x, y: cursor.y },
    };
  } catch {
    return null;
  }
}

async function applyPetWindowBounds(
  bounds: { w: number; h: number; x: number; y: number },
  edge: ResizeEdge,
) {
  const moveX = edge.includes("w");
  const moveY = edge.includes("n");
  const clamped = clampWindowSizePhysical(bounds.w, bounds.h, editResizeScaleFactor);
  const pos = clampWindowPosition(bounds.x, bounds.y, clamped.w, clamped.h);
  await invoke("pet_set_window_bounds", {
    x: pos.x,
    y: pos.y,
    width: clamped.w,
    height: clamped.h,
    move_x: moveX,
    move_y: moveY,
  });
}

function resizeBoundsKey(bounds: ResizeBoundsPayload) {
  return `${Math.round(bounds.w)}x${Math.round(bounds.h)}@${bounds.x ?? ""},${bounds.y ?? ""}`;
}

function applyEditResizeCanvasSync(
  logW: number,
  logH: number,
  prevW: number,
  prevH: number,
  edge: ResizeEdge,
) {
  const moveNorth = edge.includes("n");
  const moveWest = edge.includes("w");
  canvasDisplayW = logW;
  canvasDisplayH = logH;
  applyCanvasDisplaySize();
  if (pet) {
    pet.resizeCanvasForEditResize(
      logW,
      logH,
      prevW,
      prevH,
      moveNorth,
      moveWest,
      stageScale,
    );
  } else {
    ensureCanvasAttached();
    canvas.width = logW;
    canvas.height = logH;
  }
}

function scheduleEditResize(bounds: ResizeBoundsPayload) {
  pendingResizeBounds = bounds;
  if (resizeRafId) return;
  resizeRafId = requestAnimationFrame(() => {
    resizeRafId = 0;
    const next = pendingResizeBounds;
    if (!next || !resizeDragging) return;
    const key = resizeBoundsKey(next);
    if (key === lastResizeKey) return;
    lastResizeKey = key;
    const edge = resizeEdge;
    resizeApplySerial = resizeApplySerial
      .then(async () => {
        const sf = editResizeScaleFactor || (await getCurrentWindow().scaleFactor());
        const prevW = canvasDisplayW;
        const prevH = canvasDisplayH;
        const logW = Math.max(MIN_W, Math.round(next.w / sf));
        const logH = Math.max(MIN_H, Math.round(next.h / sf));

        if (next.x !== undefined && next.y !== undefined) {
          await applyPetWindowBounds(
            { w: next.w, h: next.h, x: next.x, y: next.y },
            edge,
          );
        } else {
          await setWindowSizeOnly(logW, logH);
        }

        applyEditResizeCanvasSync(logW, logH, prevW, prevH, edge);
      })
      .catch((err) => {
        console.error("edit resize apply failed", err);
      });
  });
}

async function commitEditResize(bounds: ResizeBoundsPayload, edge: ResizeEdge = resizeEdge) {
  await resizeApplySerial;
  const sf = editResizeScaleFactor || (await getCurrentWindow().scaleFactor());
  const prevW = canvasDisplayW;
  const prevH = canvasDisplayH;
  const logW = Math.max(MIN_W, Math.round(bounds.w / sf));
  const logH = Math.max(MIN_H, Math.round(bounds.h / sf));
  if (bounds.x !== undefined && bounds.y !== undefined) {
    await applyPetWindowBounds(
      { w: bounds.w, h: bounds.h, x: bounds.x, y: bounds.y },
      edge,
    );
    if (edge.includes("n") || edge.includes("w")) {
      void savePosition();
    }
  } else {
    await setWindowSizeOnly(logW, logH);
  }
  if (logW !== prevW || logH !== prevH) {
    applyEditResizeCanvasSync(logW, logH, prevW, prevH, edge);
  } else {
    syncCanvasToWindow(logW, logH, false);
  }
  applyStageTransform();
  suppressEditBoundsExit(800);
}

function computeResizeBounds(mx: number, my: number): ResizeBoundsPayload {
  const edge = resizeEdge;
  const dx = mx - resizeStart.x;
  const dy = my - resizeStart.y;
  let w = resizeStart.w;
  let h = resizeStart.h;
  let x = resizeStart.posX;
  let y = resizeStart.posY;
  if (edge.includes("e")) w = resizeStart.w + dx;
  if (edge.includes("w")) {
    w = resizeStart.w - dx;
    x = resizeStart.posX + dx;
  }
  if (edge.includes("s")) h = resizeStart.h + dy;
  if (edge.includes("n")) {
    h = resizeStart.h - dy;
    y = resizeStart.posY + dy;
  }
  const clamped = clampWindowSizePhysical(w, h, editResizeScaleFactor);
  if (edge.includes("w")) x = resizeStart.posX + (resizeStart.w - clamped.w);
  if (edge.includes("n")) y = resizeStart.posY + (resizeStart.h - clamped.h);
  const pos = clampWindowPosition(x, y, clamped.w, clamped.h);
  return {
    w: clamped.w,
    h: clamped.h,
    x: pos.x,
    y: pos.y,
  };
}

async function syncCanvasFromWindow(refitCanvas = false) {
  if (!pet) return;
  const bounds = await readWindowBounds();
  const w = Math.max(MIN_W, bounds.w);
  const h = Math.max(MIN_H, bounds.h);
  if (!Number.isFinite(w) || !Number.isFinite(h)) return;
  syncCanvasToWindow(w, h, refitCanvas);
}

async function applyWindowSize(w: number, h: number, refitCanvas = false) {

  await setWindowSizeOnly(w, h);

  syncCanvasToWindow(w, h, refitCanvas);

}



function syncCanvasToWindow(w: number, h: number, refitCanvas = false) {

  canvasDisplayW = Math.max(MIN_W, Math.round(w));

  canvasDisplayH = Math.max(MIN_H, Math.round(h));

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
  if (editBoundsMode) {
    canvasWrap.style.width = "100%";
    canvasWrap.style.height = "100%";
    return;
  }
  canvasWrap.style.width = `${canvasDisplayW}px`;
  canvasWrap.style.height = `${canvasDisplayH}px`;
}

function suppressEditBoundsExit(ms = 400) {
  editBoundsSuppressUntil = Math.max(editBoundsSuppressUntil, Date.now() + ms);
}

async function isCursorOutsidePetWindow(): Promise<boolean> {
  const [cursor, bounds] = await Promise.all([
    cursorPosition(),
    readWindowBoundsPhysical(),
  ]);
  return isCursorOutsideWindow(cursor.x, cursor.y, bounds, 12);
}

async function runEditBoundsPollTick() {
  if (!editBoundsMode) return;
  if (editBoundsEnterPending) return;

  let down = false;
  try {
    down = await invoke<boolean>("pet_is_left_mouse_down");
  } catch {
    return;
  }

  if (!down) {
    editBoundsAwaitMouseUp = false;
    if (resizeDragging && pendingResizeBounds) {
      const bounds = pendingResizeBounds;
      const edge = resizeEdge;
      pendingResizeBounds = null;
      resizeDragging = false;
      root.classList.remove("edit-bounds-resizing");
      void commitEditResize(bounds, edge);
      suppressEditBoundsExit(800);
    } else if (offsetDragging) {
      clampStageOffset();
      applyStageTransform();
      offsetDragging = false;
      resizeDragging = false;
    } else if (resizeDragging) {
      resizeDragging = false;
      root.classList.remove("edit-bounds-resizing");
    }
  }

  const edgeDown = down && !editBoundsPollLeftDown;
  editBoundsPollLeftDown = down;

  if (Date.now() < editBoundsSuppressUntil) return;
  if (Date.now() < menuCloseSuppressUntil) return;
  if (editBoundsAwaitMouseUp) return;
  if (offsetDragging || resizeDragging || exitEditBoundsInFlight) return;
  if (!edgeDown) return;

  try {
    if (await isCursorOutsidePetWindow()) {
      petMovementLog("outside-check", { outside: true, ...movementLogFlags() });
      void exitEditBounds("poll-outside");
    }
  } catch (err) {
    petMovementLog("outside-check", { outside: "error", error: String(err), ...movementLogFlags() });
  }
}

function startEditBoundsPoll() {
  editBoundsPollLeftDown = false;
  ensureClickThroughPoll();
}

function stopEditBoundsPoll() {
  editBoundsPollLeftDown = false;
}

function cancelEditBoundsBlurExit() {
  if (editBoundsBlurTimer) {
    petMovementLog("blur-exit-cancel", movementLogFlags());
    clearTimeout(editBoundsBlurTimer);
    editBoundsBlurTimer = null;
  }
}



function resetEditOverlayLayout() {
  editOverlay.style.inset = "";
  editOverlay.style.width = "";
  editOverlay.style.height = "";
  editOverlay.style.left = "";
  editOverlay.style.right = "";
  editOverlay.style.top = "";
  editOverlay.style.bottom = "";
  root.classList.remove("edit-bounds-resizing");
}

async function persistLayoutSnapshot() {
  const bounds = await readWindowBounds();
  const phys = await readWindowBoundsPhysical();
  const saveW = Math.max(MIN_W, bounds.w);
  const saveH = Math.max(MIN_H, bounds.h);
  const offset = stageOffsetForPersist();
  await invoke("pet_save_layout", {
    width: saveW,
    height: saveH,
    scale: stageScale,
    offsetX: offset.x,
    offsetY: offset.y,
    positionWinWidth: phys.width,
    positionWinHeight: phys.height,
  });
}

async function persistLayoutSnapshotSafe(context: string) {
  try {
    await persistLayoutSnapshot();
  } catch (err) {
    console.error(`layout save failed (${context})`, err);
  }
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

function releaseCanvasGlContext() {
  try {
    const gl =
      canvas.getContext("webgl2") ??
      canvas.getContext("webgl") ??
      canvas.getContext("experimental-webgl");
    if (gl && gl instanceof WebGLRenderingContext) {
      gl.getExtension("WEBGL_lose_context")?.loseContext();
    }
  } catch {
    // ignore
  }
}

function disposePetForExit() {
  if (appExiting) return;
  appExiting = true;
  stopClickThroughPoll();
  stopEditBoundsPoll();
  cancelEditBoundsBlurExit();
  clearBubble();
  if (resizeRafId) {
    cancelAnimationFrame(resizeRafId);
    resizeRafId = 0;
  }
  if (dragPositionRaf) {
    cancelAnimationFrame(dragPositionRaf);
    dragPositionRaf = 0;
  }
  pendingDragPos = null;
  pointerDown = false;
  windowDragStarted = false;
  windowDragAnchorReady = false;
  pet?.dispose();
  pet = null;
  petAssetResolver?.dispose();
  petAssetResolver = null;
  releaseCanvasGlContext();
  petEventUnlisten?.();
}

async function isPetWindowVisible(): Promise<boolean> {
  if (!document.hidden) return true;
  try {
    return await getCurrentWindow().isVisible();
  } catch {
    return false;
  }
}



function applyStageTransform() {
  stage.style.transformOrigin = editBoundsMode ? "top left" : "bottom center";
  stage.style.transform = `translate(${stageOffsetX}px, ${stageOffsetY}px) scale(${stageScale})`;
}

function stageOffsetForPersist(): { x: number; y: number } {
  if (editBoundsMode) {
    return convertStageOffsetForOrigin(
      stageOffsetX,
      stageOffsetY,
      stageScale,
      canvasDisplayW,
      canvasDisplayH,
      "top-left",
      "bottom-center",
    );
  }
  return { x: stageOffsetX, y: stageOffsetY };
}



async function waitUntilVisibleForLoad(maxWaitMs = 1200): Promise<void> {
  if (!document.hidden) {
    await awaitAnimationFrames(2);
    return;
  }
  const deadline = Date.now() + maxWaitMs;
  while (Date.now() < deadline) {
    if (await isPetWindowVisible()) {
      await awaitAnimationFrames(2);
      return;
    }
    await new Promise<void>((resolve) => window.setTimeout(resolve, 50));
  }
  await awaitAnimationFrames(2);
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
  stageScale = Math.round(Math.max(0.4, Math.min(1.5, scale)) * 100) / 100;
  clampStageOffset();
  applyStageTransform();
}



function applyStageOffset(x: number, y: number) {

  stageOffsetX = Math.round(x);

  stageOffsetY = Math.round(y);

  clampStageOffset();
  applyStageTransform();

}



async function initSpine(
  cfg: PetConfigPayload,
  opts?: { skipBoot?: boolean; skipVisibilityWait?: boolean; hotReload?: boolean; forceGlTeardown?: boolean },
): Promise<boolean> {

  const skipBoot = opts?.skipBoot ?? false;
  const hotReload = opts?.hotReload ?? false;
  const forceGlTeardown = opts?.forceGlTeardown ?? false;

  stageScale = cfg.scale || 0.8;

  stageOffsetX = cfg.offset_x ?? 0;

  stageOffsetY = cfg.offset_y ?? 0;

  clampStageOffset();

  petAssetResolver?.dispose();
  petAssetResolver = createPetAssetResolver(cfg);
  const assets = assetConfigFromPayload(cfg);
  const assetFiles = [...modelAssetFilenames(cfg), cfg.png_file].filter(Boolean);
  const hadPet = pet !== null;
  let fallbackUrl: string;
  if (cfg.use_file_src) {
    const warmPromise = warmModelBundleCache(cfg.model_id, assetFiles);
    fallbackUrl = await Promise.all([
      warmPromise,
      petAssetResolver.urlFor(cfg.png_file),
    ]).then(([, url]) => url);
  } else {
    void preloadModelAssets(cfg.model_id, assetFiles, false, cfg.asset_base);
    fallbackUrl = await petAssetResolver.urlFor(cfg.png_file);
  }
  lastFallbackSrc = fallbackUrl;
  fallback.src = fallbackUrl;

  let nextW = Math.max(MIN_W, cfg.window_width || 240);
  let nextH = Math.max(MIN_H, cfg.window_height || 320);
  if (skipBoot && !hotReload) {
    try {
      const frameBounds = await readWindowBounds();
      if (Number.isFinite(frameBounds.w) && frameBounds.w >= MIN_W) {
        nextW = Math.max(MIN_W, frameBounds.w);
      }
      if (Number.isFinite(frameBounds.h) && frameBounds.h >= MIN_H) {
        nextH = Math.max(MIN_H, frameBounds.h);
      }
    } catch {
      // 使用 DB 尺寸
    }
  } else if (hotReload) {
    nextW = Math.max(MIN_W, canvasDisplayW);
    nextH = Math.max(MIN_H, canvasDisplayH);
  }
  const sizeChanged = nextW !== canvasDisplayW || nextH !== canvasDisplayH;
  if (sizeChanged) {
    await applyWindowSize(nextW, nextH);
  }

  if (hotReload) {
    await awaitAnimationFrames(1);
    ensureCanvasAttached();
    if (canvas.width !== canvasDisplayW || canvas.height !== canvasDisplayH) {
      canvas.width = canvasDisplayW;
      canvas.height = canvasDisplayH;
    }
  } else if (!hadPet && !forceGlTeardown) {
    if (!skipBoot) showBootHint();
    canvasWrap.style.visibility = "hidden";
    fallback.style.display = "none";
    ensureCanvasAttached();
    if (canvas.width !== canvasDisplayW || canvas.height !== canvasDisplayH) {
      canvas.width = canvasDisplayW;
      canvas.height = canvasDisplayH;
    }
  } else {
    canvasWrap.style.visibility = "hidden";
    fallback.style.display = "none";
    showBootHint();
    pet?.dispose();
    pet = null;
    releaseCanvasGlContext();
    canvas.width = 0;
    canvas.height = 0;
    await awaitAnimationFrames(2);
    ensureCanvasAttached();
    canvas.width = canvasDisplayW;
    canvas.height = canvasDisplayH;
  }

  if (!opts?.skipVisibilityWait) {
    await waitUntilVisibleForLoad();
  }

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
    if (hotReload && pet) {
      pet.dispose();
      pet = null;
    }

    pet = new SpinePet(canvas, assets, {
      resolveAssetUrl: petAssetResolver.urlFor,
      readViaIpc: petAssetResolver.readViaIpc,
      skipBootAnimation: true,
      ...animOptions,
      onTap: (animation) => {
        if (!animation) return;
        const text = pickLineForAnimation(petLines, animation);
        if (text) showBubble(text, animation);
      },
    });

    const names = await pet.start();

    canvasWrap.style.display = "block";
    fallback.style.display = "none";

   pet.resizeCanvas(canvasDisplayW, canvasDisplayH, !hotReload);

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
    hideBootHint();
    clearPetLoadError();
    if (!hotReload) {
      startClickThrough();
    }

    if (names.length > 0) {
      void syncAnimations(cfg.model_id, names, cfg.idle_animation).then((meta) => {
        if (!pet) return;
        if (!meta) return;
        pet.configureAnimations({
          idleAnimation: meta.idle_animation ?? cfg.idle_animation,
          clickAnimation: meta.click_animation ?? cfg.click_animation,
          bootAnimation: meta.boot_animation ?? cfg.boot_animation,
         returnIdleAnimation: meta.return_idle_animation ?? cfg.return_idle_animation,
         dragAnimation: meta.drag_animation ?? cfg.drag_animation,
         randomAnimations: meta.random_animations ?? cfg.random_animations ?? [],
         randomMinSec: meta.random_min_sec ?? cfg.random_min_sec ?? 30,
         randomMaxSec: meta.random_max_sec ?? cfg.random_max_sec ?? 120,
        }, { soft: true });
        petLines = meta.lines ?? cfg.lines ?? petLines;
      });
    }
    return true;

  } catch (err) {
    console.error("Spine 初始化失败", err);
    pet?.dispose();
    pet = null;
    if (hotReload) {
      hideBootHint();
      return false;
    }
    releaseCanvasGlContext();
    canvasWrap.style.visibility = "visible";
    canvasWrap.style.display = "none";

    fallback.style.display = "block";
    hideBootHint();

    applyStageTransform();
    startClickThrough();
    return false;

  }

}



let reloadSerial: Promise<void> = Promise.resolve();
let reloadInProgress = false;
let reloadEverStarted = false;
let resumeInFlight = false;
let lastResumeAt = 0;
const RESUME_DEBOUNCE_MS = 250;

function clearPetLoadError() {
  document.getElementById("pet-load-error")?.remove();
}

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
  if (mainWindowCovering) return;
  if (Date.now() < suppressVisibilityUntil) return;
  if (reloadInProgress) {
    void reloadSerial.then(() => {
      if (!document.hidden && !mainWindowCovering) void resumePetFromHidden();
    });
    return;
  }
  const now = Date.now();
  if (resumeInFlight || now - lastResumeAt < RESUME_DEBOUNCE_MS) return;
  resumeInFlight = true;
  lastResumeAt = now;
  try {
    await waitUntilVisibleForLoad();
    if (!pet) {
      void reloadPet();
      return;
    }
    pet.setRenderPaused(false);
    await syncCanvasFromWindow(false);
    applyStageTransform();
  } finally {
    resumeInFlight = false;
  }
}

async function reloadPet() {

  if (appExiting) return;

  if (editBoundsMode) {
    await exitEditBounds();
  }

  reloadSerial = reloadSerial.then(async () => {
    reloadInProgress = true;
    reloadEverStarted = true;
    const skipBoot = pet !== null;
    if (!skipBoot) showBootHint();
    try {
      const [, cfg] = await Promise.all([
        refreshScreenBounds(),
        loadConfig(),
      ]);
      applyBubbleEnabledFromConfig(cfg.bubble_enabled);

      let ok = false;
      if (skipBoot) {
        ok = await initSpine(cfg, { skipBoot: true, skipVisibilityWait: true, hotReload: true });
      }
      if (!ok) {
        await awaitAnimationFrames(1);
        ok = await initSpine(cfg, {
          skipBoot: true,
          skipVisibilityWait: true,
          hotReload: false,
          forceGlTeardown: skipBoot,
        });
      }

      if (ok && pet) {
        await invoke("pet_mark_spine_ready");
        clearPetLoadError();
      } else if (!ok) {
        showPetLoadError(new Error("Spine 模型加载失败，已显示静态图"));
      }
    } catch (e) {

      console.error("桌宠配置加载失败", e);

      fallback.src = lastFallbackSrc;

      canvasWrap.style.display = "none";

      fallback.style.display = "block";

      applyStageTransform();

      showPetLoadError(e);
    } finally {
      hideBootHint();
      reloadInProgress = false;
      void restoreNormalInteraction();
    }

  });

  await reloadSerial;

}

// 尽早注册，避免 Rust on_page_load 发出的 pet-reload 在监听器就绪前丢失
let suppressVisibilityUntil = 0;
let mainWindowCovering = false;

const tauriReady = waitForTauriInternals();

void tauriReady.then(() => {
  void listen("pet-app-exiting", () => {
    disposePetForExit();
  });

  void listen<number>("pet-main-opening", (ev) => {
    mainWindowCovering = true;
    suppressVisibilityUntil = Date.now() + (ev.payload ?? 1500);
    pet?.setRenderPaused(true);
  });

  void listen("pet-main-closed", () => {
    mainWindowCovering = false;
    suppressVisibilityUntil = Date.now() + 400;
    if (pet) {
      pet.setRenderPaused(false);
      void syncCanvasFromWindow(false).then(() => {
        applyStageTransform();
        startClickThrough();
      });
      return;
    }
    void resumePetFromHidden();
  });

  void listen("pet-hidden", () => {
    mainWindowCovering = false;
    petMenuOpen = false;
    pendingBubble = null;
    stopClickThrough();
    pet?.setRenderPaused(true);
    if (editBoundsMode) {
      void abandonEditBoundsOnHidden();
    }
  });
});

const petReloadUnlistenPromise = tauriReady.then(() =>
  listen("pet-reload", () => {
    void reloadPet();
  }),
);

const petResumeUnlistenPromise = tauriReady.then(() =>
  listen("pet-resume", () => {
    void resumePetFromHidden();
  }),
);

async function refreshPetAnimations() {
  if (!pet) return;
  try {
    const cfg = await loadConfig();
    applyBubbleEnabledFromConfig(cfg.bubble_enabled);
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
    const bounds = await readWindowBoundsPhysical();
    const saved = await invoke<PetPoint>("pet_save_position", {
      x: bounds.x,
      y: bounds.y,
      win_width: bounds.width,
      win_height: bounds.height,
    });
    if (saved.x !== bounds.x || saved.y !== bounds.y) {
      await getCurrentWindow().setPosition(new PhysicalPosition(saved.x, saved.y));
    }
    cachedWinPos = { x: saved.x, y: saved.y };
  } catch (err) {
    console.error("savePosition failed", err);
  }
}

async function refreshDragWindowSize() {
  try {
    const bounds = await readWindowBoundsPhysical();
    dragWindowPhysW = bounds.width;
    dragWindowPhysH = bounds.height;
  } catch {
    // keep previous
  }
}

function beginOffsetDrag(clientX: number, clientY: number) {
  if (exitEditBoundsInFlight || editBoundsEnterPending) return;
  suppressEditBoundsExit(600);
  offsetDragging = true;
  void applyClickThrough(false, true);
  offsetDragStart = {
    x: clientX,
    y: clientY,
    ox: stageOffsetX,
    oy: stageOffsetY,
  };
  petMovementLog("offset-start", {
    ...movementLogFlags(),
    offsetDragStart,
    client: { x: clientX, y: clientY },
  });
}

function setWindowDragPreview(active: boolean) {
  root.classList.toggle("pet-window-dragging", active);
  dragPreview.classList.toggle("visible", active);
}

async function beginWindowDrag(screenX: number, screenY: number) {
  if (windowDragStarted && windowDragAnchorReady) return;
  if (!screenBounds) void refreshScreenBounds();
  windowDragStarted = true;
  void refreshDragWindowSize();
  let previewStarted = false;
  const applyAnchor = (winX: number, winY: number) => {
    dragAnchor = { winX, winY, screenX, screenY };
    windowDragAnchorReady = true;
    if (!previewStarted) {
      previewStarted = true;
      setWindowDragPreview(true);
      pet?.playDrag();
    }
  };
  if (cachedWinPos) {
    applyAnchor(cachedWinPos.x, cachedWinPos.y);
  }
  try {
    const bounds = await readWindowBoundsPhysical();
    cachedWinPos = { x: bounds.x, y: bounds.y };
    applyAnchor(bounds.x, bounds.y);
  } catch {
    if (!windowDragAnchorReady) {
      windowDragStarted = false;
      setWindowDragPreview(false);
    }
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



function setEditBoundsMode(on: boolean) {

  editBoundsMode = on;

  editOverlay.classList.toggle("active", on);

  stage.classList.toggle("edit-bounds-active", on);

  applyCanvasDisplaySize();

  if (!on) {

    offsetDragging = false;

    resizeDragging = false;
    pendingResizeBounds = null;

    if (resizeRafId) {

      cancelAnimationFrame(resizeRafId);

      resizeRafId = 0;

    }

    resetEditOverlayLayout();
    stopEditBoundsPoll();
    applyStageTransform();

    return;

  }

  stopClickThrough();
  cancelEditBoundsBlurExit();

}



async function waitForExitEditIdle(maxMs = 4000): Promise<boolean> {
  const deadline = Date.now() + maxMs;
  while (exitEditBoundsInFlight && Date.now() < deadline) {
    await new Promise<void>((resolve) => window.setTimeout(resolve, 40));
  }
  return !exitEditBoundsInFlight;
}

async function enterEditBounds() {
  if (editBoundsMode || editBoundsEnterPending || reloadInProgress) return;
  if (exitEditBoundsInFlight) {
    petMovementLog("enter-blocked", { detail: "exit-in-flight", ...movementLogFlags() });
    const idle = await waitForExitEditIdle();
    if (!idle || editBoundsMode) return;
  }
  editBoundsEnterPending = true;
  const enterT0 = performance.now();
  petMovementLog("enter-edit", { phase: "start", ...movementLogFlags() });
  cancelEditBoundsBlurExit();
  suppressEditBoundsExit(800);
  editBoundsAwaitMouseUp = true;

  try {
    await ensureClickThroughDisabled();
    if (!screenBounds) {
      await refreshScreenBounds();
    }
    petMovementLog("enter-edit", {
      phase: "ipc-done",
      ms: Math.round(performance.now() - enterT0),
      ...movementLogFlags(),
    });

    resetEditOverlayLayout();
    if (resizeDragging) {
      await resizeApplySerial;
    } else {
      resizeApplySerial = Promise.resolve();
    }
    resizeDragging = false;
    offsetDragging = false;
    pendingResizeBounds = null;
    lastResizeKey = "";

    await syncCanvasFromWindow(false);

    const cfg = await loadConfig();
    stageScale = cfg.scale || 0.8;

    setEditBoundsMode(true);

    const converted = convertStageOffsetForOrigin(
      cfg.offset_x ?? 0,
      cfg.offset_y ?? 0,
      stageScale,
      canvasDisplayW,
      canvasDisplayH,
      "bottom-center",
      "top-left",
    );
    stageOffsetX = converted.x;
    stageOffsetY = converted.y;

    if (pet) {
      pet.resizeCanvas(canvasDisplayW, canvasDisplayH, true);
    }
    clampStageOffset();
    applyStageTransform();
    startEditBoundsPoll();

    void petWindow.setFocus().catch(() => {});
    suppressEditBoundsExit(3500);
    pet?.setRenderPaused(false);
    petMovementLog("enter-edit", {
      phase: "ready",
      ms: Math.round(performance.now() - enterT0),
      ...movementLogFlags(),
    });
  } catch (err) {
    console.error("enterEditBounds failed", err);
    petMovementLog("enter-edit-fail", {
      error: String(err),
      ms: Math.round(performance.now() - enterT0),
      ...movementLogFlags(),
    });
    try {
      const cfg = await loadConfig();
      stageScale = cfg.scale || 0.8;
      stageOffsetX = cfg.offset_x ?? 0;
      stageOffsetY = cfg.offset_y ?? 0;
      clampStageOffset();
    } catch {
      // 恢复失败时仍退出编辑模式
    }
    setEditBoundsMode(false);
    applyStageTransform();
    void restoreNormalInteraction();
  } finally {
    editBoundsEnterPending = false;
  }
}



async function abandonEditBoundsOnHidden() {
  if (!editBoundsMode) return;
  stopEditBoundsPoll();
  cancelEditBoundsBlurExit();
  offsetDragging = false;
  resizeDragging = false;
  pendingResizeBounds = null;
  if (resizeRafId) {
    cancelAnimationFrame(resizeRafId);
    resizeRafId = 0;
  }
  await resizeApplySerial;
  if (pet) {
    const baked = pet.refitAndConsumeInternalOffset(stageScale);
    stageOffsetX += baked.dx;
    stageOffsetY += baked.dy;
  }
  const normalOffset = convertStageOffsetForOrigin(
    stageOffsetX,
    stageOffsetY,
    stageScale,
    canvasDisplayW,
    canvasDisplayH,
    "top-left",
    "bottom-center",
  );
  stageOffsetX = normalOffset.x;
  stageOffsetY = normalOffset.y;
  setEditBoundsMode(false);
  resetEditOverlayLayout();
  try {
    await persistLayoutSnapshotSafe("pet-hidden");
  } finally {
    await restoreNormalInteraction();
  }
}



async function exitEditBounds(reason = "unknown") {
  if (!editBoundsMode) {
    petMovementLog("exit-blocked", { reason, detail: "not-in-edit-mode", ...movementLogFlags() });
    return;
  }
  if (editBoundsEnterPending) {
    petMovementLog("exit-blocked", { reason, detail: "enter-pending", ...movementLogFlags() });
    return;
  }
  if (exitEditBoundsInFlight) {
    petMovementLog("exit-blocked", { reason, detail: "in-flight", ...movementLogFlags() });
    return;
  }

  petMovementLog("exit-attempt", { reason, ...movementLogFlags() });
  exitEditBoundsInFlight = true;

  await resizeApplySerial;

  const pendingCommit = pendingResizeBounds;
  const pendingEdge = resizeEdge;

  cancelEditBoundsBlurExit();
  stopEditBoundsPoll();
  offsetDragging = false;
  resizeDragging = false;
  pendingResizeBounds = null;
  lastResizeKey = "";
  resetEditOverlayLayout();

  pet?.setRenderPaused(false);
  suppressEditBoundsExit(800);

  try {
    if (pendingCommit) {
      await commitEditResize(pendingCommit, pendingEdge);
    }
    if (pet) {
      const baked = pet.refitAndConsumeInternalOffset(stageScale);
      stageOffsetX += baked.dx;
      stageOffsetY += baked.dy;
    }
    const normalOffset = convertStageOffsetForOrigin(
      stageOffsetX,
      stageOffsetY,
      stageScale,
      canvasDisplayW,
      canvasDisplayH,
      "top-left",
      "bottom-center",
    );
    stageOffsetX = normalOffset.x;
    stageOffsetY = normalOffset.y;
    setEditBoundsMode(false);
    await syncCanvasFromWindow(false);
    resetEditOverlayLayout();
    clampStageOffset();
    applyStageTransform();
    applyCanvasDisplaySize();
    await persistLayoutSnapshot();
    await savePosition();
  } catch (err) {
    console.error("exitEditBounds failed", err);
    petMovementLog("exit-error", { reason, error: String(err), ...movementLogFlags() });
    setEditBoundsMode(false);
    applyCanvasDisplaySize();
    clampStageOffset();
    applyStageTransform();
  } finally {
    exitEditBoundsInFlight = false;
    petMovementLog("exit-done", { reason, ...movementLogFlags() });
    void restoreNormalInteraction();
  }

}



resizeHandles.forEach((handle) => {

  handle.addEventListener("mousedown", (e: Event) => {

    const me = e as MouseEvent;

    if (!editBoundsMode || me.button !== 0) return;

    e.preventDefault();

    e.stopPropagation();

    suppressEditBoundsExit(1200);
    offsetDragging = false;

    const edge = handle.getAttribute("data-edge") as ResizeEdge | null;

    if (!edge) return;

    void beginResizeDrag(me, edge);

  }, true);

});

async function beginResizeDrag(me: MouseEvent, edge: ResizeEdge) {
  if (exitEditBoundsInFlight || editBoundsEnterPending) return;
  resizeEdge = edge;
  pendingResizeBounds = null;
  lastResizeKey = "";
  offsetDragging = false;
  suppressEditBoundsExit(1200);
  root.classList.add("edit-bounds-resizing");
  resizeDragging = true;

  try {
    const [bounds, sf] = await Promise.all([
      readWindowBoundsPhysical(),
      getCurrentWindow().scaleFactor(),
    ]);
    editResizeScaleFactor = sf;
    resizeStart = {
      x: me.screenX,
      y: me.screenY,
      w: bounds.width,
      h: bounds.height,
      posX: bounds.x,
      posY: bounds.y,
    };
    petMovementLog("resize-start", {
      edge,
      cursor: movementCursor(me),
      resizeStart,
      scaleFactor: editResizeScaleFactor,
      ...movementLogFlags(),
    });
  } catch (err) {
    console.error("beginResizeDrag failed", err);
    petMovementLog("resize-end", { edge, error: String(err), aborted: true, ...movementLogFlags() });
    resizeDragging = false;
    root.classList.remove("edit-bounds-resizing");
  }
}



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
      dragWindowPhysW,
      dragWindowPhysH,
    );

    scheduleDragPosition(clamped.x, clamped.y);

  }

  if (resizeDragging) {
    const bounds = computeResizeBounds(me.screenX, me.screenY);
    pendingResizeBounds = {
      w: bounds.w,
      h: bounds.h,
      x: bounds.x,
      y: bounds.y,
    };
    petMovementLogThrottled(`resize-${resizeEdge}`, "resize-move", {
      ...movementLogFlags(),
      cursor: movementCursor(me),
      bounds: pendingResizeBounds,
      resizeStart,
    });
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
    petMovementLogThrottled("offset-drag", "offset-move", {
      dx,
      dy,
      stageOffsetX,
      stageOffsetY,
      ...movementLogFlags(),
    });

  }

  if (!ignoreCursorActive && !mustCapturePointer()) {
    void syncClickThroughState({ x: me.clientX, y: me.clientY });
  }

});



document.addEventListener("mouseup", (e: Event) => {
  handlePointerUp(e as MouseEvent);
}, true);

function handlePointerUp(me: MouseEvent) {

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

      dispatchStageClick(me.clientX, me.clientY);

    } else if (windowDragStarted) {

      endWindowDrag();

      void savePosition();

    }

    windowDragStarted = false;

    windowDragAnchorReady = false;

  }

  if (resizeDragging && me.button === 0) {
    resizeDragging = false;
    root.classList.remove("edit-bounds-resizing");
    const bounds = pendingResizeBounds;
    const edge = resizeEdge;
    pendingResizeBounds = null;
    petMovementLog("resize-end", { edge, bounds, commit: !!bounds, ...movementLogFlags() });
    if (bounds) {
      void commitEditResize(bounds, edge);
      suppressEditBoundsExit(800);
    } else {
      resetEditOverlayLayout();
    }
    return;
  }

  if (offsetDragging && me.button === 0) {
    clampStageOffset();
    applyStageTransform();
    petMovementLog("offset-end", {
      stageOffsetX,
      stageOffsetY,
      ...movementLogFlags(),
    });
  }

  offsetDragging = false;

  if (!mustCapturePointer()) {
    void syncClickThroughState({ x: me.clientX, y: me.clientY });
  }
}



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

  if (petMenuOpen) {
    e.preventDefault();
    void invoke("pet_menu_hide");
    return;
  }

  if (editBoundsMode) {
    if (exitEditBoundsInFlight || editBoundsEnterPending) return;
    e.preventDefault();
    const edge = hitTestEditEdge(e.clientX, e.clientY);
    if (edge) {
      suppressEditBoundsExit(1200);
      void beginResizeDrag(e, edge);
      return;
    }
    if (!hitInteractive(e.clientX, e.clientY)) {
      void exitEditBounds("click-empty");
      return;
    }
    suppressEditBoundsExit(1200);
    offsetDragging = false;
    beginOffsetDrag(e.clientX, e.clientY);
    return;
  }

  suppressClickCapture(DOUBLE_CLICK_MS);

  pointerDown = true;
  void applyClickThrough(false, true);
  void refreshDragWindowSize();
  void readWindowBoundsPhysical().then((bounds) => {
    cachedWinPos = { x: bounds.x, y: bounds.y };
  });

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



stage.addEventListener("contextmenu", (e) => {
  if (!hitInteractive(e.clientX, e.clientY)) return;
  e.preventDefault();
  void applyClickThrough(false, true).then(() => openPetMenu());
});

root.addEventListener("contextmenu", (e) => {
  if (e.target === stage || stage.contains(e.target as Node)) return;
  if (!hitInteractive(e.clientX, e.clientY)) return;
  e.preventDefault();
  void applyClickThrough(false, true).then(() => openPetMenu());
});

async function openPetMenu() {
  if (rightClickMenuLock) return;
  rightClickMenuLock = true;
  try {
    await invoke<boolean>("pet_menu_toggle_at_cursor");
  } catch (err) {
    console.error("打开桌宠菜单失败", err);
    if (!petMenuOpen && !shouldDeferClickThrough()) startClickThrough();
  } finally {
    window.setTimeout(() => {
      rightClickMenuLock = false;
    }, 250);
  }
}



document.addEventListener("mousedown", (e) => {

  if (Date.now() < editBoundsSuppressUntil) {

    return;

  }

  if (resizeDragging || offsetDragging) {

    return;

  }

  if (editBoundsMode) {

    if (isInsideEditArea(e.target)) {

      return;

    }

    petMovementLog("mousedown-outside", {
      target: e.target instanceof Element ? e.target.className : String(e.target),
      ...movementLogFlags(),
    });
    void exitEditBounds("mousedown-outside");

    return;

  }

});



void tauriReady.then(() => {
  void getCurrentWindow().listen("tauri://blur", () => {
    if (!editBoundsMode || exitEditBoundsInFlight) return;
    if (Date.now() < editBoundsSuppressUntil) return;
    if (Date.now() < menuCloseSuppressUntil) return;
    void exitEditBounds("blur");
  });

  void getCurrentWindow().listen("tauri://focus", () => {
    if (editBoundsMode) {
      cancelEditBoundsBlurExit();
    }
  });
});

window.addEventListener("keydown", (e) => {
  if (!editBoundsMode || e.key !== "Escape") return;
  e.preventDefault();
  petMovementLog("esc-exit", movementLogFlags());
  offsetDragging = false;
  void (async () => {
    if (resizeDragging) {
      resizeDragging = false;
      root.classList.remove("edit-bounds-resizing");
      resetEditOverlayLayout();
    }
    await exitEditBounds("esc");
  })();
});



document.addEventListener("visibilitychange", () => {
  if (mainWindowCovering || Date.now() < suppressVisibilityUntil) {
    return;
  }
  if (document.hidden) {
    pet?.setRenderPaused(true);
    stopClickThrough();
  } else {
    if (pendingBubble) {
      const next = pendingBubble;
      pendingBubble = null;
      showBubble(next.text, next.animation);
    }
    void resumePetFromHidden();
  }
  if (!document.hidden && !shouldDeferClickThrough()) startClickThrough();
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

  const unlistenEditBounds = await listen("pet-enter-edit-bounds", () => {
    void enterEditBounds();
  });

  const unlistenMenuState = await listen<boolean>("pet-menu-state", (ev) => {
    const wasOpen = petMenuOpen;
    petMenuOpen = !!ev.payload;
    if (wasOpen && !petMenuOpen) {
      menuCloseSuppressUntil = Date.now() + 1800;
      suppressEditBoundsExit(1800);
    }
    if (!petMenuOpen) {
      prevLeftMouseDown = false;
      if (!shouldDeferClickThrough()) startClickThrough();
    }
  });

  const unlistenClickThrough = await listen("pet-sync-click-through", () => {
    if (!petMenuOpen && !shouldDeferClickThrough()) startClickThrough();
  });

  const unlistenBubble = await listen<boolean>("pet-bubble-enabled-changed", (ev) => {
    applyBubbleEnabledFromConfig(ev.payload);
    if (!ev.payload) clearBubble();
  });

  const unlistenScale = await listen<number>("pet-scale-changed", (ev) => {
    if (appExiting || editBoundsMode) return;
    const scale = ev.payload;
    if (typeof scale !== "number") return;
    void applyStageScale(scale);
    clampStageOffset();
    applyStageTransform();
  });

  petEventUnlisten = () => {
    unlistenRemark();
    unlistenReload();
    unlistenResume();
    unlistenAnimations();
    unlistenPreview();
    unlistenContext();
    unlistenEditBounds();
    unlistenMenuState();
    unlistenClickThrough();
    unlistenBubble();
    unlistenScale();
    petEventUnlisten = null;
  };

  if (import.meta.hot) {
    import.meta.hot.dispose(() => {
      petEventUnlisten?.();
      stopClickThroughPoll();
      stopEditBoundsPoll();
      pet?.dispose();
      pet = null;
      petAssetResolver?.dispose();
      petAssetResolver = null;
      releaseCanvasGlContext();
    });
  }
}

async function bootPetWindow() {
  await waitForTauriInternals();
  try {
    const dbPath = await invoke<string>("app_get_data_path");
    const dataDir = dbPath.replace(/[\\/][^\\/]+$/, "");
    initPetMovementLog(
      (lines) => invoke("pet_append_movement_logs", { lines }),
      dataDir,
    );
  } catch (err) {
    console.error("pet movement log init failed", err);
    initPetMovementLog(async () => {});
  }
  void refreshScreenBounds();
  await setupPetEvents();
  await loadBubbleEnabled();
  // 首启仅依赖 Rust nudge；短兜底防监听器就绪前 nudge 丢失（勿立即 reload，避免与 nudge 双重重载）
  window.setTimeout(() => {
    if (!pet && !reloadInProgress && !reloadEverStarted) {
      void reloadPet();
    }
  }, 600);
}

void bootPetWindow();


