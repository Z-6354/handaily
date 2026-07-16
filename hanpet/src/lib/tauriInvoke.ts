type TauriInvokeFn = <T>(
  cmd: string,
  args?: Record<string, unknown>,
) => Promise<T>;

let tauriReadyPromise: Promise<void> | null = null;
let cachedInvoke: TauriInvokeFn | null = null;

function waitForTauriInternalsImpl(timeoutMs: number): Promise<void> {
  if (typeof window === "undefined") {
    return Promise.reject(new Error("无 window 环境"));
  }
  if ("__TAURI_INTERNALS__" in window) {
    return Promise.resolve();
  }
  return new Promise((resolve, reject) => {
    const start = Date.now();
    const tick = () => {
      if ("__TAURI_INTERNALS__" in window) {
        resolve();
        return;
      }
      if (Date.now() - start >= timeoutMs) {
        reject(
          new Error(
            "Tauri 环境未就绪。请使用 npm run tauri:dev 启动，不要单独运行 npm run dev。",
          ),
        );
        return;
      }
      requestAnimationFrame(tick);
    };
    tick();
  });
}

/** 等待 Tauri IPC 注入（避免 __TAURI_INTERNALS__ 未就绪时 invoke 报错） */
export function waitForTauriInternals(timeoutMs = 12_000): Promise<void> {
  if (!tauriReadyPromise) {
    tauriReadyPromise = waitForTauriInternalsImpl(timeoutMs);
  }
  return tauriReadyPromise;
}

async function getInvoke(): Promise<TauriInvokeFn> {
  await waitForTauriInternals();
  if (!cachedInvoke) {
    const mod = await import("@tauri-apps/api/core");
    cachedInvoke = mod.invoke as TauriInvokeFn;
  }
  return cachedInvoke;
}

export async function tauriInvoke<T>(
  cmd: string,
  args?: Record<string, unknown>,
): Promise<T> {
  const invoke = await getInvoke();
  return invoke<T>(cmd, args);
}
