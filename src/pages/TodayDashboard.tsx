import type {
  OverviewPayload,
  PeriodSummary,
  HeatmapDay,
  WorkType,
} from "../lib/xiaohan";
import { formatUsageMs } from "../lib/formatUsageMs";
import { IconCat, IconShield } from "../components/Icons";
import { TodayHeatmap } from "../components/TodayHeatmap";

interface Props {
  overview: OverviewPayload | null;
  heatmap: HeatmapDay[];
  workTypes: WorkType[];
  periodSummaries: PeriodSummary[];
}

export function TodayDashboard({
  overview,
  heatmap,
  workTypes,
  periodSummaries,
}: Props) {
  const todayHeatmap = heatmap.find((d) => d.label === "今天") ?? heatmap[0] ?? null;
  const fgHours = overview ? (overview.foreground_ms / 3_600_000).toFixed(1) : "0";
  const appUsage = formatUsageMs(overview?.app_usage_ms ?? 0);
  const companion = formatUsageMs(overview?.companion_ms ?? 0);
  const mainWork =
    periodSummaries[0]?.work_type ?? overview?.top_app_display ?? "暂无";

  const quote = overview
    ? periodSummaries[0]?.summary ??
      `今天在「${overview.top_app_display ?? "各种应用"}」上待得最久，节奏还不错～`
    : "开工后小寒会悄悄记下你的一天，随手就能写成日报。";

  return (
    <div className="page-stack today-page">
      <div className="panel hero-panel hero-panel--today">
        <div className="hero-icon-wrap">
          <IconCat />
        </div>
        <div className="hero-body">
          <h2>小寒陪你，把今天收成一篇小记</h2>
          <p>屏幕前的碎片自动记下，想写的时候点一下就好。</p>
          <div className="hero-tags">
            <span className="tag">
              <IconShield /> 截图阅后即焚
            </span>
            <span className="tag">只存本机</span>
            <span className="tag">🐾 桌宠陪写</span>
          </div>
        </div>
      </div>

      <div className="panel work-overview-card">
        <div className="panel-title">今日一览</div>
        <div className="work-overview-quote">
          <span className="work-overview-quote-avatar" aria-hidden>
            ❄️
          </span>
          <p>{quote}</p>
        </div>
        <div className="work-overview-stats">
          <div className="work-stat">
            <div className="work-stat-icon" aria-hidden>
              🌙
            </div>
            <div className="work-stat-value">{appUsage}</div>
            <div className="work-stat-label">应用时长</div>
          </div>
          <div className="work-stat">
            <div className="work-stat-icon" aria-hidden>
              🐾
            </div>
            <div className="work-stat-value">{companion}</div>
            <div className="work-stat-label">陪伴时长</div>
          </div>
          <div className="work-stat">
            <div className="work-stat-icon" aria-hidden>
              📝
            </div>
            <div className="work-stat-value">{overview?.switch_count ?? 0}</div>
            <div className="work-stat-label">记录条数</div>
          </div>
          <div className="work-stat">
            <div className="work-stat-icon" aria-hidden>
              ⏱️
            </div>
            <div className="work-stat-value">{fgHours}h</div>
            <div className="work-stat-label">专注时长</div>
          </div>
          <div className="work-stat">
            <div className="work-stat-icon" aria-hidden>
              💻
            </div>
            <div className="work-stat-value work-stat-value--text">{mainWork}</div>
            <div className="work-stat-label">主要工作</div>
          </div>
        </div>
      </div>

      <div className="panel today-heatmap-panel">
        <TodayHeatmap day={todayHeatmap} workTypes={workTypes} />
      </div>
    </div>
  );
}
