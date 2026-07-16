export function isWindowsDesktop(): boolean {
  return typeof navigator !== "undefined" && /Windows/i.test(navigator.userAgent);
}

export function isTauriRuntime(): boolean {
  if (typeof window === "undefined") return false;
  return "__TAURI_INTERNALS__" in window || "__TAURI__" in window;
}

/** Windows 桌面客户端才支持注册表自启动 */
export function isAutostartSupportedClient(): boolean {
  return isTauriRuntime() && isWindowsDesktop();
}
