import type { Live2DModel } from "pixi-live2d-display-lipsyncpatch/cubism4";
import { tauriInvoke as invoke } from "../lib/tauriInvoke";
import { ensureKanmusuAssetReady } from "./kanmusuAssets";

export interface TouchAreaBound {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface TouchArea {
  id: string;
  zone: string;
  click_animation?: string | null;
  priority?: number;
  attachments?: string[];
  bounds: TouchAreaBound;
}

export interface InteractMeta {
  /** 当前皮肤 model_dir（点击补拉 motion 用） */
  modelDir?: string | null;
  idleAnimation?: string | null;
  clickAnimation?: string | null;
  defaultClickAnimation?: string | null;
  dragAnimation?: string | null;
  bootAnimation?: string | null;
  randomAnimations?: string[];
  randomMinSec?: number;
  randomMaxSec?: number;
  animations: string[];
  touchAreas: TouchArea[];
  lines: Array<{ text: string; animation?: string | null }>;
}

/** pet = 桌宠透明窗；preview = 独立「舰娘预览」有边框窗 */
export type KanmusuShell = "pet" | "preview";

export interface InteractCallbacks {
  onStatus?: (text: string) => void;
  onLine?: (text: string, animation?: string | null) => void;
  onOpenMain?: () => void;
  /** When false, skip bubble (pet_bubble_enabled off). */
  bubbleEnabled?: () => boolean;
  /** True while pet-menu is open — mousedown should dismiss first. */
  isPetMenuOpen?: () => boolean;
  /** Hide pet-menu (same as Spine stage mousedown while menu open). */
  onHideMenu?: () => void;
  /** True briefly after menu close — ignore stray click/drag. */
  isMenuCloseSuppressed?: () => boolean;
  /** 「编辑范围」：与桌宠一致，滚轮缩放 / 预览平移仅此时允许。 */
  isEditBoundsMode?: () => boolean;
  /** 编辑中点模型外空白 → 退出编辑（对齐 Spine click-empty）。 */
  onEditBoundsEmptyClick?: () => void;
  /** Window-drag visual/feedback (preview overlay). */
  onDragChange?: (dragging: boolean) => void;
  /** User scale changed (wheel); persist via pet_set_scale. */
  onUserScaleChange?: (scale: number) => void;
  /** 编辑范围拖模型偏移结束后持久化（对齐桌宠 offset）。 */
  onModelPanChange?: (pan: { x: number; y: number }) => void;
}

const CLICK_MAX_MS = 280;
const DRAG_THRESHOLD = 10;
/** 与 Spine `pet_scale` 同区间，设置页滑条可共用 */
const SCALE_MIN = 0.4;
const SCALE_MAX = 1.5;
const SCALE_STEP = 0.05;
const DOUBLE_CLICK_MS = 500;
const DOUBLE_CLICK_DIST = 28;
const HIT_PAD = 4;
const SCREEN_MARGIN = 8;

export interface ScreenBounds {
  left: number;
  top: number;
  right: number;
  bottom: number;
}

type PointerState = {
  x: number;
  y: number;
  screenX: number;
  screenY: number;
  clientX: number;
  clientY: number;
  time: number;
  winX: number;
  winY: number;
  /** 按下时的模型平移起点（编辑模式 / 预览） */
  panX: number;
  panY: number;
} | null;

/**
 * 舰娘交互：
 * - pet：拖 OS 窗 / Touch* 热区穿透 / 双击开主窗 / 右键菜单（对齐桌宠）
 * - preview：拖平移模型 / 点播动作 / 滚轮缩放（不写桌宠位置与缩放）
 */
export class KanmusuInteractor {
  private model: Live2DModel | null = null;
  private meta: InteractMeta = {
    animations: [],
    touchAreas: [],
    lines: [],
  };
  private callbacks: InteractCallbacks = {};
  private shell: KanmusuShell = "pet";
  private baseScale = 1;
  private userScale = 1;
  private fitCenter = { x: 0, y: 0 };
  /** 仅预览：模型相对 fitCenter 的平移 */
  private modelPan = { x: 0, y: 0 };
  private pointer: PointerState = null;
  private dragging = false;
  private clickBusy = false;
  private clickBusyToken = 0;
  private clickBusyTimer = 0;
  /** 拖动中播过 drag 动作；松手后回 idle */
  private dragMotionActive = false;
  private canvas: HTMLCanvasElement | null = null;
  private overlay: HTMLCanvasElement | null = null;
  private hitDebug = false;
  private overlayRaf = 0;
  private unbind: (() => void) | null = null;
  private lastHitLabel: string | null = null;
  private lastClickAt = 0;
  private lastClickX = 0;
  private lastClickY = 0;
  private lastDispatchedClickAt = 0;
  private lastMainOpenAt = 0;
  private captureActive = false;
  private initialUserScale = 1;
  private cachedWinPos: { x: number; y: number } | null = null;
  private dragAnchorReady = false;
  private dragPositionRaf = 0;
  private pendingDragPos: { x: number; y: number } | null = null;
  private screenBounds: ScreenBounds | null = null;
  private dragWindowPhysW = 220;
  private dragWindowPhysH = 280;
  private pollGesture = false;
  /** Touch* 并集热区缓存；穿透轮询绝不能每次 24² 网格采样 */
  private touchUnionCache: { x: number; y: number; w: number; h: number } | null = null;
  private touchUnionDirty = true;
  private touchCacheRebuildTimer = 0;
  private areaBoxCache = new Map<string, { x: number; y: number; w: number; h: number }>();
  private overlayLastDraw = 0;
  private randomTimer = 0;

  /** True while user is dragging the OS window or holding pointer. */
  isCapturingPointer(): boolean {
    return this.captureActive || this.dragging || this.pointer != null;
  }

  isHitDebug(): boolean {
    return this.hitDebug;
  }

  private isPreview(): boolean {
    return this.shell === "preview";
  }

