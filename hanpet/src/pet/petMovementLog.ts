export type PetMoveEvent =
  | "logger-ready"
  | "resize-start"
  | "resize-move"
  | "resize-apply"
  | "resize-commit"
  | "resize-end"
  | "offset-start"
  | "offset-move"
  | "offset-end"
  | "hit-area-move-start"
  | "hit-area-move"
  | "hit-area-move-end"
  | "exit-attempt"
  | "exit-blocked"
  | "exit-done"
  | "exit-error"
  | "outside-check"
  | "enter-edit"
  | "enter-edit-fail"
  | "enter-blocked"
  | "click-empty"
  | "blur-exit-schedule"
  | "blur-exit-cancel"
  | "mousedown-outside"
  | "esc-exit"
  | "click-through"
  | "restore-interaction"
  | "test-action"
  | "window-drag-move";

export type PetMovementLogPayload = Record<string, unknown>;

export interface PetMovementLogEntry {
  ts: number;
  event: PetMoveEvent;
  data: PetMovementLogPayload;
}

const LOG_PREFIX = "[pet-move]";
const MAX_BUFFER = 4000;
const MOVE_THROTTLE_MS = 60;
const FLUSH_DEBOUNCE_MS = 350;

const buffer: PetMovementLogEntry[] = [];
const moveThrottle = new Map<string, number>();
const pendingLines: string[] = [];
let flushTimer: ReturnType<typeof setTimeout> | null = null;
let flushChain = Promise.resolve();
let appendLogs: ((lines: string[]) => Promise<void>) | null = null;
let logFileHint = "";

export function initPetMovementLog(
  invokeAppend: (lines: string[]) => Promise<void>,
  dataPath?: string,
) {
  appendLogs = invokeAppend;
  if (dataPath) {
    logFileHint = `${dataPath}/logs/pet-movement.jsonl`;
    petMovementLog("logger-ready", { logFile: logFileHint });
  }
}

export function petMovementLog(event: PetMoveEvent, data: PetMovementLogPayload = {}) {
  const entry: PetMovementLogEntry = { ts: Date.now(), event, data };
  buffer.push(entry);
  if (buffer.length > MAX_BUFFER) {
    buffer.splice(0, buffer.length - MAX_BUFFER);
  }
  console.log(LOG_PREFIX, event, data);
  queueFileFlush(entry);
}

export function petMovementLogThrottled(
  key: string,
  event: PetMoveEvent,
  data: PetMovementLogPayload = {},
  intervalMs = MOVE_THROTTLE_MS,
) {
  const now = Date.now();
  const last = moveThrottle.get(key) ?? 0;
  if (now - last < intervalMs) return;
  moveThrottle.set(key, now);
  petMovementLog(event, data);
}

function queueFileFlush(entry: PetMovementLogEntry) {
  pendingLines.push(JSON.stringify(entry));
  if (flushTimer) return;
  flushTimer = window.setTimeout(() => {
    flushTimer = null;
    void flushPendingLines();
  }, FLUSH_DEBOUNCE_MS);
}

async function flushPendingLines() {
  if (!appendLogs || pendingLines.length === 0) return;
  const batch = pendingLines.splice(0, pendingLines.length);
  flushChain = flushChain
    .then(() => appendLogs!(batch))
    .catch((err) => {
      console.error(`${LOG_PREFIX} file-flush-failed`, err);
    });
  await flushChain;
}

export function dumpPetMovementLog(): string {
  return JSON.stringify(buffer, null, 2);
}

export function recentPetMovementLog(count = 80): PetMovementLogEntry[] {
  return buffer.slice(-count);
}

export function clearPetMovementLog() {
  buffer.length = 0;
  moveThrottle.clear();
}

export function getPetMovementLogPath(): string {
  return logFileHint;
}

declare global {
  interface Window {
    __petMovementLog?: {
      dump: () => string;
      recent: (n?: number) => PetMovementLogEntry[];
      clear: () => void;
      path: () => string;
    };
  }
}

if (typeof window !== "undefined") {
  window.__petMovementLog = {
    dump: dumpPetMovementLog,
    recent: recentPetMovementLog,
    clear: clearPetMovementLog,
    path: getPetMovementLogPath,
  };
}
