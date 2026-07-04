import { useEffect, useRef, useState } from "react";
import {
  xiaohan,
  type OverviewPayload,
  type AppBreakdownItem,
  type WorkType,
  type PeriodSummary,
  type HeatmapDay,
} from "./lib/xiaohan";
import { TodayDashboard } from "./pages/TodayDashboard";
import { TimelineView } from "./pages/TimelineView";
import { PetPanel } from "./pages/PetPanel";
import { SettingsPanel } from "./pages/SettingsPanel";
import { PersonaPanel } from "./pages/PersonaPanel";
import { VaultPanel } from "./pages/VaultPanel";
import { HeatmapPage } from "./pages/HeatmapPage";
import { AppRecordsPage } from "./pages/AppRecordsPage";
import { ReportGeneratePage } from "./pages/ReportGeneratePage";
import { HistoryReportsPage } from "./pages/HistoryReportsPage";
import { AgentConnectPage } from "./pages/AgentConnectPage";
import { PerformancePage } from "./pages/PerformancePage";
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
  IconPet,
  IconPerformance,
} from "./components/Icons";

type Page =
  | "today"
  | "report"
  | "timeline"
  | "heatmap"
  | "apps"
  | "history"
  | "agent"
  | "persona"
  | "pet"
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
  persona: { title: "AI 人设", subtitle: "换种语气，总结更像在聊天" },
  pet: { title: "桌宠" },
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
  { id: "persona", label: "AI 人设", icon: <IconPersona /> },
  { id: "pet", label: "桌宠", icon: <IconPet /> },
];

const MORE_NAV: { id: Page; label: string; icon: React.ReactNode }[] = [
  { id: "performance", label: "性能检测", icon: <IconPerformance /> },
  { id: "vault", label: "密码本", icon: <IconVault /> },
  { id: "settings", label: "设置", icon: <IconSettings /> },
  { id: "help", label: "帮助", icon: <IconHelp /> },
];

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

  const refresh = async () => {
    const gen = ++refreshGenRef.current;
    try {
      setError(null);
      const [ov, bd, hm, status, wt, ps] = await Promise.all([
        xiaohan.getOverview(),
        xiaohan.getAppBreakdown(),
        xiaohan.getThreeDayHeatmap(),
        xiaohan.getStatus(),
        xiaohan.workTypesGet(),
        xiaohan.periodListSummaries(20),
      ]);
      if (gen !== refreshGenRef.current) return;
      setOverview(ov);
      setBreakdown(bd);
      setHeatmap(hm);
      setWorkTypes(wt.types);
      setPeriodSummaries(ps);
      setTracking(status.tracking);
    } catch (e) {
      if (gen !== refreshGenRef.current) return;
      setError(String(e));
    }
  };

  useEffect(() => {
    refresh();
    const id = setInterval(refresh, 5000);
    return () => clearInterval(id);
  }, []);

  const meta = PAGE_META[page];

  return (
    <div className="app">
      <aside className="sidebar">
        <div className="sidebar-brand">
          <div className="brand-icon">
            <IconChart />
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
        {page !== "pet" && <PageHeader title={meta.title} subtitle={meta.subtitle} />}

        {error && <div className="error">加载失败：{error}</div>}

        {page === "today" && (
          <TodayDashboard
            overview={overview}
            heatmap={heatmap}
            workTypes={workTypes}
            periodSummaries={periodSummaries}
          />
        )}
        {page === "report" && <ReportGeneratePage />}
        {page === "timeline" && <TimelineView active />}
        {page === "heatmap" && (
          <HeatmapPage heatmap={heatmap} workTypes={workTypes} onRefresh={refresh} />
        )}
        {page === "apps" && <AppRecordsPage breakdown={breakdown} />}
        {page === "history" && <HistoryReportsPage />}
        {page === "agent" && <AgentConnectPage />}
        {page === "persona" && <PersonaPanel />}
        {page === "pet" && <PetPanel />}
        {page === "vault" && <VaultPanel />}
        {page === "settings" && <SettingsPanel onTrackingChange={setTracking} />}
        {page === "performance" && <PerformancePage />}
        {page === "help" && (
          <div className="panel">
            <HelpGuideGrid />
          </div>
        )}
      </main>
    </div>
  );
}