  attach(
    canvas: HTMLCanvasElement,
    model: Live2DModel,
    meta: InteractMeta,
    callbacks: InteractCallbacks,
    overlay?: HTMLCanvasElement | null,
    initialUserScale = 1,
    shell: KanmusuShell = "pet",
    initialPan: { x: number; y: number } = { x: 0, y: 0 },
  ) {
    this.detach();
    this.canvas = canvas;
    this.overlay = overlay ?? null;
    this.model = model;
    this.meta = meta;
    this.callbacks = callbacks;
    this.shell = shell;
    this.modelPan = {
      x: Math.round(initialPan.x || 0),
      y: Math.round(initialPan.y || 0),
    };
    this.initialUserScale = Math.max(
      SCALE_MIN,
      Math.min(SCALE_MAX, Number.isFinite(initialUserScale) ? initialUserScale : 1),
    );
    this.userScale = this.initialUserScale;
    this.clickBusy = false;
    this.clickBusyToken += 1;
    this.clearClickBusyTimer();
    this.dragMotionActive = false;
    this.lastHitLabel = null;
    this.captureActive = false;
    this.invalidateTouchCache();

    const onDown = (e: PointerEvent) => {
      if (e.button !== 0 || !this.model) return;
      if (this.pollGesture || this.pointer) return;
      if (this.callbacks.isPetMenuOpen?.()) {
        e.preventDefault();
        this.callbacks.onHideMenu?.();
        return;
      }
      if (this.callbacks.isMenuCloseSuppressed?.()) return;
      const pt = this.toLocal(e);
      const editing = !!this.callbacks.isEditBoundsMode?.();
      if (editing) {
        // 编辑：只在模型上拖偏移；点空白立刻退出（勿仅依赖穿透轮询）
        if (!this.hitModelLocal(pt.x, pt.y)) {
          this.callbacks.onEditBoundsEmptyClick?.();
          return;
        }
      } else if (this.isPreview()) {
        if (!this.hitModelLocal(pt.x, pt.y)) return;
      } else if (!this.hitInteractiveLocal(pt.x, pt.y)) {
        return;
      }
      e.preventDefault();
      canvas.setPointerCapture(e.pointerId);
      canvas.classList.add("is-grabbing");
      this.beginPointerCore(pt.x, pt.y, e.screenX, e.screenY, e.clientX, e.clientY);
      if (!this.isPreview() && !this.callbacks.isEditBoundsMode?.()) {
        void this.refreshAnchorFromWindow();
      }
    };

    const onMove = (e: PointerEvent) => {
      if (this.pollGesture) return;
      this.updatePointerCore(e.screenX, e.screenY, e.clientX, e.clientY);
    };

    const finishPointer = (e: PointerEvent) => {
      if (this.pollGesture) return;
      canvas.classList.remove("is-grabbing");
      try {
        canvas.releasePointerCapture(e.pointerId);
      } catch {
        /* ignore */
      }
      const pt = this.toLocal(e);
      this.endPointerCore(pt.x, pt.y);
    };

    const onWheel = (e: WheelEvent) => {
      if (!this.model) return;
      // 与桌宠一致：仅「编辑范围」内滚轮缩放
      if (!this.callbacks.isEditBoundsMode?.()) return;
      const pt = this.toLocal(e);
      if (this.isPreview()) {
        if (!this.hitModelLocal(pt.x, pt.y)) return;
      } else if (
        !this.hitInteractiveLocal(pt.x, pt.y) &&
        !this.hitModelLocal(pt.x, pt.y)
      ) {
        return;
      }
      e.preventDefault();
      const delta = e.deltaY > 0 ? -SCALE_STEP : SCALE_STEP;
      this.setUserScale(this.userScale + delta, !this.isPreview());
    };

    const onContext = (e: MouseEvent) => {
      // 预览/桌宠：右键打开；已开则关闭（对齐桌宠菜单）
      if (this.callbacks.isPetMenuOpen?.()) {
        e.preventDefault();
        this.callbacks.onHideMenu?.();
        return;
      }
      const pt = this.toLocal(e);
      const editing = !!this.callbacks.isEditBoundsMode?.();
      if (this.isPreview()) {
        if (!this.hitModelLocal(pt.x, pt.y) && !editing) return;
      } else if (!this.hitInteractiveLocal(pt.x, pt.y) && !editing) {
        return;
      }
      e.preventDefault();
      void invoke("pet_menu_toggle_at_cursor").catch(() => undefined);
    };

    canvas.addEventListener("pointerdown", onDown);
    canvas.addEventListener("pointermove", onMove);
    canvas.addEventListener("pointerup", finishPointer);
    canvas.addEventListener("pointercancel", finishPointer);
    canvas.addEventListener("wheel", onWheel, { passive: false });
    canvas.addEventListener("contextmenu", onContext);
    canvas.classList.add("kanmusu-interactive");

    this.unbind = () => {
      canvas.removeEventListener("pointerdown", onDown);
      canvas.removeEventListener("pointermove", onMove);
      canvas.removeEventListener("pointerup", finishPointer);
      canvas.removeEventListener("pointercancel", finishPointer);
      canvas.removeEventListener("wheel", onWheel);
      canvas.removeEventListener("contextmenu", onContext);
      canvas.classList.remove("kanmusu-interactive", "is-grabbing");
    };

    // 优先采用 Cubism model3 HitAreas（系统 Touch*），不用自绘 bounds 冒充
    this.hydrateTouchAreasFromHitAreas();
    this.startLifeMotions();
    // 可点范围用模型 AABB（O(1)）；三区精确命中靠 hitTest，不再栅格采样阻塞首帧
    this.ensureTouchUnionCache(true);
    this.syncOverlayLoop();
  }

  private clearRandomTimer() {
    if (this.randomTimer) {
      window.clearTimeout(this.randomTimer);
      this.randomTimer = 0;
    }
  }

  private startLifeMotions() {
    this.clearRandomTimer();
    const boot = this.meta.bootAnimation?.trim();
    const idle = this.meta.idleAnimation;
    const playIdle = () => {
      // FORCE 应用 idle，确保 PartOpacity 切到正确套件（避免多套部件同时全亮）
      if (idle) void this.tryPlayMotion(idle, true, { force: true });
      this.restartRandomScheduler();
    };
    if (boot && boot.toLowerCase() !== idle?.toLowerCase()) {
      void this.tryPlayMotion(boot, false, {
        onFinish: playIdle,
      }).then((ok) => {
        if (!ok) playIdle();
      });
      return;
    }
    playIdle();
  }

  private randomDelayMs(): number {
    const min = Math.max(8, this.meta.randomMinSec ?? 45) * 1000;
    const max = Math.max(min / 1000, this.meta.randomMaxSec ?? 120) * 1000;
    return min + Math.random() * Math.max(0, max - min);
  }

