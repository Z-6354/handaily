import { Application, BaseTexture } from "pixi.js";
import { Spine } from "@pixi-spine/runtime-3.8";
import { TextureAtlas } from "@pixi-spine/base";
import { AtlasAttachmentLoader, SkeletonBinary36 } from "./skeletonBinary36";
import { loadViewerExSpineConfig, type ViewerExSpineConfig } from "./viewerExConfig";
import {
  applyViewerExBindings,
  pickIdleAnimation,
  pickClickAnimation,
  isLikelyIdleName,
  pickStartAnimation,
} from "./viewerExApply";
import type { ResolveAssetUrl } from "./petAssetResolver";

export interface PetAssetConfig {
  pathPrefix: string;
  configFile?: string | null;
  skelFile: string;
  atlasFile: string;
  pngFile?: string;
}

export interface PetAnimationOptions {
  idleAnimation?: string | null;
  clickAnimation?: string | null;
  bootAnimation?: string | null;
  returnIdleAnimation?: string | null;
  dragAnimation?: string | null;
  randomAnimations?: string[];
  randomMinSec?: number;
  randomMaxSec?: number;
  onRandomAction?: (animation: string) => void;
  skipBootAnimation?: boolean;
}

const IDLE_MIX_SEC = 0.18;
const ACTION_MIX_SEC = 0.15;
/** 单轨模型：待机和一次性动作都在同一条 track 上播放 */
const BASE_TRACK = 0;

async function loadTextureAtlas(
  resolveUrl: ResolveAssetUrl,
  atlasText: string,
  fallbackPng?: string,
): Promise<TextureAtlas> {
  return new Promise((resolve, reject) => {
    new TextureAtlas(
      atlasText,
      (imagePath, loadTexture) => {
        const img = new Image();
        img.crossOrigin = "anonymous";
        void (async () => {
          try {
            img.onload = () => loadTexture(BaseTexture.from(img));
            img.onerror = () => {
              if (fallbackPng && imagePath !== fallbackPng) {
                void resolveUrl(fallbackPng).then((url) => {
                  img.onerror = () => reject(new Error(`纹理加载失败: ${imagePath}`));
                  img.src = url;
                });
                return;
              }
              reject(new Error(`纹理加载失败: ${imagePath}`));
            };
            img.src = await resolveUrl(imagePath);
          } catch (e) {
            reject(e instanceof Error ? e : new Error(String(e)));
          }
        })();
      },
      (atlas) => {
        if (atlas) resolve(atlas);
        else reject(new Error("atlas 解析失败"));
      },
    );
  });
}

export class SpinePet {
  private canvas: HTMLCanvasElement;
  private app: Application | null = null;
  private spine: Spine | null = null;

  private idleName = "";
  private clickName: string | null = null;
  private bootName: string | null = null;
  private returnIdleName: string | null = null;
  private dragName: string | null = null;

  private animationNames: string[] = [];
  private randomAnimations: string[] = [];
  private randomMinSec = 30;
  private randomMaxSec = 120;
  private randomTimer: ReturnType<typeof setTimeout> | null = null;

  private clickActionBusy = false;
  private actionPlaying = false;
  private running = true;

  private viewerExConfig: ViewerExSpineConfig | null = null;
  private onTap?: (animation: string | null) => void;
  private onRandomAction?: (animation: string) => void;
  private assets: PetAssetConfig;
  private resolveAssetUrl?: ResolveAssetUrl;
  private skipBootAnimation = false;

  constructor(
    canvas: HTMLCanvasElement,
    assets: PetAssetConfig,
    options?: {
      onTap?: (animation: string | null) => void;
      onRandomAction?: (animation: string) => void;
      resolveAssetUrl?: ResolveAssetUrl;
    } & PetAnimationOptions,
  ) {
    this.canvas = canvas;
    this.assets = assets;
    this.resolveAssetUrl = options?.resolveAssetUrl;
    this.onTap = options?.onTap;
    this.onRandomAction = options?.onRandomAction;
    this.applyAnimationOptions(options);
    this.skipBootAnimation = options?.skipBootAnimation ?? false;
  }

  getAnimationNames(): string[] {
    return [...this.animationNames];
  }

  isClickActionBusy(): boolean {
    return this.clickActionBusy;
  }

  isActionPlaying(): boolean {
    return this.actionPlaying;
  }

  configureAnimations(options: PetAnimationOptions, opts?: { soft?: boolean }) {
    this.applyAnimationOptions(options);
    if (!this.spine) return;
    if (opts?.soft) {
      this.syncBaseIdleAfterMeta();
      this.restartRandomScheduler();
      return;
    }
    this.returnToIdle();
    this.restartRandomScheduler();
  }

