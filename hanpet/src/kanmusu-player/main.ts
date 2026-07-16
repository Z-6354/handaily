import * as PIXI from "pixi.js";
import { Live2DModel } from "pixi-live2d-display-lipsyncpatch/cubism4";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { tauriInvoke as invoke } from "../lib/tauriInvoke";

import {
  buildCubismSettings,
  clearActiveKanmusuResolveMap,
  disposeKanmusuBlobs,
  model3FilenameFromPath,
  prefetchKanmusuSkin,
} from "./kanmusuAssets";
import { KanmusuInteractor } from "./interact";
import "./player.css";

interface KanmusuPlayerLoadPayload {
  skin_id: string;
  skin_name: string;
  model_dir: string;
  model3_path: string;
  /** AppData 绝对目录；有则走 convertFileSrc */
  model_abs_dir?: string | null;
  lines: Array<{ text: string; animation?: string | null }>;
  idle_animation?: string | null;
  click_animation?: string | null;
  drag_animation?: string | null;
  boot_animation?: string | null;
  random_animations?: string[];
  random_min_sec?: number;
  random_max_sec?: number;
  animations?: string[];
  touch_areas?: Array<{
    id: string;
    zone: string;
    click_animation?: string | null;
    priority?: number;
    attachments?: string[];
    bounds?: { x: number; y: number; width: number; height: number };
  }>;
}

declare global {
  interface Window {
    PIXI: typeof PIXI;
    Live2DCubismCore?: unknown;
  }
}

window.PIXI = PIXI;
Live2DModel.registerTicker(PIXI.Ticker as never);

const rootEl = document.getElementById("kanmusu-player-root");
const stageHost = document.getElementById("kanmusu-stage");
const bubbleEl = document.getElementById("kanmusu-bubble");
const overlayEl = document.getElementById("kanmusu-hit-overlay") as HTMLCanvasElement | null;
const editBoundsEl = document.getElementById("kanmusu-edit-bounds");
const dragPreviewEl = document.getElementById("kanmusu-drag-preview");
const bootHintEl = document.getElementById("kanmusu-boot-hint");

let pixiApp: PIXI.Application | null = null;
let currentModel: Live2DModel | null = null;
let currentModelDir: string | null = null;
let currentSkinId: string | null = null;
let loadSeq = 0;
/** 与冷开 page_load emit + consume_pending 竞态时去重，避免双份整包加载 */
let loadingSkinKey: string | null = null;
let interactor = new KanmusuInteractor();
let bubbleTimer: ReturnType<typeof setTimeout> | null = null;
let bubbleEnabled = true;
/** 菜单/主窗挡住时暂存，结束后补播 */
let pendingBubble: string | null = null;
let savedUserScale = 0.8;
let savedModelPan = { x: 0, y: 0 };
let scaleSaveTimer = 0;
let panSaveTimer = 0;
let renderPaused = false;
let ignoreCursorActive = true;
let clickThroughInterval = 0;
let clickThroughPollInFlight = false;
/** 稳态穿透时可放慢；命中态与桌宠同级 */
const CLICK_THROUGH_POLL_MS = 100;
const CLICK_THROUGH_POLL_IDLE_MS = 180;
let clickThroughPollMs = CLICK_THROUGH_POLL_MS;
let cachedScaleFactor = 0;
let winGeomAt = 0;
let screenBoundsAt = 0;
let screenBoundsCache: {
  left: number;
  top: number;
  right: number;
  bottom: number;
} | null = null;
const SCREEN_MARGIN = 8;
const EDIT_EDGE_HIT_PX = 14;
const EDIT_OUTSIDE_MARGIN_PX = 12;

function isCursorOutsidePetPhysical(
  cursorX: number,
  cursorY: number,
  winX: number,
  winY: number,
  winW: number,
  winH: number,
  margin = EDIT_OUTSIDE_MARGIN_PX,
): boolean {
  return (
    cursorX < winX - margin ||
    cursorY < winY - margin ||
    cursorX > winX + winW + margin ||
    cursorY > winY + winH + margin
  );
}

function isNearEditResizeEdge(localX: number, localY: number, cssW: number, cssH: number): boolean {
  if (cssW < 1 || cssH < 1) return false;
  return (
    localX <= EDIT_EDGE_HIT_PX ||
    localX >= cssW - EDIT_EDGE_HIT_PX ||
    localY <= EDIT_EDGE_HIT_PX ||
    localY >= cssH - EDIT_EDGE_HIT_PX
  );
}

/** 「编辑范围」：缩放/布置；必须可退出，否则整窗关闭穿透挡桌面 */
let editBoundsMode = false;
/** 本次桌宠会话：显示解包点击区域（默认关，不落库） */
let hitAreasVisible = false;
/** 进入编辑后短暂屏蔽「点空白退出」，避免菜单点击抬起误关 */
let editBoundsSuppressUntil = 0;
let petMenuOpen = false;
let mainWindowVisible = false;
/** 菜单关闭后短暂抑制点击/拖拽，防误触 */
let menuCloseSuppressUntil = 0;
/** 菜单外点关闭：专用左键边沿，与穿透手势解耦 */
let menuDismissLeftWasDown = false;
let pollLeftWasDown = false;
let pollRightWasDown = false;
let pollPointerCapture = false;
const petWindow = getCurrentWindow();
/** 独立「舰娘预览」窗（有边框）；共用 pet 时为桌宠模式 */
const isPreviewShell = petWindow.label === "kanmusu-player";

function setStatus(_text: string) {
  /* desktop companion / preview: no status chrome in DOM */
}

function setBootHint(text: string | null) {
  if (!bootHintEl) return;
  if (!text) {
    bootHintEl.hidden = true;
    bootHintEl.textContent = "";
    return;
  }
  bootHintEl.hidden = false;
  bootHintEl.textContent = text;
}

