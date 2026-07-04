import type { HeatmapDay, WorkType } from "../lib/xiaohan";
import { formatDuration } from "../lib/xiaohan";

interface Props {
  day: HeatmapDay | null;
  workTypes: WorkType[];
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

/** 今日页内嵌的单日 24 小时热力条（对齐小黑日报） */
export function TodayHeatmap({ day, workTypes }: Props) {
  if (!day) {
    return <p className="empty empty--compact">暂无时段数据</p>;
  }

  const maxSlot = Math.max(...day.slots, 1);

  return (
    <div className="today-heatmap">
      <div className="today-heatmap-head">
        <span className="panel-title panel-title--inline">时段记录</span>
        <div className="heatmap-legend">
          <span>少</span>
          <div className="legend-scale">
            {[0, 1, 2, 3, 4].map((l) => (
              <div key={l} className={`legend-cell level-${l}`} />
            ))}
          </div>
          <span>多</span>
        </div>
      </div>

      <div className="today-heatmap-track">
        {day.slots.map((ms, hour) => {
          const wt = day.work_types?.[hour];
          const summary = day.summaries?.[hour];
          const level = cellLevel(ms, maxSlot);
          const wtColor = colorForType(wt ?? null, workTypes);
          const showCount = ms > 0 && level >= 3;
          return (
            <div
              key={hour}
              className={`today-heatmap-cell${wtColor ? "" : ` level-${level}`}${showCount ? " has-value" : ""}`}
              style={wtColor ? { background: wtColor } : undefined}
              title={
                summary
                  ? `${hour}:00 · ${wt} · ${summary} · ${formatDuration(ms)}`
                  : `${hour}:00 · ${formatDuration(ms)}`
              }
            >
              {showCount && <span className="cell-count">{Math.round(ms / 60_000) || 1}</span>}
            </div>
          );
        })}
      </div>

      <div className="today-heatmap-axis">
        {AXIS_TICKS.map((h) => (
          <span key={h} style={{ gridColumn: h + 1 }}>
            {h}:00
          </span>
        ))}
      </div>
    </div>
  );
}
