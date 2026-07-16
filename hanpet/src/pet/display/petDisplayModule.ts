import type { SpinePet } from "../spinePet";

import type { PetAssetResolver } from "../petAssetResolver";

import { petLog, PetPhaseTimer } from "../log/petDebugLog";

import { disposeAssetResolver } from "./assetPipeline";
import { awaitAnimationFrames, releaseCanvasGlContext } from "./canvasHost";

import type { PetDisplayDom, PetDisplayHost, InvokeFn } from "./displayContracts";

import { PetDisplayEventBus, type PetDisplayEventMap } from "./petDisplayEvents";

import {

  createPipelineContext,

  runLoadPipeline,

} from "./reloadPipeline";

import {

  normalizeReloadCommand,

  reloadCommand,

  reloadCommandLabel,

  type ReloadCommand,

} from "./reloadCommand";

import {

  PetDisplayStateMachine,

  spineModeToLoadingState,

  type PetDisplayState,

} from "./reloadStateMachine";

import type { PetConfigPayload, SpineInitMode } from "./types";



export type { PetDisplayDom, PetDisplayHost, InvokeFn } from "./displayContracts";



export class PetDisplayModule {

  private spine: SpinePet | null = null;

  private resolver: PetAssetResolver | null = null;

  private lines: PetConfigPayload["lines"] = [];

  private reloadSerial: Promise<void> = Promise.resolve();

  private lastFallbackSrc = "";
  private lastLoadError: unknown = null;
  private loadedModelId: string | null = null;



  readonly stateMachine = new PetDisplayStateMachine();

  readonly events = new PetDisplayEventBus();



  constructor(

    private readonly dom: PetDisplayDom,

    private readonly host: PetDisplayHost,

    private readonly invoke: InvokeFn,

  ) {

    this.stateMachine.onTransition((from, to) => {

      this.events.emit("state-changed", { from, to });

    });

  }



  get pet(): SpinePet | null {

    return this.spine;

  }

  get currentModelId(): string | null {
    return this.loadedModelId;
  }

  async switchModel(config: PetConfigPayload, switchId: number): Promise<boolean> {
    return this.reload(
      reloadCommand("switch-model", "menu", `switch-${switchId}`),
      config,
    );
  }



  get reloadInProgress(): boolean {

    return this.stateMachine.reloadInProgress;

  }



  get reloadEverStarted(): boolean {

    return this.stateMachine.reloadEverStarted;

  }



  get displayState(): PetDisplayState {

    return this.stateMachine.current;

  }



  get isInteractive(): boolean {

    return this.stateMachine.isInteractive;

  }



  whenIdle(): Promise<void> {

    return this.reloadSerial;

  }



  get petLines(): PetConfigPayload["lines"] {

    return this.lines;

  }



  setPetLines(lines: PetConfigPayload["lines"]) {

    this.lines = lines;

  }



  on<K extends keyof PetDisplayEventMap>(

    event: K,

    handler: (payload: PetDisplayEventMap[K]) => void,

  ): () => void {

    return this.events.on(event, handler);

  }



  disposeForExit() {

    this.spine?.dispose("teardown");

    this.spine = null;

    disposeAssetResolver(this.resolver);

    this.resolver = null;

    this.stateMachine.resetToIdle();

  }