  private restartRandomScheduler() {
    this.clearRandomTimer();
    if (this.isPreview()) return;
    const pool = this.meta.randomAnimations?.filter((a) => a.trim()) ?? [];
    if (!pool.length || this.clickBusy) return;
    this.randomTimer = window.setTimeout(() => this.playRandomExtra(), this.randomDelayMs());
  }

  private playRandomExtra() {
    this.randomTimer = 0;
    const pool = this.meta.randomAnimations?.filter((a) => a.trim()) ?? [];
    if (
      !pool.length ||
      this.clickBusy ||
      this.dragging ||
      this.callbacks.isEditBoundsMode?.() ||
      this.callbacks.isPetMenuOpen?.()
    ) {
      this.restartRandomScheduler();
      return;
    }
    const name = pool[Math.floor(Math.random() * pool.length)];
    void this.tryPlayMotion(name, false, {
      onFinish: () => {
        if (this.meta.idleAnimation) {
          void this.tryPlayMotion(this.meta.idleAnimation, true, { force: true });
        }
        this.restartRandomScheduler();
      },
    }).then((ok) => {
      if (!ok) this.restartRandomScheduler();
    });
  }

  /** 以 internalModel.hitAreas / settings.hitAreas 为准合并三区配置 */
  private hydrateTouchAreasFromHitAreas() {
    const names = this.readSystemHitAreaNames();
    if (!names.length) return;
    const existing = new Map(
      this.meta.touchAreas.map((a) => [a.id.toLowerCase(), a] as const),
    );
    const touchNames = names.filter((n) => /touch/i.test(n));
    const use = touchNames.length ? touchNames : names;
    const merged: TouchArea[] = [];
    for (const id of use) {
      const prev = existing.get(id.toLowerCase());
      const zone = this.zoneFromName(id);
      const logic =
        zone === "special"
          ? "touch_special"
          : zone === "head"
            ? "touch_head"
            : "touch_body";
      merged.push({
        id,
        zone: prev?.zone || zone,
        attachments: [id],
        priority:
          typeof prev?.priority === "number"
            ? prev.priority
            : this.zonePriority(zone),
        click_animation: prev?.click_animation || logic,
        bounds: { x: 0, y: 0, width: 0, height: 0 },
      });
    }
    if (merged.length) this.meta.touchAreas = merged;
  }

  private readSystemHitAreaNames(): string[] {
    const model = this.model as unknown as {
      internalModel?: {
        hitAreas?: Record<string, { id?: string; name?: string }>;
        settings?: { hitAreas?: Array<{ Id?: string; Name?: string; id?: string; name?: string }> };
      };
    } | null;
    const im = model?.internalModel;
    if (!im) return [];
    const out: string[] = [];
    if (im.hitAreas && typeof im.hitAreas === "object") {
      for (const key of Object.keys(im.hitAreas)) {
        const v = im.hitAreas[key];
        const id = (v?.name || v?.id || key || "").trim();
        if (id) out.push(id);
      }
    }
    const list = im.settings?.hitAreas;
    if (Array.isArray(list)) {
      for (const h of list) {
        const id = String(h?.Id || h?.Name || h?.id || h?.name || "").trim();
        if (id) out.push(id);
      }
    }
    return [...new Set(out)];
  }

  private zoneFromName(name: string): string {
    const n = name.toLowerCase();
    if (n.includes("special")) return "special";
    if (n.includes("head")) return "head";
    if (n.includes("body")) return "body";
    return "body";
  }

  /** 设置页试听 / 定时台词附带动画 */
  playNamedMotion(name: string, loop = false) {
    if (!name.trim()) return;
    if (loop) {
      void this.tryPlayMotion(name, true);
      return;
    }
    void this.tryPlayMotion(name, false, {
      onFinish: () => {
        if (this.meta.idleAnimation) {
          void this.tryPlayMotion(this.meta.idleAnimation, true, { force: true });
        }
      },
    });
  }

  setHitDebug(enabled: boolean) {
    this.hitDebug = enabled;
    if (this.overlay) this.overlay.hidden = !enabled;
    if (enabled) {
      this.areaBoxCache.clear();
      this.ensureTouchUnionCache(true);
      this.drawHitOverlay(true);
      this.syncOverlayLoop();
    } else if (this.overlayRaf) {
      window.clearInterval(this.overlayRaf);
      this.overlayRaf = 0;
      const ctx = this.overlay?.getContext("2d");
      if (ctx && this.overlay) ctx.clearRect(0, 0, this.overlay.width, this.overlay.height);
    }
  }

  toggleHitDebug(): boolean {
    this.setHitDebug(!this.hitDebug);
    return this.hitDebug;
  }

  detach() {
    this.unbind?.();
    this.unbind = null;
    this.clearRandomTimer();
    if (this.overlayRaf) {
      window.clearInterval(this.overlayRaf);
      this.overlayRaf = 0;
    }
    if (this.dragPositionRaf) {
      cancelAnimationFrame(this.dragPositionRaf);
      this.dragPositionRaf = 0;
    }
    if (this.touchCacheRebuildTimer) {
      window.clearTimeout(this.touchCacheRebuildTimer);
      this.touchCacheRebuildTimer = 0;
    }
    this.clickBusyToken += 1;
    this.clearClickBusyTimer();
    this.clickBusy = false;
    this.dragMotionActive = false;
    this.pendingDragPos = null;
    this.model = null;
    this.canvas = null;
    this.overlay = null;
    this.pointer = null;
    this.dragging = false;
    this.dragAnchorReady = false;
    this.captureActive = false;
    this.pollGesture = false;
    this.touchUnionDirty = true;
    this.touchUnionCache = null;
    this.areaBoxCache.clear();
  }

  /** Warm cache so the next drag can start without waiting on IPC. */
  refreshCachedWindowPos(x: number, y: number) {
    this.cachedWinPos = { x, y };
  }

  setScreenBounds(bounds: ScreenBounds | null) {
    this.screenBounds = bounds;
  }

  setDragWindowSize(width: number, height: number) {
    if (width > 0) this.dragWindowPhysW = width;
    if (height > 0) this.dragWindowPhysH = height;
  }

  /**
   * 穿透态下由 OS 鼠标轮询拉起的手势：在 ignore-cursor 时 DOM 收不到 pointerdown。
   */
  beginPollGesture(localX: number, localY: number, screenX: number, screenY: number): boolean {
    if (!this.model || this.pointer) return false;
    if (this.callbacks.isPetMenuOpen?.() || this.callbacks.isMenuCloseSuppressed?.()) return false;
    if (!this.hitInteractiveLocal(localX, localY)) return false;
    this.pollGesture = true;
    this.canvas?.classList.add("is-grabbing");
    this.beginPointerCore(localX, localY, screenX, screenY);
    void this.refreshAnchorFromWindow();
    return true;
  }

