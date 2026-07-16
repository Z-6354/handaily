import { useCallback, useEffect, useState } from "react";
import { StatBar } from "../components/StatBar";
import { xiaohan, type PerformanceSnapshot } from "../lib/xiaohan";

function formatBytes(bytes: number): string {
  if (bytes <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${value >= 100 || unit === 0 ? value.toFixed(0) : value.toFixed(1)} ${units[unit]}`;
}

function formatPercent(value: number): string {
  return `${value.toFixed(1)}%`;
}

function clampPercent(value: number): number {
  return Math.max(0, Math.min(100, value));
}

interface MeterProps {
  label: string;
  value: number;
  detail: string;
  tone?: "system" | "app";
}

function PerfMeter({ label, value, detail, tone = "system" }: MeterProps) {
  const pct = clampPercent(value);
  return (
    <div className="perf-meter">
      <div className="perf-meter-head">
        <span className="perf-meter-label">{label}</span>
        <span className="perf-meter-value">{formatPercent(value)}</span>
      </div>
      <div className="perf-meter-track" role="presentation">
        <div
          className={`perf-meter-fill perf-meter-fill--${tone}`}
          style={{ width: `${pct}%` }}
        />
      </div>
      <div className="perf-meter-detail">{detail}</div>
    </div>
  );
}

export function PerformancePage({ active = true }: { active?: boolean }) {
  const [snapshot, setSnapshot] = useState<PerformanceSnapshot | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [refreshing, setRefreshing] = useState(false);

  const refresh = useCallback(async (manual = false) => {
    if (manual) setRefreshing(true);
    try {
      setError(null);
      const data = await xiaohan.getPerformanceSnapshot();
      setSnapshot(data);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
      if (manual) setRefreshing(false);
    }
  }, []);

  useEffect(() => {
    if (!active) return;
    void refresh();
    const id = setInterval(() => {
      if (document.visibilityState === "visible") void refresh();
    }, 2500);
    return () => clearInterval(id);
  }, [refresh, active]);

  if (loading && !snapshot) {
    return (
      <div className="panel">
        <p className="hint-block">正在采样系统性能…</p>
      </div>
    );
  }

  const snap = snapshot;
  const memUsed = snap?.systemMemoryUsedBytes ?? 0;
  const memTotal = snap?.systemMemoryTotalBytes ?? 0;

  return (
    <div className="page-stack">
      <StatBar
        stats={[
          { value: snap ? formatPercent(snap.systemCpuPercent) : "—", label: "系统 CPU" },
          { value: snap ? formatPercent(snap.systemMemoryPercent) : "—", label: "系统内存" },
          { value: snap ? formatPercent(snap.appCpuPercent) : "—", label: "本应用 CPU" },
          {
            value: snap ? formatBytes(snap.appMemoryWorkingSetBytes) : "—",
            label: "本应用内存",
          },
        ]}
        trailing={
          <button
            type="button"
            className="btn-refresh"
            disabled={refreshing}
            onClick={() => void refresh(true)}
          >
            {refreshing ? "采样中…" : "立即刷新"}
          </button>
        }
      />

      {error && <div className="error">读取失败：{error}</div>}

      <div className="panel settings-card">
        <div className="panel-header">
          <div className="panel-title">系统性能</div>
        </div>
        <div className="perf-section">
          <PerfMeter
            label="CPU 占用"
            value={snap?.systemCpuPercent ?? 0}
            detail="全系统处理器使用率（约 0.2 秒采样）"
          />
          <PerfMeter
            label="内存占用"
            value={snap?.systemMemoryPercent ?? 0}
            detail={
              memTotal > 0
                ? `${formatBytes(memUsed)} / ${formatBytes(memTotal)}`
                : "物理内存使用情况"
            }
          />
        </div>
      </div>

      <div className="panel settings-card">
        <div className="panel-header">
          <div>
            <div className="panel-title">本应用性能</div>
            <div className="perf-panel-subtitle">{snap?.processName ?? "xiaohan-daily"}</div>
          </div>
        </div>
        <div className="perf-section">
          <PerfMeter
            label="CPU 占用"
            value={snap?.appCpuPercent ?? 0}
            detail="占系统总 CPU 时间的比例"
            tone="app"
          />
          <div className="perf-memory-grid">
            <div className="perf-memory-item">
              <div className="perf-memory-label">工作集</div>
              <div className="perf-memory-value">
                {snap ? formatBytes(snap.appMemoryWorkingSetBytes) : "—"}
              </div>
              <div className="perf-memory-hint">当前占用的物理内存</div>
            </div>
            <div className="perf-memory-item">
              <div className="perf-memory-label">提交内存</div>
              <div className="perf-memory-value">
                {snap ? formatBytes(snap.appMemoryPrivateBytes) : "—"}
              </div>
              <div className="perf-memory-hint">进程已提交的虚拟内存</div>
            </div>
          </div>
        </div>
      </div>

      <p className="hint-block perf-footnote">每 2.5 秒自动刷新；CPU 采样会短暂阻塞约 0.2 秒。</p>
    </div>
  );
}