  async reload(command: ReloadCommand | string, explicitConfig?: PetConfigPayload): Promise<boolean> {

    const cmd = normalizeReloadCommand(command);
    petLog("info", "display", "reload queued", {
      command: reloadCommandLabel(cmd),
      state: this.stateMachine.current,
    });
    if (this.host.isAppExiting()) return false;

    await this.host.shouldExitEditBeforeReload();

    let success = false;

    this.reloadSerial = this.reloadSerial.then(async () => {
      const timer = new PetPhaseTimer("reload");

      this.stateMachine.transition("reloading");

      const isSwitch = this.spine !== null;
      const isMenuSwitch = cmd.kind === "switch-model";

      this.events.emit("reload-start", { command: cmd, isSwitch });



      try {

        const configT0 = performance.now();
        const cfg = explicitConfig ?? (await this.host.loadConfig());
        const loadConfigMs = Math.round(performance.now() - configT0);

        let screenMs = 0;
        if (isSwitch) {
          void this.host.refreshScreenBounds();
        } else {
          const screenT0 = performance.now();
          await this.host.refreshScreenBounds();
          screenMs = Math.round(performance.now() - screenT0);
        }

        timer.mark("config", { loadConfigMs, screenMs, isSwitch });

        const modelChanged =
          isSwitch &&
          this.loadedModelId !== null &&
          this.loadedModelId !== cfg.model_id;
        // 菜单换模必须 teardown（replace canvas）；hot 在同 canvas 换模会触发 WebGL shader 0 错误
        const tryHot = isSwitch && !modelChanged && !isMenuSwitch;

        const hotOk = tryHot
          ? await this.runLoad(cfg, "hot", cmd)
          : false;

        let ok = hotOk;

        if (!ok) {
          if (isSwitch && tryHot) {
            releaseCanvasGlContext(this.dom.canvas);
            await awaitAnimationFrames(3);
          }

          const fallbackMode: SpineInitMode = isMenuSwitch || isSwitch ? "teardown" : "cold";

          const fallbackCmd: ReloadCommand = {

            ...cmd,

            trace: `${reloadCommandLabel(cmd)}-fallback`,

          };

          ok = await this.runLoad(cfg, fallbackMode, fallbackCmd);

          if (!ok && isMenuSwitch && isSwitch) {
            const retryCmd: ReloadCommand = {
              ...cmd,
              trace: `${reloadCommandLabel(cmd)}-retry`,
            };
            ok = await this.runLoad(cfg, "teardown", retryCmd);
          }

        }

        timer.mark("load");

        if (ok && this.spine) {

          this.loadedModelId = cfg.model_id;
          if (isMenuSwitch) {
            const switchId = Number.parseInt(cmd.trace?.replace("switch-", "") ?? "", 10);
            if (Number.isFinite(switchId) && switchId > 0) {
              await this.invoke("pet_confirm_switch", {
                switchId,
                modelId: cfg.model_id,
              });
            }
          } else {
            await this.invoke("pet_mark_spine_ready", { modelId: cfg.model_id });
          }

          this.dom.clearLoadError();

          this.stateMachine.transition("ready");

          success = true;

        } else if (!ok) {
          this.stateMachine.transition("fallback");
          const err =
            this.lastLoadError instanceof Error
              ? this.lastLoadError
              : new Error(
                  this.lastLoadError
                    ? String(this.lastLoadError)
                    : "Spine 模型加载失败",
                );
          this.events.emit("reload-failure", { command: cmd, err });
        }

      } catch (e) {

        petLog("error", "reload", "failed", {

          command: reloadCommandLabel(cmd),

          err: String(e),

        });

        this.dom.setFallbackSrc(this.lastFallbackSrc);

        this.stateMachine.transition("fallback");

        this.events.emit("reload-failure", { command: cmd, err: e });

        success = false;

      } finally {

        this.dom.hideBootHint();

        const state = this.stateMachine.current;

        if (

          (state === "reloading" || state.startsWith("loading-")) &&

          !success &&

          state !== "fallback"

        ) {

          this.stateMachine.resetToIdle();

        }

        this.events.emit("reload-finally", { command: cmd });

        timer.finish(`reload(${reloadCommandLabel(cmd)})`);

      }

    });



    await this.reloadSerial;

    return success;

  }



  private async runLoad(

    cfg: PetConfigPayload,

    mode: SpineInitMode,

    command: ReloadCommand,

  ): Promise<boolean> {

    const trace = reloadCommandLabel(command);

    petLog("info", "display", `runLoad ${mode}`, { modelId: cfg.model_id, trace });



    this.stateMachine.transition(spineModeToLoadingState(mode));



    const ctx = createPipelineContext({

      command,

      mode,

      trace,

      cfg,

      dom: this.dom,

      host: this.host,

      invoke: this.invoke,

      priorSpine: this.spine,

      getSpine: () => this.spine,

      setSpine: (spine) => {

        this.spine = spine;

      },

      getResolver: () => this.resolver,

      setResolver: (resolver) => {

        this.resolver = resolver;

      },

      lastFallbackSrc: this.lastFallbackSrc,

      setLastFallbackSrc: (url) => {

        this.lastFallbackSrc = url;

      },

      setLines: (lines) => {

        this.lines = lines;

      },

      onPhaseComplete: (phase, ms) => {

        this.events.emit("phase-complete", { command, phase, ms });

      },

    });



    const ok = await runLoadPipeline(ctx);
    if (!ok || !ctx.result?.ok) {
      this.lastLoadError = ctx.result?.error ?? new Error(`runLoad ${mode} pipeline failed`);
      return false;
    }
    this.lastLoadError = null;

    const pending = this.host.getPendingPreview();

    if (pending) {

      this.host.clearPendingPreview();

      this.host.runPreviewAnimation(pending.animation, pending.loop);

    }



    this.events.emit("reload-success", {

      command,

      cfg,

      animationNames: ctx.result.animationNames,

    });



    if (ctx.result.animationNames.length > 0) {

      void this.host

        .syncAnimations(cfg.model_id, ctx.result.animationNames, cfg.idle_animation)

        .then((meta) => {

          if (!this.spine || !meta) return;

          this.spine.configureAnimations(

            {

              idleAnimation: meta.idle_animation ?? cfg.idle_animation,

              clickAnimation: meta.click_animation ?? cfg.click_animation,

              bootAnimation: meta.boot_animation ?? cfg.boot_animation,

              returnIdleAnimation: meta.return_idle_animation ?? cfg.return_idle_animation,

              dragAnimation: meta.drag_animation ?? cfg.drag_animation,

              randomAnimations: meta.random_animations ?? cfg.random_animations ?? [],

              randomMinSec: meta.random_min_sec ?? cfg.random_min_sec ?? 30,

              randomMaxSec: meta.random_max_sec ?? cfg.random_max_sec ?? 120,

            },

            { soft: true },

          );

          this.lines = meta.lines ?? cfg.lines ?? this.lines;

        });

    }



    return true;

  }

}



export function createPetDisplayModule(

  dom: PetDisplayDom,

  host: PetDisplayHost,

  invoke: InvokeFn,

): PetDisplayModule {

  return new PetDisplayModule(dom, host, invoke);

}