  updatePollGesture(screenX: number, screenY: number) {
    if (!this.pollGesture) return;
    this.updatePointerCore(screenX, screenY);
  }

  endPollGesture(localX: number, localY: number) {
    if (!this.pollGesture) return;
    this.canvas?.classList.remove("is-grabbing");
    this.endPointerCore(localX, localY);
    this.pollGesture = false;
  }

  private beginPointerCore(
    localX: number,
    localY: number,
    screenX: number,
    screenY: number,
    clientX = screenX,
    clientY = screenY,
  ) {
    this.captureActive = true;
    this.dragging = false;
    const cached = this.cachedWinPos;
    const editing = !!this.callbacks.isEditBoundsMode?.();
    // 编辑布置只挪模型，不依赖 OS 窗锚点
    this.dragAnchorReady = editing || this.isPreview() ? true : cached != null;
    this.pointer = {
      x: localX,
      y: localY,
      screenX,
      screenY,
      clientX,
      clientY,
      time: Date.now(),
      winX: cached?.x ?? 0,
      winY: cached?.y ?? 0,
      panX: this.modelPan.x,
      panY: this.modelPan.y,
    };
  }

  private updatePointerCore(
    screenX: number,
    screenY: number,
    clientX = screenX,
    clientY = screenY,
  ) {
    if (!this.pointer || !this.model) return;
    const editing = !!this.callbacks.isEditBoundsMode?.();
    // 模型平移用 CSS 坐标，移窗仍用屏幕物理坐标
    const panDx = clientX - this.pointer.clientX;
    const panDy = clientY - this.pointer.clientY;
    const winDx = screenX - this.pointer.screenX;
    const winDy = screenY - this.pointer.screenY;
    const moveDist = editing || this.isPreview()
      ? Math.hypot(panDx, panDy)
      : Math.hypot(winDx, winDy);
    if (!this.dragging && moveDist >= DRAG_THRESHOLD) {
      // 预览普通模式：禁止平移，仅吞掉成「非点击」避免误移模型
      if (this.isPreview() && !editing) {
        this.dragging = true;
        this.clickBusyToken += 1;
        this.clearClickBusyTimer();
        this.clickBusy = false;
        return;
      }
      this.dragging = true;
      // 拖动打断点击忙态，避免松手后延迟回 idle / 吞点击
      this.clickBusyToken += 1;
      this.clearClickBusyTimer();
      this.clickBusy = false;
      // 编辑布置：不提示「拖动窗」；不播 drag 动作
      if (!editing) {
        this.callbacks.onDragChange?.(true);
        if (this.meta.dragAnimation && !this.dragMotionActive) {
          this.dragMotionActive = true;
          void this.tryPlayMotion(this.meta.dragAnimation, false);
        }
      }
    }
    if (!this.dragging) return;
    // 编辑范围：拖的是模型，不是 OS 窗（对齐桌宠 stageOffset）
    if (editing) {
      this.modelPan = {
        x: this.pointer.panX + panDx,
        y: this.pointer.panY + panDy,
      };
      this.clampModelPan();
      this.applyTransform();
      return;
    }
    if (this.isPreview()) return;
    if (this.dragAnchorReady) {
      const next = this.clampWindowPosition(
        this.pointer.winX + winDx,
        this.pointer.winY + winDy,
      );
      this.scheduleDragPosition(next.x, next.y);
    }
  }

  private endPointerCore(localX: number, localY: number) {
    if (!this.pointer) {
      this.captureActive = false;
      return;
    }
    const start = this.pointer;
    const wasDrag = this.dragging;
    const editing = !!this.callbacks.isEditBoundsMode?.();
    this.pointer = null;
    this.dragging = false;
    this.dragAnchorReady = false;
    this.captureActive = false;
    if (!this.isPreview() && !editing) this.flushPendingDragPosition();
    if (wasDrag) {
      this.dragMotionActive = false;
      if (!editing) this.callbacks.onDragChange?.(false);
      if (editing || this.isPreview()) {
        // 布置模型：落库 offset，不挪窗
        this.callbacks.onModelPanChange?.({
          x: this.modelPan.x,
          y: this.modelPan.y,
        });
        return;
      }
      if (this.meta.idleAnimation) {
        // FORCE 打断 drag / 残留 touch，立刻回 idle
        void this.tryPlayMotion(this.meta.idleAnimation, true, { force: true });
      }
      void this.saveWindowPosition();
      return;
    }
    if (Date.now() - start.time > CLICK_MAX_MS) return;
    // 预览单击：整模可点（命中 Touch* 播对应动作，否则 default）
    if (this.isPreview()) {
      if (!this.hitModelLocal(localX, localY)) return;
    } else if (!this.hitInteractiveLocal(localX, localY) && !editing) {
      return;
    }
    this.dispatchStageClick(localX, localY);
  }

  /** 单击即时；桌宠二次点击 → 开主窗；预览每击都播动作 */
  private dispatchStageClick(x: number, y: number) {
    const now = Date.now();
    const isDouble =
      now - this.lastClickAt <= DOUBLE_CLICK_MS &&
      Math.hypot(x - this.lastClickX, y - this.lastClickY) <= DOUBLE_CLICK_DIST;
    if (!isDouble && now - this.lastDispatchedClickAt < 80) return;
    this.lastDispatchedClickAt = now;
    this.lastClickAt = now;
    this.lastClickX = x;
    this.lastClickY = y;
    if (isDouble && !this.isPreview()) {
      this.openMainFromDoubleClick();
      return;
    }
    void this.handleClick(x, y);
  }

  private openMainFromDoubleClick() {
    const now = Date.now();
    if (now - this.lastMainOpenAt < 400) return;
    this.lastMainOpenAt = now;
    if (this.callbacks.onOpenMain) {
      this.callbacks.onOpenMain();
      return;
    }
    void invoke("pet_open_main", { page: null }).catch(() => undefined);
  }

