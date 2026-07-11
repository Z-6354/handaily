import { cursorPosition, getCurrentWindow } from "@tauri-apps/api/window";

import { LogicalSize, PhysicalPosition } from "@tauri-apps/api/dpi";

import { listen } from "@tauri-apps/api/event";

import { tauriInvoke as invoke, waitForTauriInternals } from "../lib/tauriInvoke";

import "./pet.css";

import { SpinePet } from "./spinePet";
import {
  createPetDisplayModule,
  reloadCommand,
  type PetAnimationMeta,
  type PetConfigPayload,
  type PetRemarkLine,
  type PetDisplayModule,
  type ReloadSource,
} from "./display";
import { releaseCanvasGlContext } from "./display/canvasHost";
import {
  cancelDeferredSkinPreload,
  scheduleSiblingSkinPreload,
} from "./display/assetPreloadScheduler";
import {
  convertStageOffsetForOrigin,
  isCursorOutsideWindow,
} from "./petLayout";
import {
  initPetMovementLog,
  petMovementLog,
  petMovementLogThrottled,
} from "./petMovementLog";
import { petLog, initPetLogSink } from "./log";

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
    mainWindowVisible,
    bubbleEnabled,
    pollPointerCapture,
    stageOffsetX,
    stageOffsetY,
    stageScale,
    canvasDisplayW,
    canvasDisplayH,
    suppressUntil: suppressVisibilityUntil,
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

interface PetRemarkPayload {

  text: string;

  source: string;

  animation?: string | null;

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



let canvas = document.createElement("canvas");

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
let bubbleLifecycleToken = 0;
let pendingBubble: { text: string; animation?: string | null } | null = null;
let bubbleEnabled = true;

let pet: SpinePet | null = null;
let petDisplay: PetDisplayModule;
let petDisplayReady = false;
let pendingPetReload = false;
let pendingSwitchReload = false;
let pendingReloadConfig: PetConfigPayload | null = null;
interface PetSwitchPayload {
  switch_id: number;
  config: PetConfigPayload;
}
let pendingPetSwitch: PetSwitchPayload | null = null;
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
  return (
    editBoundsMode ||
    editBoundsEnterPending ||
    mainWindowVisible ||
    (petDisplayReady && petDisplay.reloadInProgress)
  );
}

function shouldFocusPet(): boolean {
  return !mainWindowVisible && Date.now() >= suppressVisibilityUntil;
}

