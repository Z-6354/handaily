import { formatSinkLine, queueSinkLine } from "./petLogSink";

export type PetLogLevel = "debug" | "info" | "warn" | "error";

export interface PetTimingMark {
  label: string;
  ms: number;
  detail?: Record<string, unknown>;
}

const PREFIX = "[pet]";
const timings = new Map<string, number>();

function fmtDetail(detail?: Record<string, unknown>): string {
  if (!detail || Object.keys(detail).length === 0) return "";
  try {
    return ` ${JSON.stringify(detail)}`;
  } catch {
    return "";
  }
}

export function petLog(level: PetLogLevel, scope: string, message: string, detail?: Record<string, unknown>) {
  const line = `${PREFIX}[${scope}] ${message}${fmtDetail(detail)}`;
  if (level === "error") console.error(line);
  else if (level === "warn") console.warn(line);
  else if (level === "debug") console.debug(line);
  else console.info(line);
  if (level !== "debug") {
    queueSinkLine(formatSinkLine(level, scope, message, detail));
  }
}

export function petTimingStart(id: string): void {
  timings.set(id, performance.now());
}

export function petTimingEnd(id: string, scope: string, label: string, detail?: Record<string, unknown>): number {
  const t0 = timings.get(id) ?? performance.now();
  const ms = Math.round(performance.now() - t0);
  timings.delete(id);
  petLog("info", scope, `${label}: ${ms}ms`, detail);
  return ms;
}

export function petTimingMark(scope: string, label: string, ms: number, detail?: Record<string, unknown>) {
  let message = `${label}: ${ms}ms`;
  if (label === "config" && detail && typeof detail.loadConfigMs === "number") {
    message += ` (loadConfig=${detail.loadConfigMs}ms`;
    if (typeof detail.screenMs === "number") {
      message += ` screen=${detail.screenMs}ms`;
    }
    message += ")";
  }
  petLog("info", scope, message, detail);
}

export class PetPhaseTimer {
  private readonly t0 = performance.now();
  private readonly marks: PetTimingMark[] = [];

  constructor(readonly scope: string) {}

  mark(label: string, detail?: Record<string, unknown>) {
    const ms = Math.round(performance.now() - this.t0);
    this.marks.push({ label, ms, detail });
    petTimingMark(this.scope, label, ms, detail);
  }

  finish(label = "total") {
    const ms = Math.round(performance.now() - this.t0);
    petLog("info", this.scope, `${label}: ${ms}ms`, { phases: this.marks });
    return ms;
  }
}
