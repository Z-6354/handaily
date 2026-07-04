import type { HeatmapDay, WorkType } from "../lib/xiaohan";
import { formatDuration, formatHours } from "../lib/xiaohan";
import { StatBar } from "../components/StatBar";
import { EmptyState } from "../components/EmptyState";

interface Props {
  heatmap: HeatmapDay[];
  workTypes: WorkType[];
  onRefresh: () => void;
}

const AXIS_TICKS = [0, 3, 6, 9, 12, 15, 18, 21];

function cellLevel(ms: number, max: number): number {
  if (ms === 0) return 0;
  const r = ms / max;
  if (r < 0.15) return 1;
  if (r < 0.4) return 2;
  if (r < 0.7) return 3;
  return 4;
}

function colorForType(name: string | null, types: WorkType[]): string | undefined {
  if (!name) return undefined;
  return types.find((t) => t.name === name)?.color;
}

export function HeatmapPage({ heatmap, workTypes, onRefresh }: Props) {
  const maxSlot = Math.max(...heatmap.flatMap((d) => d.slots), 1);
  const totalRecords = heatmap.reduce((a, d) => a + d.segment_count, 0);
  const totalMs = heatmap.reduce((a, d) => a + d.total_ms, 0);
  const activeDays = heatmap.filter((d) => d.total_ms > 0).length;

  return (
    <div className="page-stack">
      <StatBar
        stats={[
          { value: totalRecords, label: "记录条数" },
          { value: formatHours(totalMs) || "0m", label: "专注时长" },
          { value: activeDays, label: "活跃天数" },
          {
            value: activeDays ? Math.round(totalRecords / activeDays) : 0,
            label: "日均记录",
          },
        ]}
        trailing={<span className="stat-bar-quote">专注工作本身，剩下的交给小寒</span>}
      />

      <div className="panel">
        <div className="panel-header">
          <div className="panel-title">时段记录</div>
          <div className="heatmap-legend">
            <span>少</span>
            <div className="legend-scale">
              {[0, 1, 2, 3, 4].map((l) => (
                <div key={l} className={`legend-cell level-${l}`} />
              ))}
            </div>
            <span>多</span>
          </div>
          <button className="btn-refresh" onClick={onRefresh}>
            刷新
          </button>
        </div>

        {workTypes.length > 0 && (
          <div className="heatmap-strip-legend heatmap-strip-legend--page">
            {workTypes.map((t) => (
              <span className="wt-legend-item" key={t.id}>
                <i style={{ background: t.color }} />
                {t.name}
              </span>
            ))}
          </div>
        )}

        {heatmap.length === 0 ? (
          <EmptyState message="暂无时段数据" hint="开始工作后热力图将自动更新" />
        ) : (
          <>
            <div className="heatmap-grid heatmap-grid--page">
              {heatmap.map((day) => (
                <div className="heatmap-row heatmap-row--page" key={day.date}>
                  <div className="heatmap-label">
                    <div>{day.label}</div>
                    <div className="heatmap-meta">
                      {day.segment_count}条 · {formatHours(day.total_ms)}
                    </div>
                  </div>
                  <div className="heatmap-cells heatmap-cells--24">
                    {day.slots.map((ms, hour) => {
                      const wt = day.work_types?.[hour];
                      const summary = day.summaries?.[hour];
                      const level = cellLevel(ms, maxSlot);
                      const wtColor = colorForType(wt ?? null, workTypes);
                      return (
                        <div
                          key={hour}
                          className={`heatmap-cell heatmap-cell--hour${wtColor ? "" : ` level-${level}`}`}
                          style={wtColor ? { background: wtColor } : undefined}
                          title={
                            summary
                              ? `${hour}:00 · ${wt} · ${summary} · ${formatDuration(ms)}`
                              : `${day.label} ${hour}:00 · ${formatDuration(ms)}`
                          }
                        />
                      );
                    })}
                  </div>
                </div>
              ))}
            </div>
            <div className="heatmap-axis heatmap-axis--24">
              <div className="heatmap-axis-spacer" />
              <div className="heatmap-axis-labels heatmap-axis-labels--24">
                {AXIS_TICKS.map((h) => (
                  <span key={h} style={{ gridColumn: h + 1 }}>
                    {h}:00
                  </span>
                ))}
              </div>
            </div>
          </>
        )}
      </div>
    </div>
  );
}