function setWindowDragPreview(active: boolean) {
  rootEl?.classList.toggle("pet-window-dragging", active);
  dragPreviewEl?.classList.toggle("visible", active);
  dragPreviewEl?.setAttribute("aria-hidden", active ? "false" : "true");
}

function setRenderPaused(paused: boolean) {
  if (renderPaused === paused) return;
  renderPaused = paused;
  try {
    if (paused) {
      PIXI.Ticker.shared.stop();
      pixiApp?.ticker.stop();
    } else {
      PIXI.Ticker.shared.start();
      pixiApp?.ticker.start();
      applyFpsBudget(false);
    }
  } catch {
    /* ignore */
  }
}

/** 空闲降到 30fps；交互/拖拽/编辑时回 60 */
function applyFpsBudget(interactive: boolean) {
  const fps = interactive ? 60 : 30;
  try {
    PIXI.Ticker.shared.maxFPS = fps;
    if (pixiApp) pixiApp.ticker.maxFPS = fps;
  } catch {
    /* ignore */
  }
}

function schedulePersistUserScale(scale: number) {
  savedUserScale = scale;
  if (scaleSaveTimer) window.clearTimeout(scaleSaveTimer);
  scaleSaveTimer = window.setTimeout(() => {
    scaleSaveTimer = 0;
    void invoke("pet_set_scale", { scale }).catch(() => undefined);
  }, 280);
}

async function loadSavedUserScale() {
  try {
    const st = await invoke<{ scale?: number }>("pet_get_status");
    if (typeof st?.scale === "number" && Number.isFinite(st.scale)) {
      savedUserScale = Math.max(0.4, Math.min(1.5, st.scale));
    }
  } catch {
    /* keep default */
  }
}

async function loadSavedLayoutOffset() {
  if (isPreviewShell) {
    savedModelPan = { x: 0, y: 0 };
    return;
  }
  try {
    const cfg = await invoke<{
      offset_x?: number;
      offset_y?: number;
      scale?: number;
    }>("pet_get_config");
    if (typeof cfg?.scale === "number" && Number.isFinite(cfg.scale)) {
      savedUserScale = Math.max(0.4, Math.min(1.5, cfg.scale));
    }
    savedModelPan = {
      x: Number.isFinite(cfg?.offset_x) ? Number(cfg.offset_x) : 0,
      y: Number.isFinite(cfg?.offset_y) ? Number(cfg.offset_y) : 0,
    };
  } catch {
    savedModelPan = { x: 0, y: 0 };
  }
}

function schedulePersistModelPan(pan: { x: number; y: number }) {
  savedModelPan = { x: Math.round(pan.x), y: Math.round(pan.y) };
  if (isPreviewShell) return;
  if (panSaveTimer) window.clearTimeout(panSaveTimer);
  panSaveTimer = window.setTimeout(() => {
    panSaveTimer = 0;
    void persistKanmusuLayout().catch(() => undefined);
  }, 280);
}

function hideBubble() {
  if (!bubbleEl) return;
  bubbleEl.classList.remove("visible");
  bubbleEl.textContent = "";
  if (bubbleTimer) {
    clearTimeout(bubbleTimer);
    bubbleTimer = null;
  }
}

function clearBubble() {
  pendingBubble = null;
  hideBubble();
}

function flushPendingBubbleIfAllowed() {
  if (!pendingBubble || !bubbleEnabled || mainWindowVisible || petMenuOpen || document.hidden) {
    return;
  }
  const next = pendingBubble;
  pendingBubble = null;
  showBubble(next);
}

function showBubble(text: string, animation?: string | null) {
  if (!bubbleEl || !bubbleEnabled) return;
  if (petMenuOpen || mainWindowVisible || document.hidden) {
    pendingBubble = text;
    return;
  }
  bubbleEl.textContent = text;
  bubbleEl.classList.add("visible");
  if (bubbleTimer) clearTimeout(bubbleTimer);
  bubbleTimer = setTimeout(() => {
    hideBubble();
  }, 8000);
  if (animation) interactor.playNamedMotion(animation, false);
}

async function refreshScreenBounds() {
  try {
    const b = await invoke<{ left: number; top: number; right: number; bottom: number }>(
      "pet_get_screen_bounds",
    );
    screenBoundsCache = b;
    interactor.setScreenBounds(b);
    screenBoundsAt = performance.now();
  } catch {
    /* ignore */
  }
}