  /** 动作列表 sync 后：修正 idle 名并确保 track 正在循环待机 */
  syncBaseIdleAfterMeta() {
    if (!this.spine) return;
    this.configureAnimationMix();
    if (!this.isCorrectIdleLooping()) {
      this.ensureIdleLoop();
    }
  }

  private applyAnimationOptions(options?: PetAnimationOptions) {
    if (!options) return;
    if (options.idleAnimation) {
      const idle = options.idleAnimation.trim();
      if (this.animationNames.length === 0) {
        this.idleName = idle;
      } else if (this.animationNames.includes(idle)) {
        this.idleName = idle;
      } else {
        this.idleName = pickIdleAnimation(this.animationNames) ?? idle;
      }
    }

    let click = options.clickAnimation?.trim() || null;
    if (click && this.animationNames.length > 0 && !this.animationNames.includes(click)) {
      click = null;
    }
    if (!click && this.animationNames.length > 0) {
      click = pickClickAnimation(this.animationNames);
    }
    this.clickName = click;
    this.bootName = options.bootAnimation?.trim() || null;
    const ret = options.returnIdleAnimation?.trim() || "";
    if (ret && (this.animationNames.length === 0 || this.animationNames.includes(ret))) {
      this.returnIdleName = ret;
    } else {
      this.returnIdleName = null;
    }
    const drag = options.dragAnimation?.trim() || "";
    if (drag && (this.animationNames.length === 0 || this.animationNames.includes(drag))) {
      this.dragName = drag;
    } else {
      this.dragName = null;
    }
    if (options.onRandomAction) {
      this.onRandomAction = options.onRandomAction;
    }

    this.randomAnimations = [...(options.randomAnimations ?? [])].filter((n) => {
      if (this.animationNames.length > 0 && !this.animationNames.includes(n)) return false;
      if (n === this.idleName || n === this.returnIdleName) return false;
      return true;
    });

    this.randomMinSec = options.randomMinSec ?? 30;
    this.randomMaxSec = options.randomMaxSec ?? 120;
  }

  async start(): Promise<string[]> {
    const resolveUrl =
      this.resolveAssetUrl ??
      ((filename: string) =>
        Promise.resolve(
          `${this.assets.pathPrefix}${this.assets.pathPrefix.endsWith("/") ? "" : "/"}${filename}`,
        ));
    const { configFile } = this.assets;
    let skelFile = this.assets.skelFile;
    let atlasFile = this.assets.atlasFile;
    this.viewerExConfig = null;

    const viewerEx = await loadViewerExSpineConfig("", configFile, resolveUrl);
    if (viewerEx) {
      skelFile = viewerEx.skelFile;
      atlasFile = viewerEx.atlasFile;
      this.viewerExConfig = viewerEx.config;
    }

    const [atlasText, skelBuf] = await Promise.all([
      fetch(await resolveUrl(atlasFile)).then((r) => {
        if (!r.ok) throw new Error(`atlas 加载失败: ${atlasFile}`);
        return r.text();
      }),
      fetch(await resolveUrl(skelFile)).then(async (r) => {
        if (!r.ok) throw new Error(`skel 加载失败: ${skelFile}`);
        return new Uint8Array(await r.arrayBuffer());
      }),
    ]);

    const atlas = await loadTextureAtlas(resolveUrl, atlasText, this.assets.pngFile);
    const binary = new SkeletonBinary36(new AtlasAttachmentLoader(atlas));
    const skeletonData = binary.readSkeletonData(skelBuf);

    this.app = new Application({
      view: this.canvas,
      backgroundAlpha: 0,
      width: this.canvas.width || 220,
      height: this.canvas.height || 280,
      antialias: true,
      resolution: window.devicePixelRatio || 1,
      autoDensity: true,
    });

    this.spine = new Spine(skeletonData);
    this.spine.state.data.defaultMix = IDLE_MIX_SEC;
    this.spine.autoUpdate = true;
    this.running = true;
    this.app.stage.addChild(this.spine);

    this.animationNames = skeletonData.animations.map((a) => a.name);
    const detectedIdle = pickIdleAnimation(this.animationNames);
    if (detectedIdle && (!this.idleName || !isLikelyIdleName(this.idleName))) {
      this.idleName = detectedIdle;
    } else if (!this.idleName) {
      this.idleName = detectedIdle ?? "";
    }
    this.configureAnimationMix();

    if (this.viewerExConfig) {
      applyViewerExBindings(this.spine, this.viewerExConfig, { setupSlots: false });
    } else if (this.spine.skeleton.data.defaultSkin) {
      this.spine.skeleton.setSkin(this.spine.skeleton.data.defaultSkin);
    }

    const idle = this.resolveIdleName();
    this.idleName = idle;
    if (idle && this.animationNames.includes(idle)) {
      this.spine.state.setAnimation(BASE_TRACK, idle, true);
    }

    const bootAnim =
      this.bootName ||
      (this.viewerExConfig && pickStartAnimation(this.viewerExConfig)) ||
      "";
    if (
      !this.skipBootAnimation &&
      bootAnim &&
      bootAnim !== idle &&
      this.animationNames.includes(bootAnim)
    ) {
      this.playOneShot(bootAnim);
    }

    this.fitSpineToCanvas();
    this.restartRandomScheduler();
    this.app.render();

    return this.animationNames;
  }

