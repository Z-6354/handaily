import type { PetWikiBulkImportProgress, PetWikiBulkImportStartResult } from "./xiaohan";

/** 兼容 camelCase / snake_case 的进度 payload */
export function normalizeWikiBulkImportProgress(
  raw: PetWikiBulkImportProgress | Record<string, unknown>,
): PetWikiBulkImportProgress {
  const r = raw as Record<string, unknown>;
  return {
    phase: String(r.phase ?? ""),
    index: Number(r.index ?? 0),
    total: Number(r.total ?? 0),
    model_id: String(r.model_id ?? r.modelId ?? ""),
    model_name: String(r.model_name ?? r.modelName ?? ""),
    message: String(r.message ?? ""),
    lines_imported: Number(r.lines_imported ?? r.linesImported ?? 0),
    succeeded: Number(r.succeeded ?? 0),
    failed: Number(r.failed ?? 0),
    skipped: Number(r.skipped ?? 0),
    updated_at_ms: Number(r.updated_at_ms ?? r.updatedAtMs ?? 0),
  };
}

export function normalizeWikiBulkImportStartResult(
  raw: PetWikiBulkImportStartResult | Record<string, unknown>,
): PetWikiBulkImportStartResult {
  const r = raw as Record<string, unknown>;
  return {
    started: Boolean(r.started ?? false),
    already_running: Boolean(r.already_running ?? r.alreadyRunning ?? false),
  };
}