function clampWindowPosPhysical(
  x: number,
  y: number,
  w: number,
  h: number,
): { x: number; y: number } {
  const b = screenBoundsCache;
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

function ensureCubismCore(): void {
  if (!window.Live2DCubismCore) {
    throw new Error("Cubism Core 未加载，请检查 /vendor/live2dcubismcore.min.js");
  }
}

async function ensureApp(): Promise<PIXI.Application> {
  if (pixiApp) return pixiApp;
  pixiApp = new PIXI.Application({
    resizeTo: window,
    backgroundAlpha: 0,
    backgroundColor: 0x000000,
    // 桌宠小窗：关 MSAA、限制 DPR，显著降低 Cubism 每帧成本
    antialias: false,
    autoDensity: true,
    resolution: Math.min(window.devicePixelRatio || 1, 1.25),
    clearBeforeRender: true,
  });
  const bg = (pixiApp.renderer as unknown as { background?: { alpha: number } }).background;
  if (bg) bg.alpha = 0;
  const canvas = (pixiApp.view ?? (pixiApp as unknown as { canvas?: HTMLCanvasElement }).canvas) as
    | HTMLCanvasElement
    | undefined;
  if (!canvas) throw new Error("Pixi canvas 创建失败");
  canvas.style.background = "transparent";
  stageHost?.replaceChildren(canvas);
  window.addEventListener("resize", () => {
    if (currentModel) refitKeepingUserTransform(currentModel);
  });
  return pixiApp;
}

function stageSize(): { w: number; h: number } {
  if (!pixiApp) return { w: window.innerWidth, h: window.innerHeight };
  return {
    w: pixiApp.screen.width || window.innerWidth,
    h: pixiApp.screen.height || window.innerHeight,
  };
}

function computeFit(model: Live2DModel): { baseScale: number; cx: number; cy: number } {
  const { w, h } = stageSize();
  const bounds = model.getLocalBounds();
  const mw = Math.max(model.width || 0, bounds.width || 0, 1);
  const mh = Math.max(model.height || 0, bounds.height || 0, 1);
  const pad = 0.88;
  let baseScale = Math.min((w * pad) / mw, (h * pad) / mh);
  if (!Number.isFinite(baseScale) || baseScale <= 0) baseScale = 0.2;
  baseScale = Math.max(0.02, Math.min(baseScale, 4));
  // Align with Spine pet: bias toward bottom-center of the cell window
  return { baseScale, cx: w / 2, cy: h * 0.72 };
}

function refitKeepingUserTransform(model: Live2DModel) {
  const fit = computeFit(model);
  interactor.setFitBase(fit.baseScale, fit.cx, fit.cy);
}

function unloadCurrent(app: PIXI.Application, options?: { disposeBlobs?: boolean }) {
  currentSkinId = null;
  interactor.detach();
  if (currentModel) {
    app.stage.removeChild(currentModel as never);
    currentModel.destroy({ children: true });
    currentModel = null;
  }
  if (currentModelDir) {
    // 换皮默认走 LRU 跨皮肤缓存；仅显式销毁时 revoke
    if (options?.disposeBlobs) {
      disposeKanmusuBlobs(currentModelDir);
    } else {
      clearActiveKanmusuResolveMap();
    }
    currentModelDir = null;
  }
  hideBubble();
}

async function applyClickThrough(ignore: boolean, force = false) {
  if (isPreviewShell) {
    // 预览窗需要正常接收鼠标，绝不穿透
    if (ignoreCursorActive || force) {
      try {
        await petWindow.setIgnoreCursorEvents(false);
        ignoreCursorActive = false;
      } catch {
        /* ignore */
      }
    }
    return;
  }
  if (ignore && (petMenuOpen || mainWindowVisible)) return;
  if (ignore && interactor.isCapturingPointer()) return;
  if (ignore && interactor.isHitDebug()) return;
  if (ignore && pollPointerCapture) return;
  if (!force && ignore === ignoreCursorActive) return;
  try {
    await petWindow.setIgnoreCursorEvents(ignore);
    ignoreCursorActive = ignore;
  } catch {
    /* ignore */
  }
}

function stopClickThroughPoll() {
  if (clickThroughInterval) {
    clearInterval(clickThroughInterval);
    clickThroughInterval = 0;
  }
  clickThroughPollInFlight = false;
}

function rescheduleClickThroughPoll(ms: number) {
  if (isPreviewShell || !clickThroughInterval) {
    clickThroughPollMs = ms;
    return;
  }
  if (clickThroughPollMs === ms) return;
  clickThroughPollMs = ms;
  clearInterval(clickThroughInterval);
  clickThroughInterval = window.setInterval(() => {
    void runClickThroughPollTick();
  }, ms);
}

/** 对齐 Spine：菜单打开时，左键在菜单外按下/抬起 → 关闭 */
async function dismissMenuOnOutsideLeftClick() {
  if (!petMenuOpen) return;
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
  const edgeDown = down && !menuDismissLeftWasDown;
  const edgeUp = !down && menuDismissLeftWasDown;
  menuDismissLeftWasDown = down;
  if (Date.now() < menuCloseSuppressUntil) return;
  if (edgeDown && !overMenu) {
    try {
      await invoke("pet_menu_hide");
    } catch {
      /* ignore */
    }
    return;
  }
  if (!edgeUp || overMenu) return;
  try {
    await invoke("pet_menu_hide");
  } catch {
    /* ignore */
  }
}

async function runClickThroughPollTick() {
  if (isPreviewShell || document.hidden) return;
  if (clickThroughPollInFlight) return;
  clickThroughPollInFlight = true;
  try {
    if (mainWindowVisible) {
      await applyClickThrough(false, true);
      applyFpsBudget(false);
      return;
    }
    if (petMenuOpen) {
      await applyClickThrough(false, true);
      applyFpsBudget(false);
      await dismissMenuOnOutsideLeftClick();
      return;
    }
    menuDismissLeftWasDown = false;
    if (Date.now() < menuCloseSuppressUntil) {
      await applyClickThrough(false, true);
      applyFpsBudget(false);
      return;
    }
    if (interactor.isCapturingPointer() || pollPointerCapture) {
      await applyClickThrough(false);
      applyFpsBudget(true);
      rescheduleClickThroughPoll(CLICK_THROUGH_POLL_MS);
      // 拖拽中仍需推进 poll 手势
      if (!pollPointerCapture) return;
    }
    const now = performance.now();
    if (now - screenBoundsAt > 8000) void refreshScreenBounds();
    // 一次 IPC：cursor + 窗体 + 键态 + scale（原先 3～5 次往返）
    const poll = await invoke<{
      cursor_x: number;
      cursor_y: number;
      win_x: number;
      win_y: number;
      win_width: number;
      win_height: number;
      left_down: boolean;
      right_down: boolean;
      scale_factor: number;
    }>("pet_click_through_poll");
    const scale =
      poll.scale_factor > 0
        ? poll.scale_factor
        : cachedScaleFactor > 0
          ? cachedScaleFactor
          : 1;
    cachedScaleFactor = scale;
    winGeomAt = now;
    interactor.refreshCachedWindowPos(poll.win_x, poll.win_y);
    interactor.setDragWindowSize(poll.win_width, poll.win_height);

    const localX = (poll.cursor_x - poll.win_x) / scale;
    const localY = (poll.cursor_y - poll.win_y) / scale;
    const cssW = poll.win_width / scale;
    const cssH = poll.win_height / scale;
    const inWindow =
      localX >= 0 && localY >= 0 && localX <= cssW && localY <= cssH;
    // Touch* 三区 ∪ 整模 AABB：点击/右键/拖窗；缩放仅编辑范围
    const hitTouch = inWindow && interactor.hitInteractiveClient(localX, localY);
    const hit = hitTouch;

    const leftDown =
      inWindow || pollPointerCapture || editBoundsMode ? !!poll.left_down : false;
    const rightDown =
      inWindow || pollPointerCapture || editBoundsMode ? !!poll.right_down : false;
    const leftEdgeDown = leftDown && !pollLeftWasDown;
    const leftEdgeUp = !leftDown && pollLeftWasDown;
    const rightEdgeDown = rightDown && !pollRightWasDown;
    pollLeftWasDown = leftDown;
    pollRightWasDown = rightDown;

    // 编辑范围：整窗可交互；窗外 / 模型外空白 → 退出（对齐 Spine；边沿按下与抬起都认）
    if (editBoundsMode) {
      await applyClickThrough(false, true);
      applyFpsBudget(true);
      const outside = isCursorOutsidePetPhysical(
        poll.cursor_x,
        poll.cursor_y,
        poll.win_x,
        poll.win_y,
        poll.win_width,
        poll.win_height,
      );
      const onModel = inWindow && interactor.hitModelClient(localX, localY);
      const nearEdge = inWindow && isNearEditResizeEdge(localX, localY, cssW, cssH);
      const emptyInside = inWindow && !onModel && !nearEdge;
      if (
        !resizeDragging &&
        Date.now() >= editBoundsSuppressUntil &&
        Date.now() >= menuCloseSuppressUntil
      ) {
        if (outside && (leftEdgeDown || leftEdgeUp)) {
          setEditBoundsMode(false);
        } else if (emptyInside && leftEdgeDown) {
          setEditBoundsMode(false);
        }
      }
      rescheduleClickThroughPoll(CLICK_THROUGH_POLL_MS);
      return;
    }

    // 窗外：不查鼠标按键，减 IPC（对齐卡顿修复目标）
    if (!inWindow && !pollPointerCapture) {
      pollLeftWasDown = false;
      pollRightWasDown = false;
      await applyClickThrough(true);
      applyFpsBudget(false);
      rescheduleClickThroughPoll(CLICK_THROUGH_POLL_IDLE_MS);
      return;
    }

    if (pollPointerCapture) {
      interactor.updatePollGesture(poll.cursor_x, poll.cursor_y);
      if (leftEdgeUp) {
        interactor.endPollGesture(localX, localY);
        pollPointerCapture = false;
      }
      await applyClickThrough(false, true);
      applyFpsBudget(true);
      rescheduleClickThroughPoll(CLICK_THROUGH_POLL_MS);
      return;
    }

    if (ignoreCursorActive && leftEdgeDown && hit) {
      if (Date.now() < menuCloseSuppressUntil) {
        await applyClickThrough(false, true);
        return;
      }
      await applyClickThrough(false, true);
      pollPointerCapture = interactor.beginPollGesture(
        localX,
        localY,
        poll.cursor_x,
        poll.cursor_y,
      );
      applyFpsBudget(true);
      rescheduleClickThroughPoll(CLICK_THROUGH_POLL_MS);
      return;
    }

    if (ignoreCursorActive && rightEdgeDown && hit) {
      if (Date.now() < menuCloseSuppressUntil) {
        await applyClickThrough(false, true);
        return;
      }
      await applyClickThrough(false, true);
      try {
        const open = await invoke<boolean>("pet_menu_toggle_at_cursor");
        // 同步本地状态，避免仅依赖 pet-menu-state 时关菜单路径失效
        if (open) {
          petMenuOpen = true;
          menuCloseSuppressUntil = Date.now() + 400;
          rootEl?.classList.add("menu-open-capture");
          startClickThroughPoll();
        } else {
          petMenuOpen = false;
          rootEl?.classList.remove("menu-open-capture");
        }
      } catch {
        /* ignore */
      }
      rescheduleClickThroughPoll(CLICK_THROUGH_POLL_MS);
      return;
    }

    // 仅 Touch* 关穿透（对齐桌宠）；滚轮缩放进编辑范围后再接收
    await applyClickThrough(!hitTouch);
    applyFpsBudget(hitTouch);
    rescheduleClickThroughPoll(hitTouch ? CLICK_THROUGH_POLL_MS : CLICK_THROUGH_POLL_IDLE_MS);
  } catch {
    /* ignore */
  } finally {
    clickThroughPollInFlight = false;
  }
}

function startClickThroughPoll() {
  if (isPreviewShell) return;
  if (clickThroughInterval) return;
  clickThroughPollMs = CLICK_THROUGH_POLL_MS;
  void runClickThroughPollTick();
  clickThroughInterval = window.setInterval(() => {
    void runClickThroughPollTick();
  }, clickThroughPollMs);
}

async function loadBubbleEnabled() {
  try {
    const v = await invoke<boolean>("pet_get_bubble_enabled");
    bubbleEnabled = v !== false;
  } catch {
    bubbleEnabled = true;
  }
}

async function loadModel(payload: KanmusuPlayerLoadPayload) {
  const skinKey = `${payload.skin_id}|${payload.model_dir}`;
  if (loadingSkinKey === skinKey) return;
  if (
    currentSkinId === payload.skin_id &&
    currentModelDir === payload.model_dir &&
    currentModel
  ) {
    return;
  }
  loadingSkinKey = skinKey;
  const seq = ++loadSeq;
  setBootHint(`加载中：${payload.skin_name}…`);
  setRenderPaused(false);
  try {
    ensureCubismCore();
    const app = await ensureApp();
    if (seq !== loadSeq) return;
    unloadCurrent(app);
    // 设置与布局可与资源 IO 并行，不必挡首屏
    const touchExtras = [
      ...(payload.touch_areas ?? []).map((a) => a.click_animation),
      "touch_head",
      "touch_body",
      "touch_special",
      // 整模点击回落：首包带 1～2 个 main，避免点完再等 deferred
      ...(payload.animations ?? []).filter((a) => /main_\d/i.test(a)).slice(0, 2),
    ]
      .filter((x): x is string => !!x && !!String(x).trim())
      .filter((v, i, arr) => arr.indexOf(v) === i)
      .slice(0, 8);
    const t0 = performance.now();
    const settingsPrefetch = (async () => {
      const model3File = model3FilenameFromPath(payload.model3_path);
      return buildCubismSettings(
        payload.model_dir,
        model3File,
        {
          idle: payload.idle_animation,
          click: payload.click_animation,
          drag: payload.drag_animation,
          boot: payload.boot_animation,
          extra: touchExtras,
        },
        payload.model_abs_dir,
      );
    })();
    await Promise.all([
      loadBubbleEnabled(),
      isPreviewShell ? loadSavedUserScale() : loadSavedLayoutOffset(),
    ]);
    if (seq !== loadSeq) return;

    setBootHint(`读取资源：${payload.skin_name}…`);
    const { settings, warmDeferred } = await settingsPrefetch;
    const tSettings = performance.now() - t0;
    if (seq !== loadSeq) {
      return;
    }
    const tFrom0 = performance.now();
    const model = await Live2DModel.from(settings, {
      autoInteract: false,
      ticker: PIXI.Ticker.shared,
    });
    const tFrom = performance.now() - tFrom0;
    console.info(
      `[kanmusu-player] load ${payload.skin_name}: settings=${Math.round(tSettings)}ms from=${Math.round(tFrom)}ms`,
    );
    if (seq !== loadSeq) {
      model.destroy({ children: true });
      return;
    }
    (model as unknown as { autoUpdate?: boolean }).autoUpdate = true;
    currentModel = model;
    currentModelDir = payload.model_dir;
    currentSkinId = payload.skin_id;
    app.stage.addChild(model as never);
    void warmDeferred(model as never);

    const canvas = app.view as HTMLCanvasElement;
    const fit = computeFit(model);
    interactor.attach(
      canvas,
      model,
      {
        modelDir: payload.model_dir,
        idleAnimation: payload.idle_animation,
        clickAnimation: payload.click_animation,
        defaultClickAnimation: payload.click_animation,
        dragAnimation: payload.drag_animation,
        bootAnimation: payload.boot_animation,
        randomAnimations: payload.random_animations ?? [],
        randomMinSec: payload.random_min_sec ?? 45,
        randomMaxSec: payload.random_max_sec ?? 120,
        animations: payload.animations ?? [],
        touchAreas: (payload.touch_areas ?? []).map((a) => ({
          id: a.id,
          zone: a.zone,
          click_animation: a.click_animation,
          priority: a.priority,
          attachments: a.attachments,
          bounds: a.bounds ?? { x: 0.2, y: 0.2, width: 0.6, height: 0.6 },
        })),
        lines: payload.lines ?? [],
      },
      {
        onStatus: setStatus,
        onLine: (text) => showBubble(text),
        onOpenMain: isPreviewShell
          ? undefined
          : () => {
              void invoke("pet_open_main", { page: null }).catch(() => undefined);
            },
        bubbleEnabled: () => bubbleEnabled,
        isPetMenuOpen: () => petMenuOpen,
        onHideMenu: () => {
          void invoke("pet_menu_hide").catch(() => undefined);
        },
        isMenuCloseSuppressed: () => Date.now() < menuCloseSuppressUntil,
        isEditBoundsMode: () => editBoundsMode,
        onEditBoundsEmptyClick: () => {
          if (
            !editBoundsMode ||
            resizeDragging ||
            Date.now() < editBoundsSuppressUntil ||
            Date.now() < menuCloseSuppressUntil
          ) {
            return;
          }
          setEditBoundsMode(false);
        },
        onDragChange: (dragging) => {
          // 仅普通模式移窗才显示「拖动中」；编辑布置挪模型不提示
          if (!isPreviewShell && !editBoundsMode) setWindowDragPreview(dragging);
        },
        onUserScaleChange: isPreviewShell
          ? undefined
          : (scale) => schedulePersistUserScale(scale),
        onModelPanChange: isPreviewShell
          ? undefined
          : (pan) => schedulePersistModelPan(pan),
      },
      overlayEl,
      isPreviewShell ? 1 : savedUserScale,
      isPreviewShell ? "preview" : "pet",
      isPreviewShell ? { x: 0, y: 0 } : savedModelPan,
    );
    setEditBoundsMode(false);
    interactor.setFitBase(fit.baseScale, fit.cx, fit.cy);
    // 换皮后恢复会话内「显示点击区域」
    interactor.setHitDebug(hitAreasVisible);
    if (!isPreviewShell) void refreshScreenBounds();
    requestAnimationFrame(() => {
      if (currentModel === model && seq === loadSeq) refitKeepingUserTransform(model);
    });
    setTimeout(() => {
      if (currentModel === model && seq === loadSeq) refitKeepingUserTransform(model);
    }, 120);

    document.title = isPreviewShell
      ? `舰娘预览 · ${payload.skin_name}`
      : "小寒桌宠";
    setBootHint(
      isPreviewShell
        ? "点击播动作 · 右键菜单 → 编辑范围可拖移/滚轮缩放"
        : null,
    );
    if (isPreviewShell) {
      window.setTimeout(() => setBootHint(null), 3200);
    }
    void applyClickThrough(isPreviewShell ? false : true, true);
    if (!isPreviewShell) startClickThroughPoll();
    // 空闲预热同角色其它皮肤，减轻菜单来回切的首包成本
    if (!isPreviewShell && seq === loadSeq) {
      scheduleSiblingSkinPrefetch(payload.model_dir, payload.skin_id);
    }
  } catch (e) {
    if (seq !== loadSeq) return;
    console.error("[kanmusu-player] load failed", e);
    const msg = e instanceof Error ? e.message : String(e);
    setBootHint(`加载失败：${msg.slice(0, 80)}`);
  } finally {
    if (seq === loadSeq) loadingSkinKey = null;
  }
}

let siblingPrefetchTimer = 0;
let siblingPrefetchToken = 0;

function scheduleSiblingSkinPrefetch(currentModelDir: string, currentSkinId: string) {
  if (siblingPrefetchTimer) window.clearTimeout(siblingPrefetchTimer);
  const token = ++siblingPrefetchToken;
  siblingPrefetchTimer = window.setTimeout(() => {
    siblingPrefetchTimer = 0;
    void runSiblingSkinPrefetch(currentModelDir, currentSkinId, token);
  }, 1800);
}

async function runSiblingSkinPrefetch(
  currentModelDir: string,
  currentSkinId: string,
  token: number,
) {
  if (token !== siblingPrefetchToken || document.hidden) return;
  try {
    const menu = await invoke<{
      character_id: string;
      skins: Array<{ id: string; model_id: string; model_ready?: boolean }>;
    }>("kanmusu_menu_skins");
    if (token !== siblingPrefetchToken) return;
    const detail = await invoke<{
      skins: Array<{
        id: string;
        model_dir: string;
        model_ready?: boolean;
        model3_path?: string | null;
      }>;
    }>("kanmusu_get_detail", { characterId: menu.character_id });
    if (token !== siblingPrefetchToken) return;
    const siblings = (detail.skins ?? [])
      .filter(
        (s) =>
          s.model_ready &&
          s.id !== currentSkinId &&
          s.model_dir &&
          s.model_dir !== currentModelDir &&
          s.model3_path,
      )
      .slice(0, 3);
    for (const skin of siblings) {
      if (token !== siblingPrefetchToken) return;
      const model3 = model3FilenameFromPath(skin.model3_path!);
      await prefetchKanmusuSkin(skin.model_dir, model3, [
        "idle",
        "touch",
        "login",
        "home",
        "touch_head",
        "touch_body",
      ]);
      // 让出主线程，避免预热卡点击
      await new Promise((r) => window.setTimeout(r, 120));
    }
  } catch {
    /* 预热失败忽略 */
  }
}

type ResizeEdge = "n" | "s" | "e" | "w" | "ne" | "nw" | "se" | "sw";

const EDIT_MIN_W = 160;
const EDIT_MAX_W = 480;
const EDIT_MIN_H = 200;
const EDIT_MAX_H = 600;

let resizeDragging = false;
let resizeEdge: ResizeEdge = "se";
let resizeStart = { x: 0, y: 0, w: 0, h: 0, posX: 0, posY: 0 };
let editResizeScaleFactor = 1;
let resizeRafId = 0;
let pendingResize: { w: number; h: number; x: number; y: number } | null = null;
let resizeApplySerial: Promise<void> = Promise.resolve();
let lastResizeKey = "";
let editResizeWired = false;

function clampEditSizePhysical(w: number, h: number, sf: number) {
  const minW = Math.round(EDIT_MIN_W * sf);
  const maxW = Math.round(EDIT_MAX_W * sf);
  const minH = Math.round(EDIT_MIN_H * sf);
  const maxH = Math.round(EDIT_MAX_H * sf);
  return {
    w: Math.max(minW, Math.min(maxW, Math.round(w))),
    h: Math.max(minH, Math.min(maxH, Math.round(h))),
  };
}

function computeEditResizeBounds(mx: number, my: number) {
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
  const clamped = clampEditSizePhysical(w, h, editResizeScaleFactor);
  if (edge.includes("w")) x = resizeStart.posX + (resizeStart.w - clamped.w);
  if (edge.includes("n")) y = resizeStart.posY + (resizeStart.h - clamped.h);
  const pos = clampWindowPosPhysical(x, y, clamped.w, clamped.h);
  return { w: clamped.w, h: clamped.h, x: pos.x, y: pos.y };
}

function scheduleEditResizeApply(next: { w: number; h: number; x: number; y: number }) {
  pendingResize = next;
  if (resizeRafId) return;
  resizeRafId = requestAnimationFrame(() => {
    resizeRafId = 0;
    const bounds = pendingResize;
    if (!bounds || !resizeDragging) return;
    const key = `${bounds.w}x${bounds.h}@${bounds.x},${bounds.y}`;
    if (key === lastResizeKey) return;
    lastResizeKey = key;
    const edge = resizeEdge;
    const moveX = edge.includes("w");
    const moveY = edge.includes("n");
    resizeApplySerial = resizeApplySerial
      .then(async () => {
        await invoke("pet_set_window_bounds", {
          x: bounds.x,
          y: bounds.y,
          width: bounds.w,
          height: bounds.h,
          move_x: moveX,
          move_y: moveY,
        });
        interactor.refreshCachedWindowPos(bounds.x, bounds.y);
        interactor.setDragWindowSize(bounds.w, bounds.h);
        if (currentModel) refitKeepingUserTransform(currentModel);
      })
      .catch(() => undefined);
  });
}

async function beginEditResize(edge: ResizeEdge, screenX: number, screenY: number) {
  if (isPreviewShell || !editBoundsMode || resizeDragging) return;
  try {
    if (!screenBoundsCache || performance.now() - screenBoundsAt > 2000) {
      await refreshScreenBounds();
    }
    const [bounds, sf] = await Promise.all([
      invoke<{ x: number; y: number; width: number; height: number }>("pet_get_window_bounds"),
      petWindow.scaleFactor(),
    ]);
    editResizeScaleFactor = sf > 0 ? sf : 1;
    resizeEdge = edge;
    resizeStart = {
      x: screenX,
      y: screenY,
      w: bounds.width,
      h: bounds.height,
      posX: bounds.x,
      posY: bounds.y,
    };
    lastResizeKey = "";
    pendingResize = null;
    resizeDragging = true;
    editBoundsSuppressUntil = Date.now() + 1200;
    rootEl?.classList.add("edit-bounds-resizing");
  } catch {
    resizeDragging = false;
  }
}

async function persistKanmusuLayout(bounds?: {
  x: number;
  y: number;
  width: number;
  height: number;
}) {
  if (isPreviewShell) return;
  const b =
    bounds ??
    (await invoke<{ x: number; y: number; width: number; height: number }>(
      "pet_get_window_bounds",
    ));
  const sf = await petWindow.scaleFactor();
  const scale = sf > 0 ? sf : 1;
  const logicalW = Math.max(EDIT_MIN_W, b.width / scale);
  const logicalH = Math.max(EDIT_MIN_H, b.height / scale);
  const pan = interactor.getModelPan();
  savedModelPan = { x: Math.round(pan.x), y: Math.round(pan.y) };
  // 与桌宠一致：尺寸 + 缩放 + 模型偏移一次落库
  await invoke("pet_save_layout", {
    width: logicalW,
    height: logicalH,
    scale: interactor.getUserScale(),
    offsetX: savedModelPan.x,
    offsetY: savedModelPan.y,
    positionWinWidth: b.width,
    positionWinHeight: b.height,
  });
  interactor.refreshCachedWindowPos(b.x, b.y);
  interactor.setDragWindowSize(b.width, b.height);
}

async function endEditResize() {
  if (!resizeDragging) return;
  resizeDragging = false;
  rootEl?.classList.remove("edit-bounds-resizing");
  editBoundsSuppressUntil = Date.now() + 800;
  try {
    await resizeApplySerial;
    const bounds = await invoke<{ x: number; y: number; width: number; height: number }>(
      "pet_get_window_bounds",
    );
    await persistKanmusuLayout(bounds);
    if (currentModel) refitKeepingUserTransform(currentModel);
  } catch {
    /* ignore */
  }
}

function wireEditBoundsResize() {
  if (editResizeWired || isPreviewShell || !editBoundsEl) return;
  editResizeWired = true;
  editBoundsEl.querySelectorAll(".pet-edit-bounds-handle").forEach((handle) => {
    handle.addEventListener(
      "pointerdown",
      (e) => {
        const pe = e as PointerEvent;
        if (!editBoundsMode || pe.button !== 0) return;
        const edge = handle.getAttribute("data-edge") as ResizeEdge | null;
        if (!edge) return;
        pe.preventDefault();
        pe.stopPropagation();
        void beginEditResize(edge, pe.screenX, pe.screenY);
      },
      true,
    );
  });
  window.addEventListener(
    "pointermove",
    (e) => {
      if (!resizeDragging) return;
      scheduleEditResizeApply(computeEditResizeBounds(e.screenX, e.screenY));
    },
    true,
  );
  window.addEventListener(
    "pointerup",
    () => {
      void endEditResize();
    },
    true,
  );
  window.addEventListener(
    "pointercancel",
    () => {
      void endEditResize();
    },
    true,
  );
}

function setHitAreasVisible(on: boolean) {
  hitAreasVisible = on;
  interactor.setHitDebug(on);
  if (on && !isPreviewShell) {
    void applyClickThrough(false, true);
    setBootHint("点击区域（解包 Touch*）· 菜单可关");
    window.setTimeout(() => {
      if (hitAreasVisible && !editBoundsMode) setBootHint(null);
    }, 2200);
  } else if (!editBoundsMode) {
    setBootHint(null);
  }
}

function setEditBoundsMode(on: boolean) {
  if (!on && resizeDragging) {
    void endEditResize();
  }
  editBoundsMode = on;
  // 编辑范围与点击区域 overlay 解耦
  editBoundsEl?.classList.toggle("active", on);
  editBoundsEl?.setAttribute("aria-hidden", on ? "false" : "true");
  if (on) {
    editBoundsSuppressUntil = Date.now() + 800;
    setBootHint(
      isPreviewShell
        ? "编辑范围 · 拖动模型 · 滚轮缩放 · Esc 退出"
        : "编辑范围 · 拖动模型 · 滚轮缩放 · 拖边框改大小 · Esc / 点空白退出",
    );
    applyFpsBudget(true);
    if (!isPreviewShell) void refreshScreenBounds();
  } else {
    setBootHint(null);
    if (!isPreviewShell) {
      void persistKanmusuLayout().catch(() => undefined);
    }
  }
  if (isPreviewShell) {
    void applyClickThrough(false, true);
    return;
  }
  if (on) {
    void applyClickThrough(false, true);
    startClickThroughPoll();
  } else {
    void applyClickThrough(true, true).finally(() => startClickThroughPoll());
    applyFpsBudget(false);
  }
}

function enterEditBoundsMode() {
  // 对齐桌宠：菜单入口只进入，不 toggle（避免菜单连点直接关掉）
  if (!editBoundsMode) setEditBoundsMode(true);
}

async function bootstrap() {
  if (isPreviewShell) {
    document.documentElement.classList.add("kanmusu-preview-shell");
    document.body.classList.add("kanmusu-preview-shell");
    document.documentElement.style.background = "";
    document.body.style.background = "";
    document.title = "舰娘预览";
    void applyClickThrough(false, true);
  } else {
    document.documentElement.style.background = "transparent";
    document.body.style.background = "transparent";
  }

  await listen<KanmusuPlayerLoadPayload>("kanmusu-player-load", (ev) => {
    void loadModel(ev.payload);
  });
  await listen("pet-enter-edit-bounds", () => {
    // 与桌宠「编辑范围」同入口：缩放布置，非热区调试
    enterEditBoundsMode();
  });
  await listen<boolean>("pet-hit-areas-visible", (ev) => {
    setHitAreasVisible(!!ev.payload);
  });
  await listen("pet-sync-click-through", () => {
    if (isPreviewShell || editBoundsMode) return;
    startClickThroughPoll();
  });
  await listen<{ text?: string; animation?: string | null }>("pet-remark", (ev) => {
    if (isPreviewShell) return;
    const text = ev.payload?.text?.trim();
    if (!text) return;
    showBubble(text, ev.payload?.animation);
  });
  await listen<{ animation?: string; loop?: boolean }>("pet-preview-animation", (ev) => {
    if (isPreviewShell) return;
    const animation = ev.payload?.animation?.trim();
    if (!animation) return;
    interactor.playNamedMotion(animation, !!ev.payload?.loop);
  });
  await listen<boolean>("pet-bubble-enabled-changed", (ev) => {
    if (typeof ev.payload === "boolean") {
      bubbleEnabled = ev.payload;
      if (!bubbleEnabled) clearBubble();
    }
  });
  await listen("pet-clear-bubble", () => {
    clearBubble();
  });
  await listen<boolean>("pet-menu-state", (ev) => {
    const wasOpen = petMenuOpen;
    petMenuOpen = !!ev.payload;
    if (petMenuOpen) {
      hideBubble();
      // 右键开菜单后短抑，避免抬起误关；随后靠轮询 / 捕获层关
      menuCloseSuppressUntil = Date.now() + 400;
      rootEl?.classList.add("menu-open-capture");
      if (!isPreviewShell) void applyClickThrough(false, true);
      startClickThroughPoll();
      return;
    }
    rootEl?.classList.remove("menu-open-capture");
    if (wasOpen) {
      menuCloseSuppressUntil = Date.now() + 1800;
      editBoundsSuppressUntil = Math.max(editBoundsSuppressUntil, Date.now() + 1800);
      menuDismissLeftWasDown = false;
      pollLeftWasDown = false;
      void loadBubbleEnabled().then(() => {
        if (!bubbleEnabled) clearBubble();
        else flushPendingBubbleIfAllowed();
      });
    }
    if (!mainWindowVisible && !editBoundsMode) {
      startClickThroughPoll();
    }
  });
  await listen("pet-main-opening", () => {
    if (isPreviewShell) return;
    mainWindowVisible = true;
    hideBubble();
    void applyClickThrough(false, true);
  });
  await listen("pet-main-closed", () => {
    if (isPreviewShell) return;
    mainWindowVisible = false;
    window.setTimeout(() => flushPendingBubbleIfAllowed(), 120);
    if (!petMenuOpen && !editBoundsMode) startClickThroughPoll();
  });
  await listen<boolean>("main-window-visible", (ev) => {
    if (isPreviewShell) return;
    mainWindowVisible = !!ev.payload;
    if (mainWindowVisible) {
      hideBubble();
      void applyClickThrough(false, true);
    } else if (!petMenuOpen && !editBoundsMode) {
      window.setTimeout(() => flushPendingBubbleIfAllowed(), 120);
      startClickThroughPoll();
    }
  });
  await listen("pet-hidden", () => {
    if (isPreviewShell) return;
    setEditBoundsMode(false);
    setWindowDragPreview(false);
    pollPointerCapture = false;
    stopClickThroughPoll();
    setRenderPaused(true);
  });
  await listen("pet-resume", () => {
    if (isPreviewShell) return;
    setRenderPaused(false);
    if (!editBoundsMode) startClickThroughPoll();
  });
  await listen<number>("pet-scale-changed", (ev) => {
    const scale = ev.payload;
    if (typeof scale !== "number" || !Number.isFinite(scale)) return;
    savedUserScale = Math.max(0.4, Math.min(1.5, scale));
    interactor.setUserScale(savedUserScale, false);
  });

  window.addEventListener("keydown", (e) => {
    if (e.key === "Escape" && editBoundsMode) {
      e.preventDefault();
      setEditBoundsMode(false);
    }
  });
  // 菜单打开：点窗体任意处（含透明空白）关菜单
  rootEl?.addEventListener(
    "pointerdown",
    (e) => {
      if (!petMenuOpen || e.button !== 0) return;
      if (Date.now() < menuCloseSuppressUntil) return;
      e.preventDefault();
      void invoke("pet_menu_hide").catch(() => undefined);
    },
    true,
  );
  document.addEventListener("visibilitychange", () => {
    if (document.hidden) {
      setRenderPaused(true);
      return;
    }
    // 预览/桌宠：从全屏或其它应用切回后必须恢复渲染（此前预览永不 unpause）
    setRenderPaused(false);
    if (!isPreviewShell && !editBoundsMode) startClickThroughPoll();
  });
  void petWindow.listen("tauri://focus", () => {
    setRenderPaused(false);
    if (!isPreviewShell && !editBoundsMode && !petMenuOpen && !mainWindowVisible) {
      startClickThroughPoll();
    }
  });
  void petWindow.listen("tauri://blur", () => {
    // 预览窗失焦很常见（切全屏应用），勿因此退出点击区域
    if (isPreviewShell) return;
    // 菜单关闭后短暂 blur 常见；加 suppress 避免误关点击区域
    if (
      editBoundsMode &&
      Date.now() >= menuCloseSuppressUntil &&
      Date.now() >= editBoundsSuppressUntil
    ) {
      setEditBoundsMode(false);
    }
  });

  await loadBubbleEnabled();
  wireEditBoundsResize();
  if (!isPreviewShell) void refreshScreenBounds();
  applyFpsBudget(false);
  setBootHint(isPreviewShell ? "选择皮肤后加载舰娘…" : "舰娘加载中…");

  const pending = await invoke<KanmusuPlayerLoadPayload | null>(
    "kanmusu_player_consume_pending",
  );
  if (pending) {
    void loadModel(pending);
  }
}

void bootstrap();