  private resolveIdleName(): string {
    const preferred = this.returnIdleName || this.idleName;
    if (preferred && this.animationNames.includes(preferred)) {
      return preferred;
    }
    return pickIdleAnimation(this.animationNames) ?? preferred;
  }

  private isCorrectIdleLooping(): boolean {
    const idle = this.returnIdleName || this.idleName;
    const cur = this.spine?.state.getCurrent(BASE_TRACK);
    return Boolean(idle && cur?.animation?.name === idle && cur.loop);
  }

  private isBaseIdleAnimation(name: string): boolean {
    const idle = this.returnIdleName || this.idleName;
    return name === idle || isLikelyIdleName(name);
  }

  /** 切回 idle 循环；动作播放中不打扰当前 track */
  private ensureIdleLoop() {
    if (!this.spine || this.actionPlaying) return;
    const idle = this.resolveIdleName();
    if (!idle || !this.animationNames.includes(idle)) return;
    this.idleName = idle;
    this.spine.state.setAnimation(BASE_TRACK, idle, true);
    this.app?.render();
  }

  private enablePlayback() {
    this.running = true;
    if (this.spine) this.spine.autoUpdate = true;
  }

  private configureAnimationMix() {
    if (!this.spine) return;
    const data = this.spine.state.data;
    data.defaultMix = IDLE_MIX_SEC;
    const idle = this.returnIdleName || this.idleName;
    if (!idle || !this.animationNames.includes(idle)) return;
    for (const name of this.animationNames) {
      if (name === idle) continue;
      data.setMix(idle, name, ACTION_MIX_SEC);
      data.setMix(name, idle, ACTION_MIX_SEC);
    }
  }

  private returnToIdle() {
    if (!this.spine) return;
    if (this.isCorrectIdleLooping()) return;
    this.ensureIdleLoop();
  }

  /**
   * 单轨播放一次性动作：在 track 0 上从当前待机 mix 到动作，
   * 播完自动排队回到 idle 循环。
   */
  private playOneShot(name: string, onComplete?: () => void): boolean {
    if (!this.spine || !name || !this.animationNames.includes(name)) return false;
    const idle = this.returnIdleName || this.idleName;
    if (!idle) return false;

    this.enablePlayback();
    this.actionPlaying = true;

    const entry = this.spine.state.setAnimation(BASE_TRACK, name, false);
    entry.mixDuration = ACTION_MIX_SEC;
    if (this.animationNames.includes(idle)) {
      this.spine.state.addAnimation(BASE_TRACK, idle, true, 0);
    }
    let finished = false;
    const finish = () => {
      if (finished) return;
      finished = true;
      this.actionPlaying = false;
      onComplete?.();
    };
    entry.listener = {
      complete: finish,
      end: finish,
      dispose: finish,
    };
    this.app?.render();
    return true;
  }

  playAnimation(name: string, loop = false) {
    if (!this.spine || !name) return false;
    if (!this.animationNames.includes(name)) return false;
    this.enablePlayback();
    if (loop && this.isBaseIdleAnimation(name)) {
      this.ensureIdleLoop();
      this.app?.render();
      return true;
    }
    return this.playOneShot(name);
  }

  /** 设置页预览 */
  previewPlay(name: string, loop = false): boolean {
    if (!this.spine || !name || !this.animationNames.includes(name)) return false;
    this.clearRandomTimer();
    this.enablePlayback();
    if (loop && this.isBaseIdleAnimation(name)) {
      this.ensureIdleLoop();
      return true;
    }
    return this.playOneShot(name, () => this.restartRandomScheduler());
  }

