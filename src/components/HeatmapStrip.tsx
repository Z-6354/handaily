import type { HeatmapDay, WorkType } from "../lib/xiaohan";
import { formatHours } from "../lib/xiaohan";

interface Props {
  days: HeatmapDay[];
  workTypes: WorkType[];
  formatDuration: (ms: number) => string;
}

const AXIS_TICKS = [0, 6, 12, 18, 23];

function colorForType(name: string | null, types: WorkType[]): string {
  if (!name) return "var(--heatmap-0)";
  const t = types.find((x) => x.name === name);
  return t?.color ?? "#d9d9d9";
}

function intensityAlpha(ms: number, maxMs: number): number {
  if (ms === 0) return 0.15;
  const r = ms / maxMs;
  if (r < 0.15) return 0.35;
  if (r < 0.4) return 0.55;
  if (r < 0.7) return 0.75;
  return 1;
}

export function HeatmapStrip({ days, workTypes, formatDuration }: Props) {
  const maxSlot = Math.max(...days.flatMap((d) => d.slots), 1);

  return (
    <div className="heatmap-strip-wrap">
      <div className="heatmap-strip-legend">
        {workTypes.map((t) => (
          <span className="wt-legend-item" key={t.id}>
            <i style={{ background: t.color }} />
            {t.name}
          </span>
        ))}
      </div>

      <div className="heatmap-strips">
        {days.map((day) => (
          <div className="heatmap-strip-row" key={day.date} title={`${day.label} · ${formatHours(day.total_ms)}`}>
            <span className="heatmap-strip-label">{day.label}</span>
            <div className="heatmap-strip-track">
              {day.slots.map((ms, hour) => {
                const wt = day.work_types[hour];
                const summary = day.summaries[hour];
                const level = ms === 0 ? 0 : intensityAlpha(ms, maxSlot) >= 0.75 ? 4 : intensityAlpha(ms, maxSlot) >= 0.55 ? 3 : intensityAlpha(ms, maxSlot) >= 0.35 ? 2 : 1;
                return (
                  <div
                    key={hour}
                    className={`heatmap-strip-cell${wt ? "" : ` level-${level}`}`}
                    style={wt ? { background: colorForType(wt, workTypes) } : undefined}
                    title={
                      summary
                        ? `${hour}:00 · ${wt} · ${summary} · ${formatDuration(ms)}`
                        : `${day.label} ${hour}:00–${hour + 1}:00 · ${formatDuration(ms)}`
                    }
                  />
                );
              })}
            </div>
          </div>
        ))}
      </div>

      <div className="heatmap-strip-axis">
        <span className="heatmap-strip-label" />
        <div className="heatmap-strip-track heatmap-strip-axis-labels">
          {AXIS_TICKS.map((h) => (
            <span key={h} style={{ gridColumn: h + 1 }}>
              {h}
            </span>
          ))}
        </div>
      </div>
    </div>
  );
}
