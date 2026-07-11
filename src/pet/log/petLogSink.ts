import type { PetLogLevel } from "./petDebugLog";

export type PetLogSinkFn = (lines: string[]) => Promise<void>;

const pending: string[] = [];
let sink: PetLogSinkFn | null = null;
let flushTimer: ReturnType<typeof setTimeout> | null = null;
let flushChain = Promise.resolve();

const FLUSH_MS = 120;
const MAX_BATCH = 48;

export function initPetLogSink(append: PetLogSinkFn, dataDir?: string) {
  sink = append;
  if (dataDir) {
    queueSinkLine(
      JSON.stringify({
        ts: Date.now(),
        level: "info",
        scope: "log",
        message: "pet display log sink ready",
        logFile: `${dataDir}/logs/pet-display.jsonl`,
      }),
    );
  }
}

export function queueSinkLine(line: string) {
  pending.push(line);
  if (!sink || flushTimer) return;
  flushTimer = window.setTimeout(() => {
    flushTimer = null;
    void flushSink();
  }, FLUSH_MS);
}

export function formatSinkLine(
  level: PetLogLevel,
  scope: string,
  message: string,
  detail?: Record<string, unknown>,
): string {
  return JSON.stringify({
    ts: Date.now(),
    level,
    scope,
    message,
    ...(detail && Object.keys(detail).length > 0 ? { detail } : {}),
  });
}

async function flushSink() {
  if (!sink || pending.length === 0) return;
  const batch = pending.splice(0, MAX_BATCH);
  flushChain = flushChain
    .then(() => sink!(batch))
    .catch((err) => {
      console.error("[pet][log] sink flush failed", err);
    });
  await flushChain;
  if (pending.length > 0) {
    flushTimer = window.setTimeout(() => {
      flushTimer = null;
      void flushSink();
    }, FLUSH_MS);
  }
}

export async function flushPetLogSink() {
  if (flushTimer) {
    clearTimeout(flushTimer);
    flushTimer = null;
  }
  await flushSink();
}
