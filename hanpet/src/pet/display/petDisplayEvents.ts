import type { ReloadCommand } from "./reloadCommand";
import type { PetDisplayState } from "./reloadStateMachine";
import type { PetConfigPayload } from "./types";

export type { PetDisplayState };

export type PetDisplayEventMap = {
  "state-changed": { from: PetDisplayState; to: PetDisplayState };
  "reload-start": { command: ReloadCommand; isSwitch: boolean };
  "reload-success": {
    command: ReloadCommand;
    cfg: PetConfigPayload;
    animationNames: string[];
  };
  "reload-failure": { command: ReloadCommand; err: unknown };
  "reload-finally": { command: ReloadCommand };
  "phase-complete": { command: ReloadCommand; phase: string; ms: number };
};

type Handler<K extends keyof PetDisplayEventMap> = (
  payload: PetDisplayEventMap[K],
) => void;

export class PetDisplayEventBus {
  private readonly handlers = new Map<
    keyof PetDisplayEventMap,
    Set<Handler<keyof PetDisplayEventMap>>
  >();

  on<K extends keyof PetDisplayEventMap>(
    event: K,
    handler: Handler<K>,
  ): () => void {
    let set = this.handlers.get(event);
    if (!set) {
      set = new Set();
      this.handlers.set(event, set);
    }
    set.add(handler as Handler<keyof PetDisplayEventMap>);
    return () => set!.delete(handler as Handler<keyof PetDisplayEventMap>);
  }

  emit<K extends keyof PetDisplayEventMap>(
    event: K,
    payload: PetDisplayEventMap[K],
  ): void {
    const set = this.handlers.get(event);
    if (!set) return;
    for (const fn of set) {
      (fn as Handler<K>)(payload);
    }
  }
}
