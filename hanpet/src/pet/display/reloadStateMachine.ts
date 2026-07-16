import { petLog } from "../log/petDebugLog";

export type PetDisplayState =
  | "idle"
  | "reloading"
  | "loading-hot"
  | "loading-cold"
  | "loading-teardown"
  | "ready"
  | "fallback";

const LOADING_STATES: ReadonlySet<PetDisplayState> = new Set([
  "reloading",
  "loading-hot",
  "loading-cold",
  "loading-teardown",
]);

const INTERACTIVE_STATES: ReadonlySet<PetDisplayState> = new Set(["idle", "ready"]);

export type StateTransitionListener = (
  from: PetDisplayState,
  to: PetDisplayState,
) => void;

export class PetDisplayStateMachine {
  private state: PetDisplayState = "idle";
  private _reloadEverStarted = false;
  private readonly listeners = new Set<StateTransitionListener>();

  get current(): PetDisplayState {
    return this.state;
  }

  get reloadInProgress(): boolean {
    return LOADING_STATES.has(this.state);
  }

  get reloadEverStarted(): boolean {
    return this._reloadEverStarted;
  }

  get isInteractive(): boolean {
    return INTERACTIVE_STATES.has(this.state);
  }

  get isReady(): boolean {
    return this.state === "ready";
  }

  onTransition(listener: StateTransitionListener): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  transition(to: PetDisplayState): void {
    const from = this.state;
    if (from === to) return;
    if (to === "reloading") {
      this._reloadEverStarted = true;
    }
    this.state = to;
    petLog("debug", "state", `${from} → ${to}`);
    for (const fn of this.listeners) {
      fn(from, to);
    }
  }

  resetToIdle(): void {
    this.transition("idle");
  }
}

export function spineModeToLoadingState(mode: "cold" | "hot" | "teardown"): PetDisplayState {
  if (mode === "hot") return "loading-hot";
  if (mode === "cold") return "loading-cold";
  return "loading-teardown";
}
