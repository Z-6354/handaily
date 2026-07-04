import { useState } from "react";
import type { AppBreakdownItem } from "../lib/xiaohan";
import { formatDuration } from "../lib/xiaohan";
import { AppIcon } from "../components/AppIcon";
import { StatBar } from "../components/StatBar";
import { EmptyState } from "../components/EmptyState";

interface Props {
  breakdown: AppBreakdownItem[];
}

type Range = "today" | "week" | "month";

export function AppRecordsPage({ breakdown }: Props) {
  const [range, setRange] = useState<Range>("today");
  const [chartMode, setChartMode] = useState<"bar" | "list">("bar");

  const totalMs = breakdown.reduce((a, b) => a + b.ms, 0);
  const maxMs = breakdown[0]?.ms ?? 1;

  return (
    <div className="page-stack">
      <div className="toolbar-row">
        <div className="segmented">
          {(["today", "week", "month"] as Range[]).map((r) => (
            <button
              key={r}
              type="button"
              className={`segmented-item${range === r ? " active" : ""}`}
              onClick={() => setRange(r)}
            >
              {r === "today" ? "今日" : r === "week" ? "本周" : "本月"}
            </button>
          ))}
        </div>
        {range !== "today" && (
          <span className="hint">本周/本月统计即将推出，当前显示今日数据</span>
        )}
      </div>

      <StatBar
        stats={[
          { value: breakdown.length, label: "总应用数" },
          { value: formatDuration(totalMs), label: "总时长" },
          {
            value: breakdown[0] ? formatDuration(breakdown[0].ms) : "—",
            label: "最长应用时长",
          },
        ]}
      />

      <div className="panel">
        <div className="panel-header">
          <div className="panel-title">应用时长分布（Top 20）</div>
          <div className="segmented segmented--sm">
            <button
              type="button"
              className={`segmented-item${chartMode === "bar" ? " active" : ""}`}
              onClick={() => setChartMode("bar")}
            >
              柱状图
            </button>
            <button
              type="button"
              className={`segmented-item${chartMode === "list" ? " active" : ""}`}
              onClick={() => setChartMode("list")}
            >
              列表
            </button>
          </div>
        </div>

        {breakdown.length === 0 ? (
          <EmptyState message="暂无应用使用数据" hint="切换应用后记录将自动出现" />
        ) : chartMode === "bar" ? (
          <div className="bar-chart">
            {breakdown.slice(0, 20).map((item, i) => (
              <div className="bar-row" key={i}>
                <div className="bar-label" title={item.key}>
                  <AppIcon icon={item.icon} name={item.display_name} />
                  <span>{item.display_name}</span>
                </div>
                <div className="bar-track">
                  <div className="bar-fill" style={{ width: `${(item.ms / maxMs) * 100}%` }} />
                </div>
                <div className="bar-value">{formatDuration(item.ms)}</div>
              </div>
            ))}
          </div>
        ) : (
          <table className="timeline-table">
            <thead>
              <tr>
                <th>#</th>
                <th>应用</th>
                <th>时长</th>
                <th>占比</th>
              </tr>
            </thead>
            <tbody>
              {breakdown.slice(0, 20).map((item, i) => (
                <tr key={i}>
                  <td>{i + 1}</td>
                  <td className="app-cell">
                    <span className="app-cell-inner">
                      <AppIcon icon={item.icon} name={item.display_name} />
                      {item.display_name}
                    </span>
                  </td>
                  <td className="dur-cell">{formatDuration(item.ms)}</td>
                  <td>{totalMs ? `${((item.ms / totalMs) * 100).toFixed(1)}%` : "—"}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>
    </div>
  );
}