  private async refreshAnchorFromWindow() {
    try {
      const bounds = await invoke<{ x: number; y: number; width: number; height: number }>(
        "pet_get_window_bounds",
      );
      this.cachedWinPos = { x: bounds.x, y: bounds.y };
      this.setDragWindowSize(bounds.width, bounds.height);
      if (!this.pointer) return;
      const wasReady = this.dragAnchorReady;
      if (!wasReady || !this.dragging) {
        this.pointer = {
          ...this.pointer,
          winX: bounds.x,
          winY: bounds.y,
        };
      }
      this.dragAnchorReady = true;
    } catch {
      /* keep cache */
    }
  }

  private clampWindowPosition(x: number, y: number): { x: number; y: number } {
    const b = this.screenBounds;
    const w = this.dragWindowPhysW;
    const h = this.dragWindowPhysH;
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

  private async saveWindowPosition() {
    try {
      const bounds = await invoke<{ x: number; y: number; width: number; height: number }>(
        "pet_get_window_bounds",
      );
      const saved = await invoke<{ x: number; y: number }>("pet_save_position", {
        x: bounds.x,
        y: bounds.y,
        win_width: bounds.width,
        win_height: bounds.height,
      });
      this.cachedWinPos = { x: saved.x, y: saved.y };
      this.setDragWindowSize(bounds.width, bounds.height);
      if (saved.x !== bounds.x || saved.y !== bounds.y) {
        await invoke("pet_move_noactivate", { x: saved.x, y: saved.y });
      }
    } catch {
      /* ignore */
    }
  }

  private invalidateTouchCache() {
    // 立刻清 area 盒，避免编辑平移后调试框/穿透仍用旧矩形；union 短时保留旧值
    this.touchUnionDirty = true;
    this.areaBoxCache.clear();
    if (this.touchCacheRebuildTimer) window.clearTimeout(this.touchCacheRebuildTimer);
    this.touchCacheRebuildTimer = window.setTimeout(() => {
      this.touchCacheRebuildTimer = 0;
      this.touchUnionDirty = true;
      this.ensureTouchUnionCache(true);
      if (this.hitDebug) this.drawHitOverlay(true);
    }, 80);
  }

  private scheduleDragPosition(x: number, y: number) {
    this.pendingDragPos = { x, y };
    if (this.dragPositionRaf) return;
    this.dragPositionRaf = requestAnimationFrame(() => {
      this.dragPositionRaf = 0;
      this.flushPendingDragPosition();
    });
  }

  private flushPendingDragPosition() {
    const next = this.pendingDragPos;
    this.pendingDragPos = null;
    if (!next) return;
    this.cachedWinPos = { x: next.x, y: next.y };
    // 勿用 Tauri setPosition：会激活窗口，全屏游戏下易把任务栏顶出来
    void invoke("pet_move_noactivate", { x: next.x, y: next.y }).catch(() => undefined);
  }

  setFitBase(baseScale: number, centerX: number, centerY: number) {
    const prev = this.fitCenter;
    this.baseScale = baseScale;
    // 窗口改大小 / refit 时补偿 fitCenter 位移，避免模型跟着跳动
    if ((prev.x !== 0 || prev.y !== 0) && (prev.x !== centerX || prev.y !== centerY)) {
      this.modelPan = {
        x: this.modelPan.x + (prev.x - centerX),
        y: this.modelPan.y + (prev.y - centerY),
      };
    }
    this.fitCenter = { x: centerX, y: centerY };
    this.clampModelPan();
    this.invalidateTouchCache();
    this.applyTransform();
  }

  private clampModelPan() {
    const canvas = this.canvas;
    const w = canvas?.clientWidth || window.innerWidth || 220;
    const h = canvas?.clientHeight || window.innerHeight || 280;
    const maxX = Math.max(40, w * 0.45);
    const maxY = Math.max(40, h * 0.45);
    this.modelPan = {
      x: Math.round(Math.max(-maxX, Math.min(maxX, this.modelPan.x))),
      y: Math.round(Math.max(-maxY, Math.min(maxY, this.modelPan.y))),
    };
  }

  getUserScale(): number {
    return this.userScale;
  }

  getModelPan(): { x: number; y: number } {
    return { x: this.modelPan.x, y: this.modelPan.y };
  }

  setModelPan(x: number, y: number, emit = false) {
    this.modelPan = { x: Math.round(x), y: Math.round(y) };
    this.clampModelPan();
    this.applyTransform();
    if (emit) this.callbacks.onModelPanChange?.(this.getModelPan());
  }

  setUserScale(scale: number, emit = false) {
    const next =
      Math.round(Math.max(SCALE_MIN, Math.min(SCALE_MAX, scale)) * 100) / 100;
    if (next === this.userScale) {
      if (emit) this.callbacks.onUserScaleChange?.(next);
      return;
    }
    this.userScale = next;
    this.applyTransform();
    this.callbacks.onStatus?.(
      this.isPreview()
        ? `缩放 ${this.userScale.toFixed(2)}× · 编辑范围可拖移`
        : `缩放 ${this.userScale.toFixed(2)}× · 普通拖窗 / 编辑拖模型`,
    );
    if (emit) this.callbacks.onUserScaleChange?.(this.userScale);
  }

  /** Client coords: true if pointer should capture (Touch* hit or bubble/debug). */
  hitInteractiveClient(clientX: number, clientY: number): boolean {
    const canvas = this.canvas;
    if (!canvas) return false;
    // 舞台铺满窗口时跳过 getBoundingClientRect（穿透轮询热路径）
    // 调试态也不整窗吃点击：仍只认 Touch* 并集
    return this.hitInteractiveLocal(clientX, clientY);
  }

  /** 编辑模式：点在模型包围盒外视为「点空白」可退出 */
  hitModelClient(clientX: number, clientY: number): boolean {
    return this.hitModelLocal(clientX, clientY);
  }

  private hitModelLocal(localX: number, localY: number): boolean {
    if (!this.model) return false;
    const b = this.model.getBounds();
    return (
      localX >= b.x &&
      localX <= b.x + b.width &&
      localY >= b.y &&
      localY <= b.y + b.height
    );
  }

  private applyTransform() {
    if (!this.model) return;
    const s = this.baseScale * this.userScale;
    this.model.anchor.set(0.5, 0.5);
    this.model.scale.set(s);
    this.model.x = this.fitCenter.x + this.modelPan.x;
    this.model.y = this.fitCenter.y + this.modelPan.y;
    this.invalidateTouchCache();
    if (this.hitDebug) this.drawHitOverlay(true);
  }

  private toLocal(e: PointerEvent | MouseEvent | WheelEvent): { x: number; y: number } {
    const canvas = this.canvas;
    if (!canvas) return { x: e.clientX, y: e.clientY };
    const rect = canvas.getBoundingClientRect();
    return {
      x: e.clientX - rect.left,
      y: e.clientY - rect.top,
    };
  }

  private hitInteractiveLocal(x: number, y: number): boolean {
    if (!this.model) return false;
    // 穿透轮询走缓存 AABB，勿每票 hitTest（Cubism 很重）；精确分区留给点击 resolve
    const union = this.ensureTouchUnionCache();
    if (
      union &&
      x >= union.x - HIT_PAD &&
      x <= union.x + union.w + HIT_PAD &&
      y >= union.y - HIT_PAD &&
      y <= union.y + union.h + HIT_PAD
    ) {
      return true;
    }
    // 整体区：三区外仍可点模型，回落 main_*（见 resolveClickAnimation）
    return this.hitModelLocal(x, y);
  }

  private zonePriority(zone: string): number {
    const z = zone.toLowerCase();
    if (z.includes("special")) return 2;
    if (z.includes("head")) return 1;
    return 0;
  }

  private areaPriority(area: TouchArea): number {
    if (typeof area.priority === "number") return area.priority;
    return this.zonePriority(area.zone || area.id);
  }

  private hitMatchesArea(hits: string[], area: TouchArea): boolean {
    const names = (area.attachments?.length ? area.attachments : [area.id])
      .map((n) => n.toLowerCase())
      .filter(Boolean);
    if (!names.length || !hits.length) return false;
    for (const hit of hits) {
      const h = hit.toLowerCase();
      for (const n of names) {
        if (h === n || h.includes(n) || n.includes(h)) return true;
      }
    }
    return false;
  }

  private collectHits(x: number, y: number): string[] {
    if (!this.model) return [];
    // Live2DModel.hitTest(worldX, worldY) → string[]（内部 toModelPosition + isHit）
    // 切勿调用 internal.hitTest(name, x, y)：签名是 (modelX, modelY)→string[]，[] 在 if 里恒为真
    try {
      const raw = (
        this.model as unknown as { hitTest?: (hx: number, hy: number) => string[] }
      ).hitTest?.(x, y);
      if (!Array.isArray(raw) || !raw.length) return [];
      return raw.map((h) => String(h)).filter(Boolean);
    } catch {
      return [];
    }
  }

  /** 整模回落：随机 main_*；无则 default / touch_body */
  private pickMainClickAnimation(): string | null {
    const mains = this.meta.animations.filter((a) => {
      const low = a.toLowerCase();
      return low.includes("main_");
    });
    if (mains.length) {
      return mains[Math.floor(Math.random() * mains.length)] ?? null;
    }
    return (
      this.meta.defaultClickAnimation ??
      this.meta.clickAnimation ??
      this.meta.animations.find((a) => a.toLowerCase().includes("touch_body")) ??
      null
    );
  }

  private resolveClickAnimation(x: number, y: number): string | null {
    if (!this.model) {
      return this.pickMainClickAnimation();
    }

    // 系统 HitArea：model.hit / hitTest(Touch*)，不以自设 bounds 矩形为准
    const hits = this.collectHits(x, y);

    if (hits.length) {
      let best: TouchArea | null = null;
      let bestPri = Number.NEGATIVE_INFINITY;
      for (const area of this.touchAreaList()) {
        if (!this.hitMatchesArea(hits, area)) continue;
        const pri = this.areaPriority(area);
        if (!best || pri > bestPri) {
          best = area;
          bestPri = pri;
        }
      }
      if (best?.click_animation) {
        this.lastHitLabel = hits.join(",");
        return best.click_animation;
      }
      // 即使配置缺一项，也按命中名本身映射 AL 三区
      let bestHit: string | null = null;
      let bestHitPri = Number.NEGATIVE_INFINITY;
      for (const h of hits) {
        const pri = this.zonePriority(this.zoneFromName(h));
        if (!bestHit || pri > bestHitPri) {
          bestHit = h;
          bestHitPri = pri;
        }
      }
      if (bestHit) {
        const zone = this.zoneFromName(bestHit);
        // 仅 special/head 走 touch_*；未知或 body 仍可用命中名映射，
        // 但若命中名不含三区关键字则视为整模 → main_*
        const low = bestHit.toLowerCase();
        const isNamedTouch =
          low.includes("special") || low.includes("head") || low.includes("body");
        if (isNamedTouch) {
          this.lastHitLabel = hits.join(",");
          const logic =
            zone === "special"
              ? "touch_special"
              : zone === "head"
                ? "touch_head"
                : "touch_body";
          const found = this.meta.animations.find((a) => a.toLowerCase().includes(logic));
          return found ?? best?.click_animation ?? logic;
        }
      }
    }

    // 未命中三区：整模回落 main_*
    this.lastHitLabel = "整体";
    return this.pickMainClickAnimation();
  }

  private clearClickBusyTimer() {
    if (this.clickBusyTimer) {
      window.clearTimeout(this.clickBusyTimer);
      this.clickBusyTimer = 0;
    }
  }

  private releaseClickBusy(token: number, restoreIdle: boolean) {
    if (token !== this.clickBusyToken) return;
    this.clearClickBusyTimer();
    // 抬高 token，避免 onFinish + 兜底超时各播一次 idle
    this.clickBusyToken += 1;
    this.clickBusy = false;
    if (
      restoreIdle &&
      this.meta.idleAnimation &&
      !this.dragging &&
      !this.dragMotionActive
    ) {
      void this.tryPlayMotion(this.meta.idleAnimation, true);
    }
  }

  private async handleClick(x: number, y: number) {
    if (!this.model || this.clickBusy) return;
    if (this.callbacks.isMenuCloseSuppressed?.()) return;

    const anim = this.resolveClickAnimation(x, y);
    const token = ++this.clickBusyToken;
    this.clickBusy = true;
    this.clearClickBusyTimer();
    try {
      this.model.tap?.(x, y);
      this.model.focus?.(x, y);
    } catch {
      /* ignore */
    }

    const finish = () => this.releaseClickBusy(token, true);
    // 兜底：循环/未回调时仍解锁（动作正常结束走 onFinish，通常远早于此）
    this.clickBusyTimer = window.setTimeout(finish, 8000);

    const played = anim
      ? await this.tryPlayMotion(anim, false, { onFinish: finish })
      : false;
    if (!played) {
      this.clearClickBusyTimer();
      this.clickBusyTimer = window.setTimeout(finish, 280);
    }

    const allowBubble = this.callbacks.bubbleEnabled?.() ?? true;
    const line = allowBubble ? this.pickLine(anim) : null;
    // 仅有真实台词才出泡，不对齐 Spine：不编造「▶ 动作名」
    if (line) this.callbacks.onLine?.(line, anim);
    const hitInfo = this.lastHitLabel ? ` · 命中 ${this.lastHitLabel}` : "";
    this.callbacks.onStatus?.(
      played
        ? `动作：${anim}${hitInfo}`
        : anim
          ? `点击「${anim}」未播成${hitInfo}`
          : `点击反馈${hitInfo}`,
    );
  }

  private zoneColor(zone: string): string {
    const z = zone.toLowerCase();
    if (z.includes("special")) return "rgba(244, 63, 94, 0.22)";
    if (z.includes("head")) return "rgba(59, 130, 246, 0.22)";
    return "rgba(34, 197, 94, 0.20)";
  }

  private zoneStroke(zone: string): string {
    const z = zone.toLowerCase();
    if (z.includes("special")) return "rgba(225, 29, 72, 0.75)";
    if (z.includes("head")) return "rgba(37, 99, 235, 0.75)";
    return "rgba(22, 163, 74, 0.7)";
  }

  private syncOverlayLoop() {
    if (!this.hitDebug || !this.overlay) return;
    // 用 interval 替代永久 rAF：调试框 5fps 足够，空转更省
    if (this.overlayRaf) {
      window.clearInterval(this.overlayRaf);
      this.overlayRaf = 0;
    }
    this.drawHitOverlay(false);
    this.overlayRaf = window.setInterval(() => {
      if (!this.hitDebug) {
        window.clearInterval(this.overlayRaf);
        this.overlayRaf = 0;
        return;
      }
      this.drawHitOverlay(false);
    }, 200);
  }

  private touchAreaList(): TouchArea[] {
    if (this.meta.touchAreas.length) return this.meta.touchAreas;
    return [
      {
        id: "TouchSpecial",
        zone: "special",
        attachments: ["TouchSpecial"],
        bounds: { x: 0, y: 0, width: 0, height: 0 },
      },
      {
        id: "TouchHead",
        zone: "head",
        attachments: ["TouchHead"],
        bounds: { x: 0, y: 0, width: 0, height: 0 },
      },
      {
        id: "TouchBody",
        zone: "body",
        attachments: ["TouchBody"],
        bounds: { x: 0, y: 0, width: 0, height: 0 },
      },
    ];
  }

  private ensureTouchUnionCache(
    force = false,
  ): { x: number; y: number; w: number; h: number } | null {
    if (!force && this.touchUnionCache && !this.touchUnionDirty) return this.touchUnionCache;
    if (!force && this.touchUnionCache && this.touchUnionDirty) {
      // 脏但有旧缓存：穿透继续用旧框，重建交给 debounce
      return this.touchUnionCache;
    }
    if (!this.model) return null;
    // 整模可点后不必再对 Touch* 做 N×N hitTest 栅格（曾阻塞加载后数秒）
    const b = this.model.getBounds();
    this.touchUnionDirty = false;
    this.touchUnionCache =
      b.width > 1 && b.height > 1
        ? { x: b.x, y: b.y, w: b.width, h: b.height }
        : null;
    return this.touchUnionCache;
  }

  private drawHitOverlay(force = false) {
    const overlay = this.overlay;
    const canvas = this.canvas;
    const model = this.model;
    if (!overlay || !canvas || !model || !this.hitDebug) return;
    const now = performance.now();
    if (!force && now - this.overlayLastDraw < 200) return;
    this.overlayLastDraw = now;

    const rect = canvas.getBoundingClientRect();
    const dpr = Math.min(window.devicePixelRatio || 1, 1.5);
    const w = Math.max(1, Math.floor(rect.width * dpr));
    const h = Math.max(1, Math.floor(rect.height * dpr));
    if (overlay.width !== w || overlay.height !== h) {
      overlay.width = w;
      overlay.height = h;
    }
    const ctx = overlay.getContext("2d");
    if (!ctx) return;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, rect.width, rect.height);

    // 整体可点区（模型 AABB）— 虚线，低于三区
    const mb = model.getBounds();
    if (mb.width > 1 && mb.height > 1) {
      ctx.beginPath();
      ctx.roundRect(mb.x, mb.y, mb.width, mb.height, 4);
      ctx.fillStyle = "rgba(148, 163, 184, 0.08)";
      ctx.fill();
      ctx.setLineDash([6, 4]);
      ctx.strokeStyle = "rgba(100, 116, 139, 0.65)";
      ctx.lineWidth = 1.25;
      ctx.stroke();
      ctx.setLineDash([]);
      ctx.fillStyle = "rgba(30, 41, 59, 0.85)";
      ctx.font = "11px Segoe UI, Microsoft YaHei, sans-serif";
      ctx.fillText("整体 · main_*", mb.x + 5, mb.y + 14);
    }

    const areas = [...this.touchAreaList()].sort(
      (a, c) => this.areaPriority(c) - this.areaPriority(a),
    );

    for (const area of areas) {
      const names = area.attachments?.length ? area.attachments : [area.id];
      const box = this.sampleDrawableBoundsCached(area.id, names);
      if (!box) continue;
      const r = 4;
      ctx.beginPath();
      ctx.roundRect(box.x, box.y, box.w, box.h, r);
      ctx.fillStyle = this.zoneColor(area.zone);
      ctx.fill();
      ctx.beginPath();
      ctx.roundRect(box.x, box.y, box.w, box.h, r);
      ctx.strokeStyle = this.zoneStroke(area.zone);
      ctx.lineWidth = 1.5;
      ctx.stroke();
      ctx.fillStyle = "rgba(30, 41, 59, 0.92)";
      ctx.font = "11px Segoe UI, Microsoft YaHei, sans-serif";
      ctx.fillText(area.id, box.x + 5, box.y + 14);
    }
  }

