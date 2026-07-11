import { lazy, Suspense, useEffect, useState, type ReactNode } from "react";
import { listen } from "@tauri-apps/api/event";
import { HelpGuideGrid } from "./components/HelpGuideGrid";
import { PageHeader } from "./components/PageHeader";
import { WikiBulkImportModal } from "./components/WikiBulkImportModal";
import { WikiBulkImportProvider, useWikiBulkImportContext } from "./contexts/WikiBulkImportContext";
import { IconSettings, IconPersona, IconHelp } from "./components/Icons";
import { waitForTauriInternals } from "./lib/tauriInvoke";

const PersonaPanel = lazy(() =>
  import("./pages/PersonaPanel").then((m) => ({ default: m.PersonaPanel })),
);
const SettingsPanel = lazy(() =>
  import("./pages/SettingsPanel").then((m) => ({ default: m.SettingsPanel })),
);

function PageLoading() {
  return (
    <div className="panel">
      <p className="empty empty--compact">加载中…</p>
    </div>
  );
}

function LazyPage({ children }: { children: ReactNode }) {
  return <Suspense fallback={<PageLoading />}>{children}</Suspense>;
}

function PageSlot({
  active,
  children,
}: {
  active: boolean;
  children: ReactNode;
}) {
  return (
    <div className="page-slot" hidden={!active} aria-hidden={!active}>
      {children}
    </div>
  );
}

type Page = "persona" | "settings" | "help";

const PAGE_META: Record<Page, { title: string; subtitle?: string }> = {
  persona: {
    title: "人物",
    subtitle: "皮肤与桌宠模型统一管理",
  },
  settings: { title: "设置" },
  help: { title: "帮助" },
};

const NAV: { id: Page; label: string; icon: React.ReactNode }[] = [
  { id: "persona", label: "人物", icon: <IconPersona /> },
  { id: "settings", label: "设置", icon: <IconSettings /> },
  { id: "help", label: "帮助", icon: <IconHelp /> },
];

const PAGE_IDS = new Set<string>(Object.keys(PAGE_META));

export default function App() {
  return (
    <WikiBulkImportProvider>
      <AppShell />
    </WikiBulkImportProvider>
  );
}

function AppShell() {
  const bulk = useWikiBulkImportContext();
  const [page, setPage] = useState<Page>("persona");
  const [mounted, setMounted] = useState<Record<Page, boolean>>({
    persona: true,
    settings: false,
    help: false,
  });

  useEffect(() => {
    setMounted((prev) => (prev[page] ? prev : { ...prev, [page]: true }));
  }, [page]);

  useEffect(() => {
    void import("./pages/SettingsPanel");
  }, []);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    void waitForTauriInternals()
      .then(() => listen<string>("main-navigate", (ev) => {
        const raw = ev.payload?.trim();
        const id = raw === "pet" ? "persona" : raw;
        if (id && PAGE_IDS.has(id)) setPage(id as Page);
      }))
      .then((fn) => {
        unlisten = fn;
      })
      .catch(() => {});
    return () => {
      unlisten?.();
    };
  }, []);

  const meta = PAGE_META[page];

  return (
    <div className="app">
      <aside className="sidebar">
        <div className="sidebar-brand">
          <div className="brand-icon brand-icon--app">
            <img src="/app-icon.png" alt="" width={28} height={28} />
          </div>
          <div className="brand-text">
            <div className="brand-name">小寒桌宠</div>
            <div className="brand-subtitle">碧蓝航线桌宠启动器</div>
          </div>
        </div>

        <nav className="sidebar-nav-main">
          {NAV.map((item) => (
            <button
              key={item.id}
              type="button"
              className={`nav-item${page === item.id ? " active" : ""}`}
              onClick={() => setPage(item.id)}
            >
              {item.icon}
              {item.label}
            </button>
          ))}
        </nav>
        <p className="sidebar-tagline">Live2D · 本地台词 · 纯桌宠</p>
      </aside>

      <main
        className={`content${page === "persona" ? " content--persona" : " content--scroll content--secondary"}`}
      >
        <WikiBulkImportModal
          open={bulk.open}
          progress={bulk.progress}
          isActive={bulk.isActive}
          isPaused={bulk.isPaused}
          onPause={() => void bulk.pause()}
          onResume={() => void bulk.resume()}
          onStop={() => void bulk.stop()}
          onDismiss={bulk.dismiss}
        />
        {page !== "persona" && <PageHeader title={meta.title} subtitle={meta.subtitle} />}

        {mounted.persona && (
          <PageSlot active={page === "persona"}>
            <LazyPage>
              <PersonaPanel />
            </LazyPage>
          </PageSlot>
        )}
        {mounted.settings && (
          <PageSlot active={page === "settings"}>
            <LazyPage>
              <SettingsPanel />
            </LazyPage>
          </PageSlot>
        )}
        {mounted.help && (
          <PageSlot active={page === "help"}>
            <HelpGuideGrid />
          </PageSlot>
        )}
      </main>
    </div>
  );
}