  private fitSpineToCanvas() {
    if (!this.spine || !this.app) return;
    const pad = 12;
    const w = this.app.screen.width - pad * 2;
    const h = this.app.screen.height - pad * 2;
    const bounds = this.spine.getLocalBounds();
    const bw = Math.max(bounds.width, 1);
    const bh = Math.max(bounds.height, 1);
    const scale = Math.min(w / bw, h / bh);
    this.spine.scale.set(scale);
    this.spine.position.set(
      this.app.screen.width / 2 - (bounds.x + bw / 2) * scale,
      this.app.screen.height - pad - (bounds.y + bh) * scale,
    );
  }

  private finishClickAction() {
    this.clickActionBusy = false;
    this.restartRandomScheduler();
  }

  playTap(): boolean {
    if (!this.spine || this.clickActionBusy) return false;
    const tap =
      this.clickName && this.animationNames.includes(this.clickName)
        ? this.clickName
        : pickClickAnimation(this.animationNames) ??
          (this.idleName && this.animationNames.includes(this.idleName)
            ? this.idleName
            : "");
    if (!tap) return false;
    this.clearRandomTimer();
    this.clickActionBusy = true;
    if (!this.playOneShot(tap, () => this.finishClickAction())) {
      this.clickActionBusy = false;
      return false;
    }
    return true;
  }

  /** 拖拽期间循环播放拖拽动作，松开后由 stopDrag 回到待机 */
  playDrag(): boolean {
    if (!this.spine || !this.dragName || !this.animationNames.includes(this.dragName)) return false;
    this.clearRandomTimer();
    this.enablePlayback();
    this.actionPlaying = true;
    const entry = this.spine.state.setAnimation(BASE_TRACK, this.dragName, true);
    entry.mixDuration = ACTION_MIX_SEC;
    this.app?.render();
    return true;
  }

  /** 拖拽结束：切回待机循环并恢复随机调度 */
  stopDrag(): void {
    if (!this.spine) return;
    this.actionPlaying = false;
    this.ensureIdleLoop();
    this.restartRandomScheduler();
  }

  setIdleAnimation(name: string) {
    this.idleName = name;
    this.randomAnimations = this.randomAnimations.filter((n) => n !== name);
    this.returnToIdle();
    this.restartRandomScheduler();
  }

  setRenderPaused(paused: boolean) {
    if (!this.spine) return;
    this.spine.autoUpdate = !paused && this.running;
  }

  handleClick(): boolean {
    if (this.clickActionBusy) return false;
    const tapAnim =
      this.clickName ??
      pickClickAnimation(this.animationNames) ??
      this.idleName ??
      null;
    const played = this.playTap();
    if (played) {
      this.onTap?.(tapAnim);
    }
    return played;
  }

  resizeCanvas(width: number, height: number, refit = true) {
    const w = Math.round(width);
    const h = Math.round(height);
    if (!this.app) {
      this.canvas.width = w;
      this.canvas.height = h;
      return;
    }
    const same =
      Math.round(this.app.screen.width) === w &&
      Math.round(this.app.screen.height) === h;
    if (same) {
      if (refit) {
        this.fitSpineToCanvas();
        this.app.render();
      }
      return;
    }
    this.app.renderer.resize(w, h);
    if (refit) {
      this.fitSpineToCanvas();
    }
    this.app.render();
  }

  private clearRandomTimer() {
    if (this.randomTimer) {
      clearTimeout(this.randomTimer);
      this.randomTimer = null;
    }
  }

  private randomDelayMs(): number {
    const min = Math.max(5, this.randomMinSec) * 1000;
    const max = Math.max(this.randomMinSec, this.randomMaxSec) * 1000;
    return min + Math.random() * Math.max(0, max - min);
  }

  private restartRandomScheduler() {
    this.clearRandomTimer();
    if (
      !this.spine ||
      this.randomAnimations.length === 0 ||
      this.clickActionBusy ||
      this.isActionPlaying()
    ) {
      return;
    }
    this.randomTimer = setTimeout(() => this.playRandomExtra(), this.randomDelayMs());
  }

  private playRandomExtra() {
    this.randomTimer = null;
    if (!this.spine || this.randomAnimations.length === 0 || this.clickActionBusy || this.isActionPlaying()) {
      if (this.clickActionBusy) {
        this.restartRandomScheduler();
      }
      return;
    }
    const name =
      this.randomAnimations[
        Math.floor(Math.random() * this.randomAnimations.length)
      ];
    if (!this.playOneShot(name, () => this.restartRandomScheduler())) {
      this.restartRandomScheduler();
      return;
    }
    this.onRandomAction?.(name);
  }

  dispose() {
    this.clearRandomTimer();
    this.running = false;
    if (this.spine) {
      this.spine.autoUpdate = false;
      this.spine.destroy({ children: true });
      this.spine = null;
    }
    this.app?.destroy(false, { children: true, texture: true, baseTexture: true });
    this.app = null;
  }
}