  private sampleDrawableBoundsCached(
    cacheKey: string,
    names: string[],
  ): { x: number; y: number; w: number; h: number } | null {
    const hit = this.areaBoxCache.get(cacheKey);
    if (hit) return hit;
    const box = this.sampleDrawableBounds(names);
    if (box) this.areaBoxCache.set(cacheKey, box);
    return box;
  }

  private nameMatchesHitList(hitName: string, names: string[]): boolean {
    const h = hitName.toLowerCase();
    for (const n of names) {
      const t = n.toLowerCase();
      if (!t) continue;
      if (h === t || h.includes(t) || t.includes(h)) return true;
    }
    return false;
  }

  /**
   * 用 Live2DModel.hitTest 在模型包围盒内采样估 Touch* 舞台 AABB。
   * 不手算 drawable→world（易错导致热区飘走、永久点不着）。
   */
  private sampleDrawableBounds(
    names: string[],
  ): { x: number; y: number; w: number; h: number } | null {
    if (!this.model || !names.length) return null;
    const hitTest = (
      this.model as unknown as { hitTest?: (hx: number, hy: number) => string[] }
    ).hitTest;
    if (typeof hitTest !== "function") return null;

    const b = this.model.getBounds();
    if (!(b.width > 4) || !(b.height > 4)) return null;

    let minX = Number.POSITIVE_INFINITY;
    let minY = Number.POSITIVE_INFINITY;
    let maxX = Number.NEGATIVE_INFINITY;
    let maxY = Number.NEGATIVE_INFINITY;
    let hits = 0;
    // 仅调试叠加层用；步数从 14→8，约减半主线程 hitTest
    const steps = 8;
    for (let iy = 0; iy <= steps; iy++) {
      for (let ix = 0; ix <= steps; ix++) {
        const x = b.x + (ix / steps) * b.width;
        const y = b.y + (iy / steps) * b.height;
        let raw: string[] = [];
        try {
          raw = hitTest.call(this.model, x, y) ?? [];
        } catch {
          continue;
        }
        if (!Array.isArray(raw) || !raw.length) continue;
        if (!raw.some((h) => this.nameMatchesHitList(String(h), names))) continue;
        hits++;
        minX = Math.min(minX, x);
        minY = Math.min(minY, y);
        maxX = Math.max(maxX, x);
        maxY = Math.max(maxY, y);
      }
    }
    if (hits < 2) return null;
    // 栅格有步长，外扩一点，悬停更好接
    const pad = Math.max(4, Math.min(b.width, b.height) * 0.02);
    return {
      x: minX - pad,
      y: minY - pad,
      w: maxX - minX + pad * 2,
      h: maxY - minY + pad * 2,
    };
  }

