import { useCallback, useEffect, useState } from "react";
import { EmptyState } from "../components/EmptyState";
import { xiaohan, type GeneratedReport } from "../lib/xiaohan";

const TEMPLATE_LABEL: Record<string, string> = {
  "period-summary": "时段总结",
  "activity-log": "完成记录",
};

function formatCreatedAt(iso: string) {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  return d.toLocaleString("zh-CN", {
    month: "numeric",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

export function HistoryReportsPage() {
  const [reports, setReports] = useState<GeneratedReport[]>([]);
  const [selectedId, setSelectedId] = useState<number | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      setError(null);
      const list = await xiaohan.reportList(50);
      setReports(list);
      setSelectedId((prev) => {
        if (prev && list.some((r) => r.id === prev)) return prev;
        return list[0]?.id ?? null;
      });
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  const selected = reports.find((r) => r.id === selectedId);

  const remove = async (id: number) => {
    if (!confirm("删掉这份小记？删了就找不回来啦")) return;
    try {
      await xiaohan.reportDelete(id);
      await load();
    } catch (e) {
      setError(String(e));
    }
  };

  if (loading) {
    return <div className="panel">加载历史小记…</div>;
  }

  return (
    <div className="history-reports">
      {error && <div className="error">加载失败：{error}</div>}

      <div className="panel">
        <div className="panel-header">
          <div className="panel-title">我的小记</div>
          <span className="count-badge">共 {reports.length} 份</span>
        </div>

        {reports.length === 0 ? (
          <EmptyState
            message="还没有小记呢"
            hint="去「生成报告」选个模板，把今天收成第一篇吧～"
          />
        ) : (
          <div className="history-reports-layout">
            <ul className="history-report-list">
              {reports.map((r) => (
                <li key={r.id}>
                  <button
                    type="button"
                    className={`history-report-item${selectedId === r.id ? " active" : ""}`}
                    onClick={() => setSelectedId(r.id)}
                  >
                    <div className="history-report-item-title">{r.title}</div>
                    <div className="history-report-item-meta">
                      <span>{TEMPLATE_LABEL[r.template_id] ?? r.template_id}</span>
                      <span>{formatCreatedAt(r.created_at)}</span>
                      {r.used_ai && <span className="history-report-ai-tag">AI</span>}
                    </div>
                  </button>
                </li>
              ))}
            </ul>

            {selected && (
              <div className="history-report-detail panel">
                <div className="panel-header">
                  <div>
                    <div className="panel-title">{selected.title}</div>
                    <p className="panel-desc">
                      {selected.date_from === selected.date_to
                        ? selected.date_from
                        : `${selected.date_from} ～ ${selected.date_to}`}
                    </p>
                  </div>
                  <button
                    type="button"
                    className="btn-secondary btn-sm"
                    onClick={() => remove(selected.id)}
                  >
                    删除
                  </button>
                </div>
                <pre className="report-markdown-pre">{selected.content}</pre>
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
}
