import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import type { PetWikiBulkImportProgress } from "../lib/xiaohan";
import { xiaohan } from "../lib/xiaohan";
import { normalizeWikiBulkImportProgress } from "../lib/wikiBulkImportProgress";
import { waitForTauriInternals } from "../lib/tauriInvoke";

const POLL_MS = 500;

function isImportActive(phase: string): boolean {
  return phase === "scan" || phase === "import" || phase === "paused";
}

function shouldAutoOpenModal(phase: string): boolean {
  return isImportActive(phase);
}

export function useWikiBulkImport() {
  const [progress, setProgress] = useState<PetWikiBulkImportProgress | null>(null);
  const [open, setOpen] = useState(false);
  const lastUpdatedRef = useRef(0);
  const pollTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  const applyPayload = useCallback((raw: PetWikiBulkImportProgress | Record<string, unknown>) => {
    const payload = normalizeWikiBulkImportProgress(raw);
    lastUpdatedRef.current = payload.updated_at_ms > 0 ? payload.updated_at_ms : Date.now();
    setProgress(payload);
    if (shouldAutoOpenModal(payload.phase)) {
      setOpen(true);
    }
    if (pollTimer.current) {
      clearTimeout(pollTimer.current);
      pollTimer.current = null;
    }
    if (isImportActive(payload.phase)) {
      pollTimer.current = setTimeout(() => {
        void xiaohan.petGetWikiBulkImportProgress().then((snapshot) => {
          if (snapshot) applyPayload(snapshot);
        });
      }, POLL_MS);
    }
  }, []);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let cancelled = false;

    void waitForTauriInternals()
      .then(() => listen<PetWikiBulkImportProgress>("pet-wiki-bulk-import-progress", (event) => {
        if (!cancelled) applyPayload(event.payload);
      }))
      .then((fn) => {
        unlisten = fn;
        return xiaohan.petGetWikiBulkImportProgress();
      })
      .then((snapshot) => {
        if (!cancelled && snapshot) applyPayload(snapshot);
      })
      .catch(() => {});

    return () => {
      cancelled = true;
      if (pollTimer.current) clearTimeout(pollTimer.current);
      unlisten?.();
    };
  }, [applyPayload]);

  const start = useCallback(async () => {
    const result = await xiaohan.petStartWikiBulkImport();
    if (result.started || result.already_running) {
      setOpen(true);
    }
    return result;
  }, []);

  const pause = useCallback(async () => {
    await xiaohan.petPauseWikiBulkImport();
  }, []);

  const resume = useCallback(async () => {
    await xiaohan.petResumeWikiBulkImport();
  }, []);

  const stop = useCallback(async () => {
    await xiaohan.petStopWikiBulkImport();
  }, []);

  const dismiss = useCallback(() => {
    if (progress && isImportActive(progress.phase)) return;
    setOpen(false);
  }, [progress]);

  const phase = progress?.phase ?? "";
  const isActive = isImportActive(phase);
  const isPaused = phase === "paused";

  return {
    progress,
    open,
    isActive,
    isPaused,
    start,
    pause,
    resume,
    stop,
    dismiss,
    setOpen,
  };
}