  private pickLine(animation: string | null): string | null {
    const lines = this.meta.lines.filter((l) => l.text.trim());
    if (!lines.length) return null;
    if (animation) {
      const matched = lines.filter(
        (l) => l.animation && l.animation.toLowerCase() === animation.toLowerCase(),
      );
      if (matched.length) {
        return matched[Math.floor(Math.random() * matched.length)].text;
      }
    }
    return lines[Math.floor(Math.random() * lines.length)].text;
  }

  private findMotionFile(name: string): string | null {
    const model = this.model as unknown as {
      internalModel?: {
        motionManager?: {
          definitions?: Record<string, Array<{ Name?: string; File?: string }> | undefined>;
        };
      };
    };
    const defs = model.internalModel?.motionManager?.definitions ?? {};
    const needle = name.toLowerCase();
    for (const list of Object.values(defs)) {
      if (!list) continue;
      for (const m of list) {
        if (m.Name && m.Name.toLowerCase() === needle && m.File) return m.File;
        if (m.File && m.File.toLowerCase().includes(needle)) return m.File;
      }
    }
    // Cubism 常见路径猜测（deferred 尚未合并时）
    const guess = [
      `motions/${name}.motion3.json`,
      `motions/${name}`,
      `${name}.motion3.json`,
    ];
    return guess[0] ?? null;
  }