function canRunClickThroughPoll(): boolean {
  return (
    !document.hidden &&
    !mainWindowVisible &&
    !petMenuOpen &&
    !shouldDeferClickThrough()
  );
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

let restoreInteractionToken = 0;
let restoreScheduleTimer: ReturnType<typeof setTimeout> | null = null;
let restoreScheduleOpts: { focusPet?: boolean; light?: boolean } | undefined;
let restoreDeferredForGesture = false;
const RESTORE_DEBOUNCE_MS = 64;
const earlyUnlisteners: Array<() => void> = [];

function isUserGestureActive(): boolean {
  return (
    pointerDown ||
    windowDragStarted ||
    pollPointerCapture ||
    offsetDragging ||
    resizeDragging
  );
}

function cancelScheduledRestore() {
  if (restoreScheduleTimer) {
    clearTimeout(restoreScheduleTimer);
    restoreScheduleTimer = null;
  }
  restoreScheduleOpts = undefined;
}

function flushDeferredRestoreIfIdle() {
  if (!restoreDeferredForGesture || isUserGestureActive()) return;
  restoreDeferredForGesture = false;
  scheduleRestoreInteraction();
}

function scheduleRestoreInteraction(options?: { focusPet?: boolean; light?: boolean }) {
  if (appExiting) return;
  if (options) {
    restoreScheduleOpts = { ...restoreScheduleOpts, ...options };
  } else if (!restoreScheduleOpts) {
    restoreScheduleOpts = options;
  }
  if (isUserGestureActive()) {
    restoreDeferredForGesture = true;
    return;
  }
  const run = () => {
    restoreScheduleTimer = null;
    if (isUserGestureActive()) {
      restoreDeferredForGesture = true;
      return;
    }
    restoreDeferredForGesture = false;
    const opts = restoreScheduleOpts;
    restoreScheduleOpts = undefined;
    void restoreNormalInteraction(opts);
  };
  if (restoreScheduleOpts?.light) {
    if (restoreScheduleTimer) {
      clearTimeout(restoreScheduleTimer);
      restoreScheduleTimer = null;
    }
    run();
    return;
  }
  if (restoreScheduleTimer) clearTimeout(restoreScheduleTimer);
  restoreScheduleTimer = window.setTimeout(run, RESTORE_DEBOUNCE_MS);
}

function pauseBubbleForOverlay() {
  bubbleLifecycleToken += 1;
  if (bubbleTimer) {
    clearTimeout(bubbleTimer);
    bubbleTimer = null;
  }
  bubble.classList.remove("visible");
}

function flushPendingBubbleIfAllowed() {
  if (!pendingBubble || !bubbleEnabled || mainWindowVisible || petMenuOpen || document.hidden) {
    return;
  }
  const next = pendingBubble;
  pendingBubble = null;
  showBubble(next.text, next.animation);
}

function requestClickThroughSync() {
  if (!canRunClickThroughPoll()) return;
  ensureClickThroughPoll();
  void syncClickThroughState();
}

function resumeInteractionAfterMainClose() {
  if (document.hidden || editBoundsMode || appExiting || mainWindowVisible) return;
  ignoreCursorActive = false;
  void applyClickThrough(false, true);
  if (canRunClickThroughPoll()) {
    ensureClickThroughPoll();
  }
  scheduleRestoreInteraction({ focusPet: false, light: true });
}

function applyMainWindowVisibility(visible: boolean, suppressMs = 600) {
  suppressVisibilityUntil = Math.max(suppressVisibilityUntil, Date.now() + suppressMs);
  if (visible) {
    if (mainWindowVisible) return;
    mainWindowVisible = true;
    stopClickThroughPoll();
    pauseBubbleForOverlay();
    cancelScheduledRestore();
    void restoreNormalInteraction({ focusPet: false });
    return;
  }
  if (!mainWindowVisible) return;
  mainWindowVisible = false;
  if (!document.hidden) {
    resumeInteractionAfterMainClose();
    window.setTimeout(() => flushPendingBubbleIfAllowed(), 120);
  }
}

function disposeEarlyListeners() {
  while (earlyUnlisteners.length > 0) {
    earlyUnlisteners.pop()?.();
  }
}

async function restoreNormalInteraction(options?: { focusPet?: boolean; light?: boolean }) {
  const token = ++restoreInteractionToken;
  const gestureActive = isUserGestureActive();
  const light = options?.light ?? false;
  if (!light && !gestureActive) {
    resetPointerGestureState();
    pollPointerCapture = false;
  }
  stopEditBoundsPoll();
  cancelEditBoundsBlurExit();
  if (!light && !gestureActive) {
    stopClickThroughPoll();
    clickThroughApplySerial += 1;
  }
  editBoundsEnterPending = false;
  if (document.hidden || editBoundsMode || appExiting) return;
  if (gestureActive) {
    restoreDeferredForGesture = true;
    return;
  }
  if (!light) {
    petMovementLog("restore-interaction", movementLogFlags());
  }
  try {
    if (!light || ignoreCursorActive) {
      ignoreCursorActive = false;
      await applyClickThrough(false, true);
    }
    if (token !== restoreInteractionToken) return;
    if (mainWindowVisible) return;
    if (token !== restoreInteractionToken) return;
    if (canRunClickThroughPoll()) {
      ensureClickThroughPoll();
    }
    const focusPet = options?.focusPet ?? shouldFocusPet();
    if (focusPet && !isUserGestureActive()) {
      window.setTimeout(() => {
        if (token !== restoreInteractionToken) return;
        if (shouldFocusPet() && !document.hidden && !editBoundsMode && !appExiting && !isUserGestureActive()) {
          void petWindow.setFocus().catch(() => {});
        }
      }, 80);
    }
  } catch (err) {
    if (token !== restoreInteractionToken) return;
    console.error("restoreNormalInteraction failed", err);
    ignoreCursorActive = false;
    void applyClickThrough(false, true).then(() => {
      if (token !== restoreInteractionToken) return;
      if (canRunClickThroughPoll()) {
        ensureClickThroughPoll();
      }
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
let mainWindowVisible = false;
let suppressVisibilityUntil = 0;
let pollPointerCapture = false;

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

  if (!ignoreCursorActive || shouldDeferClickThrough() || petMenuOpen || rightClickMenuLock || mainWindowVisible) {
    pollLeftWasDown = down;
    return;
  }

  const edgeDown = down && !pollLeftWasDown;
  const edgeUp = !down && pollLeftWasDown;

  if (edgeDown && hitInteractive(pointer.x, pointer.y)) {
    suppressClickCapture(DOUBLE_CLICK_MS);
    pollPointerCapture = true;
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
    ignoreCursorActive = false;
    void applyClickThrough(false, true);
    void refreshDragWindowSize();
    void readWindowBoundsPhysical().then((bounds) => {
      cachedWinPos = { x: bounds.x, y: bounds.y };
    });
  }

  if (edgeUp && pollLeftWasDown) {
    pollPointerCapture = false;
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

async function trackPollPointerDrag(
  pointer: { x: number; y: number },
  screen: { x: number; y: number },
) {
  if (!pointerDown || !pollPointerCapture || editBoundsMode) return;
  let down = false;
  try {
    down = await invoke<boolean>("pet_is_left_mouse_down");
  } catch {
    return;
  }

  if (!windowDragStarted) {
    const dx = pointer.x - pointerStart.x;
    const dy = pointer.y - pointerStart.y;
    if (Math.hypot(dx, dy) >= DRAG_THRESHOLD) {
      ignoreCursorActive = false;
      void applyClickThrough(false, true);
      void beginWindowDrag(pointerStart.screenX, pointerStart.screenY);
    }
  } else if (windowDragAnchorReady) {
    const dx = screen.x - dragAnchor.screenX;
    const dy = screen.y - dragAnchor.screenY;
    const clamped = clampWindowPosition(
      dragAnchor.winX + dx,
      dragAnchor.winY + dy,
      dragWindowPhysW,
      dragWindowPhysH,
    );
    scheduleDragPosition(clamped.x, clamped.y);
  }

  const edgeUp = !down && pollLeftWasDown;
  if (edgeUp) {
    pollPointerCapture = false;
    const elapsed = Date.now() - pollLeftDownAt;
    const dist = Math.hypot(pointer.x - pollLeftDownClient.x, pointer.y - pollLeftDownClient.y);
    const isClick =
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
    flushDeferredRestoreIfIdle();
  }

  pollLeftWasDown = down;
}

let clickThroughApplySerial = 0;

async function applyClickThrough(ignore: boolean, force = false) {
  if (ignore && mainWindowVisible) return;
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
  if (document.hidden || !canRunClickThroughPoll()) return;
  if (clickThroughInterval) return;
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
  if (appExiting) {
    stopClickThroughPoll();
    return;
  }
  if (mainWindowVisible) {
    stopClickThroughPoll();
    if (ignoreCursorActive) {
      ignoreCursorActive = false;
      void applyClickThrough(false, true);
    }
    return;
  }
  if (editBoundsMode) {
    await runEditBoundsPollTick();
    return;
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

  if (pointerDown && pollPointerCapture) {
    await trackPollPointerDrag(pointer, screen);
  }

  if (shouldDeferClickThrough() || mustCapturePointer()) return;

  if (petMenuOpen) {
    await dismissMenuOnOutsideLeftClick();
  } else {
    prevLeftMouseDown = false;
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
  if (mainWindowVisible || petMenuOpen) {
    if (ignoreCursorActive) {
      await applyClickThrough(false, true);
    }
    return;
  }
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
  if (!canRunClickThroughPoll()) {
    stopClickThroughPoll();
    return;
  }
  requestClickThroughSync();
}

function stopClickThrough() {
  stopClickThroughPoll();
  void applyClickThrough(false, true);
}

async function ensureClickThroughDisabled() {
  stopClickThroughPoll();
  clickThroughApplySerial += 1;
  ignoreCursorActive = false;
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
  bubbleLifecycleToken += 1;
  bubble.classList.remove("visible");
  if (bubbleTimer) {
    clearTimeout(bubbleTimer);
    bubbleTimer = null;
  }
  pendingBubble = null;
}

function showBubble(text: string, animation?: string | null) {
  if (!bubbleEnabled) return;
  if (mainWindowVisible || petMenuOpen) {
    pendingBubble = { text, animation };
    return;
  }
  if (document.hidden) {
    pendingBubble = { text, animation };
    return;
  }

  bubbleLifecycleToken += 1;
  const token = bubbleLifecycleToken;

  bubble.textContent = text;
  bubble.classList.remove("visible");
  void bubble.offsetWidth;
  bubble.classList.add("visible");
  positionBubble();

  if (bubbleTimer) clearTimeout(bubbleTimer);
  bubbleTimer = setTimeout(() => {
    if (token !== bubbleLifecycleToken || !bubbleEnabled) {
      bubbleTimer = null;
      return;
    }
    bubble.classList.remove("visible");
    bubbleTimer = null;
    if (!editBoundsMode && !petMenuOpen && !mainWindowVisible) {
      requestClickThroughSync();
    }
  }, 8000);

  if (animation && pet) {
    pet.playAnimation(animation, false);
  }
  if (!editBoundsMode && !petMenuOpen && !mainWindowVisible) {
    requestClickThroughSync();
  }
}

function applyBubbleEnabledFromConfig(enabled: boolean | undefined) {
  if (typeof enabled !== "boolean") return;
  const wasEnabled = bubbleEnabled;
  bubbleEnabled = enabled;
  if (!enabled) {
    clearBubble();
    if (wasEnabled && !petMenuOpen && !mainWindowVisible && !document.hidden) {
      requestClickThroughSync();
    }
  }
}

async function loadBubbleEnabled() {
  try {
    bubbleEnabled = await invoke<boolean>("pet_get_bubble_enabled");
  } catch {
    bubbleEnabled = true;
  }
  if (!bubbleEnabled) clearBubble();
}

async function loadConfig(): Promise<PetConfigPayload> {
  const cached = pendingReloadConfig;
  if (cached) {
    pendingReloadConfig = null;
    petLog("debug", "reload", "using nudge config", { modelId: cached.model_id });
    return cached;
  }
  return invoke<PetConfigPayload>("pet_get_config");
}

function syncPetFromDisplay() {
  pet = petDisplay.pet;
}

function initPetDisplayModule() {
  petDisplay = createPetDisplayModule(
    {
      canvas,
      canvasWrap,
      fallback,
      getDisplaySize: () => ({ w: canvasDisplayW, h: canvasDisplayH }),
      setDisplaySize: (w, h) => {
        canvasDisplayW = w;
        canvasDisplayH = h;
      },
      getFallbackSrc: () => lastFallbackSrc,
      setFallbackSrc: (url) => {
        lastFallbackSrc = url;
        fallback.src = url;
      },
      showBootHint,
      hideBootHint,
      clearLoadError: clearPetLoadError,
      pickLine: pickLineForAnimation,
      showBubble,
      replaceCanvas: (next) => {
        canvas = next;
      },
    },
    {
      loadConfig,
      refreshScreenBounds,
      resolveWindowSize: async (cfg, mode) => {
        let nextW = Math.max(MIN_W, cfg.window_width || 240);
        let nextH = Math.max(MIN_H, cfg.window_height || 320);
        if (mode === "hot") {
          nextW = Math.max(MIN_W, canvasDisplayW);
          nextH = Math.max(MIN_H, canvasDisplayH);
        } else {
          try {
            const frameBounds = await readWindowBounds();
            if (Number.isFinite(frameBounds.w) && frameBounds.w >= MIN_W) {
              nextW = Math.max(MIN_W, frameBounds.w);
            }
            if (Number.isFinite(frameBounds.h) && frameBounds.h >= MIN_H) {
              nextH = Math.max(MIN_H, frameBounds.h);
            }
          } catch {
            // DB 尺寸
          }
        }
        return { w: nextW, h: nextH };
      },
      applyWindowSize: async (w, h) => applyWindowSize(w, h, false),
      applyLayoutFromConfig: (cfg) => {
        stageScale = cfg.scale || 0.8;
        stageOffsetX = cfg.offset_x ?? 0;
        stageOffsetY = cfg.offset_y ?? 0;
        clampStageOffset();
      },
      applyCanvasDisplaySize,
      shouldExitEditBeforeReload: async () => {
        if (editBoundsMode) await exitEditBounds();
      },
      isAppExiting: () => appExiting,
      syncAnimations,
      getPendingPreview: () => pendingPreview,
      clearPendingPreview: () => {
        pendingPreview = null;
      },
      runPreviewAnimation,
    },
    invoke,
  );

  petDisplay.on("reload-start", ({ isSwitch }) => {
    cancelDeferredSkinPreload(isSwitch ? "switch" : "reload");
    if (!isSwitch) showBootHint();
  });

  petDisplay.on("reload-success", ({ cfg, animationNames }) => {
    syncPetFromDisplay();
    pet?.setRenderPaused(false);
    void applyStageScale(stageScale);
    clampStageOffset();
    applyStageTransform();
    applyCanvasDisplaySize();
    if (!petDisplay.reloadInProgress && canRunClickThroughPoll()) startClickThrough();
    petLog("info", "reload", "success", { modelId: cfg.model_id, animations: animationNames.length });
    scheduleSiblingSkinPreload(cfg.model_id);
  });

  petDisplay.on("reload-failure", ({ err }) => showPetLoadError(err));

  petDisplay.on("reload-finally", () => {
    scheduleRestoreInteraction();
    if (!pendingSwitchReload) return;
    pendingSwitchReload = false;
    void (async () => {
      const cached = pendingReloadConfig;
      const current = petDisplay.currentModelId;
      if (cached && current && cached.model_id === current) {
        pendingReloadConfig = null;
        petLog("info", "reload", "skip coalesced same model", { modelId: current });
        return;
      }
      await reloadPet("rust");
      while (pendingSwitchReload) {
        pendingSwitchReload = false;
        const again = pendingReloadConfig;
        const loaded = petDisplay.currentModelId;
        if (again && loaded && again.model_id === loaded) {
          pendingReloadConfig = null;
          petLog("info", "reload", "skip coalesced same model", { modelId: loaded });
          continue;
        }
        await reloadPet("rust");
      }
    })();
  });
}

function requestPetReload(source: ReloadSource = "rust") {
  if (!petDisplayReady) {
    pendingPetReload = true;
    petLog("info", "boot", "reload queued (display not ready)", { source });
    return;
  }
  if (petDisplay.reloadInProgress) {
    pendingSwitchReload = true;
    petLog("info", "reload", "coalesced (reload in progress)", { source });
    return;
  }
  if (!bootReloadStarted) {
    startBootReload(`nudge-${source}`);
    return;
  }
  void reloadPet(source);
}

async function reloadPet(source: ReloadSource = "rust") {
  if (!petDisplayReady) {
    pendingPetReload = true;
    return;
  }
  petLog("info", "reload", "request", { source });
  await petDisplay.reload(reloadCommand("pet-reload", source));
}

async function executePetSwitch(payload: PetSwitchPayload) {
  if (!petDisplayReady) {
    pendingPetSwitch = payload;
    return;
  }
  petLog("info", "switch", "menu switch request", {
    switchId: payload.switch_id,
    modelId: payload.config.model_id,
  });
  await petDisplay.switchModel(payload.config, payload.switch_id);
}

function flushPendingPetSwitch() {
  if (!pendingPetSwitch || !petDisplayReady) return;
  const payload = pendingPetSwitch;
  pendingPetSwitch = null;
  void executePetSwitch(payload);
}

let bootReloadStarted = false;

function startBootReload(reason: string) {
  if (bootReloadStarted || !petDisplayReady) return;
  bootReloadStarted = true;
  petLog("info", "boot", "start model load", { reason });
  void petDisplay.reload(reloadCommand("nudge", "boot", reason));
}

function flushPendingPetReload() {
  if (!pendingPetReload || !petDisplayReady) return;
  pendingPetReload = false;
  if (!bootReloadStarted) {
    startBootReload("queued-nudge");
  } else {
    void reloadPet("rust");
  }
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
        applyStageTransform();
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
    canvasWrap.style.width = `${canvasDisplayW}px`;
    canvasWrap.style.height = `${canvasDisplayH}px`;
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

function disposePetForExit() {
  if (appExiting) return;
  appExiting = true;
  restoreInteractionToken += 1;
  cancelScheduledRestore();
  root.style.visibility = "hidden";
  root.style.pointerEvents = "none";
  canvasWrap.style.visibility = "hidden";
  fallback.style.display = "none";
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
  petDisplay?.disposeForExit();
  pet = null;
  releaseCanvasGlContext(canvas);
  petEventUnlisten?.();
  disposeEarlyListeners();
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
  if (Date.now() < suppressVisibilityUntil) return;
  if (petDisplay.reloadInProgress) {
    await petDisplay.whenIdle();
    if (!document.hidden) void resumePetFromHidden();
    return;
  }
  const now = Date.now();
  if (resumeInFlight || now - lastResumeAt < RESUME_DEBOUNCE_MS) return;
  resumeInFlight = true;
  lastResumeAt = now;
  try {
    await waitUntilVisibleForLoad();
    if (!pet) {
      void reloadPet("visibility");
      return;
    }
    pet.setRenderPaused(false);
    await syncCanvasFromWindow(false);
    applyStageTransform();
  } finally {
    resumeInFlight = false;
  }
}

// 尽早注册，避免 Rust on_page_load 发出的 pet-reload 在监听器就绪前丢失

const tauriReady = waitForTauriInternals();

void tauriReady.then(async () => {
  earlyUnlisteners.push(
    await listen("pet-app-exiting", () => {
      disposePetForExit();
    }),
  );

  earlyUnlisteners.push(
    await listen<number>("pet-main-opening", (ev) => {
      applyMainWindowVisibility(true, ev.payload ?? 1200);
    }),
  );

  earlyUnlisteners.push(
    await listen("pet-main-closed", () => {
      suppressVisibilityUntil = Math.max(suppressVisibilityUntil, Date.now() + 200);
      if (pet) {
        pet.setRenderPaused(false);
        applyStageTransform();
      } else {
        void resumePetFromHidden();
      }
      applyMainWindowVisibility(false, 200);
    }),
  );

  earlyUnlisteners.push(
    await listen<boolean>("main-window-visible", (ev) => {
      if (ev.payload) {
        applyMainWindowVisibility(true, 600);
        return;
      }
      applyMainWindowVisibility(false, 200);
    }),
  );

  earlyUnlisteners.push(
    await listen("pet-hidden", () => {
      petMenuOpen = false;
      pendingBubble = null;
      stopClickThrough();
      pet?.setRenderPaused(true);
      if (editBoundsMode) {
        void abandonEditBoundsOnHidden();
      }
    }),
  );
});

const petReloadUnlistenPromise = tauriReady.then(() =>
  listen<PetConfigPayload | null>("pet-reload", (ev) => {
    if (ev.payload) {
      pendingReloadConfig = ev.payload;
    }
    requestPetReload("rust");
  }),
);

const petSwitchUnlistenPromise = tauriReady.then(() =>
  listen<PetSwitchPayload>("pet-switch", (ev) => {
    void executePetSwitch(ev.payload);
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
   petDisplay.setPetLines(cfg.lines ?? []);
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
  if (editBoundsMode || editBoundsEnterPending) return;
  if (petDisplayReady && petDisplay.reloadInProgress) {
    petLog("info", "edit", "waiting for model reload before enter");
    await petDisplay.whenIdle();
  }
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
    suppressEditBoundsExit(400);
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
    pollPointerCapture = false;

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
  flushDeferredRestoreIfIdle();
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
      editBoundsAwaitMouseUp = false;
      void exitEditBounds("click-empty");
      return;
    }
    suppressEditBoundsExit(1200);
    offsetDragging = false;
    beginOffsetDrag(e.clientX, e.clientY);
    return;
  }

  suppressClickCapture(DOUBLE_CLICK_MS);
  suppressVisibilityUntil = Math.max(suppressVisibilityUntil, Date.now() + 500);

  pollPointerCapture = false;
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
  if (ignoreCursorActive) return;
  stopClickThroughPoll();
  void openPetMenu();
  void applyClickThrough(false, true);
});

root.addEventListener("contextmenu", (e) => {
  if (e.target === stage || stage.contains(e.target as Node)) return;
  if (!hitInteractive(e.clientX, e.clientY)) return;
  e.preventDefault();
  if (ignoreCursorActive) return;
  stopClickThroughPoll();
  void openPetMenu();
  void applyClickThrough(false, true);
});

async function openPetMenu() {
  if (rightClickMenuLock) return;
  rightClickMenuLock = true;
  try {
    if (petDisplayReady && petDisplay.reloadInProgress) {
      petLog("info", "menu", "open during reload");
    }
    const open = await invoke<boolean>("pet_menu_toggle_at_cursor");
    if (open) {
      void ensureClickThroughDisabled();
    }
  } catch (err) {
    console.error("打开桌宠菜单失败", err);
    if (!petMenuOpen && !shouldDeferClickThrough()) startClickThrough();
  } finally {
    window.setTimeout(() => {
      rightClickMenuLock = false;
    }, 120);
  }
}



document.addEventListener("mousedown", (e) => {

  if (resizeDragging || offsetDragging) {

    return;

  }

  if (editBoundsMode) {

    if (isInsideEditArea(e.target)) {

      return;

    }

    editBoundsAwaitMouseUp = false;
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
  if (Date.now() < suppressVisibilityUntil) {
    return;
  }
  if (document.hidden) {
    pet?.setRenderPaused(true);
    stopClickThrough();
  } else {
    flushPendingBubbleIfAllowed();
    void resumePetFromHidden();
  }
  if (!document.hidden && canRunClickThroughPoll()) startClickThrough();
});



let petEventUnlisten: (() => void) | null = null;

async function setupPetEvents() {
  if (petEventUnlisten) return;

  const unlistenReload = await petReloadUnlistenPromise;
  const unlistenSwitch = await petSwitchUnlistenPromise;
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
    if (petMenuOpen) {
      void ensureClickThroughDisabled();
      return;
    }
    if (wasOpen) {
      menuCloseSuppressUntil = Date.now() + 1800;
      suppressEditBoundsExit(1800);
      prevLeftMouseDown = false;
      void loadBubbleEnabled().then(() => {
        if (!bubbleEnabled) {
          clearBubble();
        } else {
          flushPendingBubbleIfAllowed();
        }
      });
    }
    scheduleRestoreInteraction({ focusPet: shouldFocusPet() });
  });

  const unlistenClickThrough = await listen("pet-sync-click-through", () => {
    if (mainWindowVisible) {
      stopClickThroughPoll();
      if (ignoreCursorActive) {
        void applyClickThrough(false, true);
      }
      return;
    }
    if (!shouldDeferClickThrough()) {
      requestClickThroughSync();
    }
  });

  const unlistenBubble = await listen<boolean>("pet-bubble-enabled-changed", (ev) => {
    applyBubbleEnabledFromConfig(ev.payload);
  });
  const unlistenClearBubble = await listen("pet-clear-bubble", () => {
    clearBubble();
  });

  const unlistenTestAction = await listen<string>("pet-test-action", (ev) => {
    const action = ev.payload;
    petMovementLog("test-action", { action, ...movementLogFlags() });
    if (action === "click-left") {
      pet?.handleClick();
      return;
    }
    if (action === "click-double") {
      void openMainFromDoubleClick();
      return;
    }
    if (action === "sync-interaction") {
      scheduleRestoreInteraction({ focusPet: shouldFocusPet() });
    }
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
    unlistenSwitch();
    unlistenResume();
    unlistenAnimations();
    unlistenPreview();
    unlistenContext();
    unlistenEditBounds();
    unlistenMenuState();
    unlistenClickThrough();
    unlistenBubble();
    unlistenClearBubble();
    unlistenTestAction();
    unlistenScale();
    petEventUnlisten = null;
  };

  if (import.meta.hot) {
    import.meta.hot.dispose(() => {
      restoreInteractionToken += 1;
      cancelScheduledRestore();
      petEventUnlisten?.();
      disposeEarlyListeners();
      stopClickThroughPoll();
      stopEditBoundsPoll();
      bubbleLifecycleToken += 1;
      if (bubbleTimer) {
        clearTimeout(bubbleTimer);
        bubbleTimer = null;
      }
      petDisplay?.disposeForExit();
      pet = null;
      releaseCanvasGlContext(canvas);
    });
  }
}

async function bootPetWindow() {
  petLog("info", "boot", "pet window script start");
  await waitForTauriInternals();
  initPetDisplayModule();
  petLog("info", "boot", "display module ready");

  try {
    const dbPath = await invoke<string>("app_get_data_path");
    const dataDir = dbPath.replace(/[\\/][^\\/]+$/, "");
    initPetLogSink(
      (lines) => invoke("pet_append_display_logs", { lines }),
      dataDir,
    );
    initPetMovementLog(
      (lines) => invoke("pet_append_movement_logs", { lines }),
      dataDir,
    );
  } catch (err) {
    console.error("pet log init failed", err);
    initPetMovementLog(async () => {});
  }

  await setupPetEvents();
  await loadBubbleEnabled();
  petDisplayReady = true;
  flushPendingPetReload();
  flushPendingPetSwitch();
  void refreshScreenBounds();

  if (!bootReloadStarted) {
    startBootReload("boot-initial");
  }

  window.setTimeout(() => {
    if (!pet && !bootReloadStarted) {
      petLog("warn", "boot", "fallback reload (no load started)");
      startBootReload("boot-fallback");
    }
  }, 1200);
}

void bootPetWindow();


