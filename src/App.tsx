import { lazy, Suspense, useEffect, useRef, useState, type ReactNode } from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  xiaohan,
  type OverviewPayload,
  type AppBreakdownItem,
  type WorkType,
  type PeriodSummary,
  type HeatmapDay,
} from "./lib/xiaohan";
import { TodayDashboard } from "./pages/TodayDashboard";
import { HelpGuideGrid } from "./components/HelpGuideGrid";
import { PageHeader } from "./components/PageHeader";
import {
  IconChart,
  IconTimeline,
  IconSettings,
  IconVault,
  IconReport,
  IconHeatmap,
  IconApps,
  IconHistory,
  IconAgent,
  IconPersona,
  IconHelp,
  IconPerformance,
} from "./components/Icons";

const TimelineView = lazy(() =>
  import("./pages/TimelineView").then((m) => ({ default: m.TimelineView })),
);
const HeatmapPage = lazy(() =>
  import("./pages/HeatmapPage").then((m) => ({ default: m.HeatmapPage })),
);
const AppRecordsPage = lazy(() =>
  import("./pages/AppRecordsPage").then((m) => ({ default: m.AppRecordsPage })),
);
const ReportGeneratePage = lazy(() =>
  import("./pages/ReportGeneratePage").then((m) => ({ default: m.ReportGeneratePage })),
);
const HistoryReportsPage = lazy(() =>
  import("./pages/HistoryReportsPage").then((m) => ({ default: m.HistoryReportsPage })),
);
const AgentConnectPage = lazy(() =>
  import("./pages/AgentConnectPage").then((m) => ({ default: m.AgentConnectPage })),
);
const PersonaPanel = lazy(() =>
  import("./pages/PersonaPanel").then((m) => ({ default: m.PersonaPanel })),
);
const VaultPanel = lazy(() => import("./pages/VaultPanel").then((m) => ({ default: m.VaultPanel })));
const SettingsPanel = lazy(() =>
  import("./pages/SettingsPanel").then((m) => ({ default: m.SettingsPanel })),
);
const PerformancePage = lazy(() =>
  import("./pages/PerformancePage").then((m) => ({ default: m.PerformancePage })),
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

type Page =
  | "today"
  | "report"
  | "timeline"
  | "heatmap"
  | "apps"
  | "history"
  | "agent"
  | "persona"
  | "vault"
  | "settings"
  | "performance"
  | "help";

const PAGE_META: Record<Page, { title: string; subtitle?: string }> = {
  today: { title: "今日工作", subtitle: "小寒帮你瞄一眼今天的节奏" },
  report: { title: "生成报告", subtitle: "把碎片收成一篇给自己看的小记" },
  timeline: { title: "工作时间线", subtitle: "按时间轴翻翻今天切换了什么" },
  heatmap: { title: "时段热力图", subtitle: "什么时候最忙，一眼就知道" },
  apps: { title: "应用记录", subtitle: "时间都花在哪些软件上啦" },
  history: { title: "历史报告", subtitle: "以前写好的小记都在这里" },
  agent: { title: "接入 Agent", subtitle: "让别的助手也能读到你的工作记录" },
  persona: {
    title: "人物",
    subtitle: "性格、皮肤与桌宠模型统一管理",
  },
  vault: { title: "密码本", subtitle: "API 密钥本地加密保存" },
  settings: { title: "设置", subtitle: "模型、人设与采集习惯" },
  performance: { title: "性能检测", subtitle: "系统与本应用的实时占用" },
  help: { title: "帮助", subtitle: "快速认识小寒日报" },
};

const MAIN_NAV: { id: Page; label: string; icon: React.ReactNode }[] = [
  { id: "today", label: "今日工作", icon: <IconChart /> },
  { id: "report", label: "生成报告", icon: <IconReport /> },
  { id: "timeline", label: "工作时间线", icon: <IconTimeline /> },
  { id: "heatmap", label: "时段热力图", icon: <IconHeatmap /> },
  { id: "apps", label: "应用记录", icon: <IconApps /> },
  { id: "history", label: "历史报告", icon: <IconHistory /> },
  { id: "agent", label: "接入 Agent", icon: <IconAgent /> },
  { id: "persona", label: "人物", icon: <IconPersona /> },
];

const MORE_NAV: { id: Page; label: string; icon: React.ReactNode }[] = [
  { id: "performance", label: "性能检测", icon: <IconPerformance /> },
  { id: "vault", label: "密码本", icon: <IconVault /> },
  { id: "settings", label: "设置", icon: <IconSettings /> },
  { id: "help", label: "帮助", icon: <IconHelp /> },
];

const PAGE_IDS = new Set<string>(Object.keys(PAGE_META));

const HEAVY_PAGES = new Set<Page>(["today", "heatmap", "apps"]);
const LIGHT_INTERVAL_MS = 5000;
const HEAVY_INTERVAL_MS = 30000;

export default function App() {
  const [page, setPage] = useState<Page>("today");
  const [overview, setOverview] = useState<OverviewPayload | null>(null);
  const [breakdown, setBreakdown] = useState<AppBreakdownItem[]>([]);
  const [heatmap, setHeatmap] = useState<HeatmapDay[]>([]);
  const [workTypes, setWorkTypes] = useState<WorkType[]>([]);
  const [periodSummaries, setPeriodSummaries] = useState<PeriodSummary[]>([]);
  const [tracking, setTracking] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const refreshGenRef = useRef(0);
  const pageRef = useRef<Page>(page);
  const windowVisibleRef = useRef(false);
  pageRef.current = page;

  const refreshLight = async (gen: number) => {
    const [ov, status, wt] = await Promise.all([
      xiaohan.getOverview(),
      xiaohan.getStatus(),
      xiaohan.workTypesGet(),
    ]);
    if (gen !== refreshGenRef.current) return;
    setOverview(ov);
    setWorkTypes(wt.types);
    setTracking(status.tracking);
  };

  const refreshHeavy = async (gen: number) => {
    const [bd, hm, ps] = await Promise.all([
      xiaohan.getAppBreakdown(),
      xiaohan.getThreeDayHeatmap(),
      xiaohan.periodListSummaries(20),
    ]);
    if (gen !== refreshGenRef.current) return;
    setBreakdown(bd);
    setHeatmap(hm);
    setPeriodSummaries(ps);
  };

  const refresh = async (opts?: { heavy?: boolean }) => {
    if (!windowVisibleRef.current) return;
    const gen = ++refreshGenRef.current;
    try {
      setError(null);
      await refreshLight(gen);
      if (gen !== refreshGenRef.current) return;
      const wantHeavy =
        opts?.heavy ?? HEAVY_PAGES.has(pageRef.current);
      if (wantHeavy) {
        await refreshHeavy(gen);
      }
    } catch (e) {
      if (gen !== refreshGenRef.current) return;
      setError(String(e));
    }
  };

  useEffect(() => {
    let lastHeavy = Date.now();

    void getCurrentWindow()
      .isVisible()
      .then((v) => {
        windowVisibleRef.current = v;
        if (v) void refresh({ heavy: true });
      })
      .catch(() => {
        windowVisibleRef.current = true;
        void refresh({ heavy: true });
      });

    const lightId = setInterval(() => {
      if (!windowVisibleRef.current) return;
      const now = Date.now();
      const onHeavyPage = HEAVY_PAGES.has(pageRef.current);
      const heavyDue = onHeavyPage && now - lastHeavy >= HEAVY_INTERVAL_MS;
      if (heavyDue) lastHeavy = now;
      void refresh({ heavy: heavyDue });
    }, LIGHT_INTERVAL_MS);

    let unlistenVisible: (() => void) | undefined;
    void listen<boolean>("main-window-visible", (ev) => {
      windowVisibleRef.current = Boolean(ev.payload);
      if (windowVisibleRef.current) {
        void refresh({ heavy: HEAVY_PAGES.has(pageRef.current) });
      }
    }).then((fn) => {
      unlistenVisible = fn;
    });

    let unlistenFocus: (() => void) | undefined;
    void getCurrentWindow()
      .listen("tauri://focus", () => {
        if (!windowVisibleRef.current) return;
        void refresh({ heavy: HEAVY_PAGES.has(pageRef.current) });
      })
      .then((fn) => {
        unlistenFocus = fn;
      });

    return () => {
      clearInterval(lightId);
      unlistenVisible?.();
      unlistenFocus?.();
    };
  }, []);

  useEffect(() => {
    if (HEAVY_PAGES.has(page)) {
      void refresh({ heavy: true });
    }
  }, [page]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    void listen<string>("main-navigate", (ev) => {
      const raw = ev.payload?.trim();
      const id = raw === "pet" ? "persona" : raw;
      if (id && PAGE_IDS.has(id)) setPage(id as Page);
    }).then((fn) => {
      unlisten = fn;
    });
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
          <div className="brand-name">小寒日报助手</div>
        </div>

        <nav className="sidebar-nav-main">
          {MAIN_NAV.map((item) => (
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

        <div className="nav-section-label">更多</div>
        <nav className="sidebar-nav-more">
          {MORE_NAV.map((item) => (
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

        <div className="sidebar-footer">
          <span className={`status-dot${tracking ? "" : " paused"}`} />
          <span className="status-text">{tracking ? "后台采集中" : "采集已暂停"}</span>
        </div>
      </aside>

      <main className={`content${page === "persona" ? " content--persona" : ""}`}>
        {page !== "persona" && <PageHeader title={meta.title} subtitle={meta.subtitle} />}

        {error && <div className="error">加载失败：{error}</div>}

        {page === "today" && (
          <TodayDashboard
            overview={overview}
            heatmap={heatmap}
            workTypes={workTypes}
            periodSummaries={periodSummaries}
          />
        )}
        {page === "report" && (
          <LazyPage>
            <ReportGeneratePage />
          </LazyPage>
        )}
        {page === "timeline" && (
          <LazyPage>
            <TimelineView active />
          </LazyPage>
        )}
        {page === "heatmap" && (
          <LazyPage>
            <HeatmapPage heatmap={heatmap} workTypes={workTypes} onRefresh={() => refresh({ heavy: true })} />
          </LazyPage>
        )}
        {page === "apps" && (
          <LazyPage>
            <AppRecordsPage breakdown={breakdown} />
          </LazyPage>
        )}
        {page === "history" && (
          <LazyPage>
            <HistoryReportsPage />
          </LazyPage>
        )}
        {page === "agent" && (
          <LazyPage>
            <AgentConnectPage />
          </LazyPage>
        )}
        {page === "persona" && (
          <LazyPage>
            <PersonaPanel />
          </LazyPage>
        )}
        {page === "vault" && (
          <LazyPage>
            <VaultPanel />
          </LazyPage>
        )}
        {page === "settings" && (
          <LazyPage>
            <SettingsPanel onTrackingChange={setTracking} />
          </LazyPage>
        )}
        {page === "performance" && (
          <LazyPage>
            <PerformancePage />
          </LazyPage>
        )}
        {page === "help" && (
          <div className="panel">
            <HelpGuideGrid />
          </div>
        )}
      </main>
    </div>
  );
}