  private async tryPlayMotionCore(
    name: string,
    idle: boolean,
    opts?: { onFinish?: () => void; force?: boolean },
  ): Promise<boolean> {
    if (!this.model) return false;
    type MotionOpts = {
      onFinish?: () => void;
      onError?: (e: Error) => void;
    };
    const model = this.model as Live2DModel & {
      motion: (
        group: string,
        index?: number,
        priority?: number,
        options?: MotionOpts,
      ) => Promise<boolean>;
      internalModel?: {
        motionManager?: {
          definitions?: Record<string, Array<{ Name?: string; File?: string }> | undefined>;
        };
      };
    };
    // IDLE=1 NORMAL=2 FORCE=3；松手回 idle / 点击反馈用 FORCE 才能打断上一段
    const priority = idle ? (opts?.force ? 3 : 1) : 3;
    const motionOpts: MotionOpts | undefined = opts?.onFinish
      ? {
          onFinish: opts.onFinish,
          onError: () => opts.onFinish?.(),
        }
      : undefined;
    try {
      const defs = model.internalModel?.motionManager?.definitions ?? {};
      const available = Object.keys(defs);
      const groups = [name, idle ? "Idle" : "Tap", idle ? "idle" : "tap", ...available];
      for (const g of groups) {
        if (!g) continue;
        if (available.length && !available.includes(g) && g !== name) continue;
        try {
          if (await model.motion(g, 0, priority, motionOpts)) return true;
        } catch {
          /* next */
        }
      }
      for (const g of available) {
        const list = defs[g];
        if (!list) continue;
        const idx = list.findIndex(
          (m) =>
            (m.Name && m.Name.toLowerCase() === name.toLowerCase()) ||
            (m.File && m.File.toLowerCase().includes(name.toLowerCase())),
        );
        if (idx >= 0 && (await model.motion(g, idx, priority, motionOpts))) return true;
      }
    } catch {
      return false;
    }
    return false;
  }

  private async tryPlayMotion(
    name: string,
    idle = false,
    opts?: { onFinish?: () => void; force?: boolean },
  ): Promise<boolean> {
    if (!this.model || !name.trim()) return false;
    if (await this.tryPlayMotionCore(name, idle, opts)) return true;
    if (idle) return false;
    // deferred warm 尚未合并：短等再试 + 补拉文件
    await new Promise((r) => window.setTimeout(r, 280));
    if (await this.tryPlayMotionCore(name, idle, opts)) return true;
    const file = this.findMotionFile(name);
    const dir = this.meta.modelDir?.trim();
    if (file && dir) {
      await ensureKanmusuAssetReady(dir, file);
      await new Promise((r) => window.setTimeout(r, 60));
      return this.tryPlayMotionCore(name, idle, opts);
    }
    return false;
  }
}
