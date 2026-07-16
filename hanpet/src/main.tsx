import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./styles.css";
import "./styles/secondary-pages.css";
import { waitForTauriInternals } from "./lib/tauriInvoke";

async function bootstrap() {
  const root = document.getElementById("root");
  if (!root) return;

  const inTauri =
    typeof window !== "undefined" &&
    ("__TAURI_INTERNALS__" in window || "__TAURI__" in window);

  if (inTauri) {
    try {
      await waitForTauriInternals();
    } catch (e) {
      root.innerHTML = `<div class="panel" style="margin:24px"><p class="error">${String(e)}</p></div>`;
      return;
    }
  }

  ReactDOM.createRoot(root).render(
    <React.StrictMode>
      <App />
    </React.StrictMode>,
  );
}

void bootstrap();
