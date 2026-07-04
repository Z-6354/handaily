import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  xiaohan,
  formatDuration,
  type Segment,
  type TimelineAiEntry,
  type TimelineDescribeChunkEvent,
  type WorkType,
} from "../lib/xiaohan";
import { parseApiError } from "../lib/apiErrorMessage";
import { AppIcon } from "../components/AppIcon";
import { EmptyState } from "../components/EmptyState";

type Filter = "30m" | "1h" | "2h" | "today";

function filterToMinutes(filter: Filter): number | undefined {
  switch (filter) {
    case "30m":
      return 30;
    case "1h":
      return 60;
    case "2h":
      return 120;
    default:
      return undefined;
  }
}

type TimelineViewProps = {
  active: boolean;
};

function formatApiError(raw: unknown, context: string): string {
  const fb = parseApiError(raw, context);
  return fb.detail ? `${fb.title}：${fb.detail}` : fb.title;
}

function tagColor(category: string, workTypes: WorkType[]): string {
  const t = workTypes.find((w) => w.name === category);
  if (t) return t.color;
  const map: Record<string, string> = {
    开发: "#22c55e",
    文档: "#8b5cf6",
    数据分析: "#3b82f6",
    会议: "#f59e0b",
    沟通: "#06b6d4",
  };
  return map[category] ?? "#94a3b8";
}

const AUDIO_LABELS: Record<string, string> = {
  music: "后台听歌",
  video: "后台看视频",
  chat: "后台聊天",
  other: "后台音频",
};

function audioBadge(seg: Segment): string | null {
  if (seg.source_type !== "audio") return null;
  return AUDIO_LABELS[seg.audio_activity ?? ""] ?? "后台音频";
}

function isRawWindowTitle(text: string): boolean {
  const t = text.trim();
  return (
    t.includes(" · 窗口「") ||
    t.startsWith("开发：") ||
    t.startsWith("文档：") ||
    t.startsWith("[text·") ||
    / - .+ - (Cursor|Visual Studio Code|Code|Microsoft Edge|Chrome|Firefox)$/i.test(t)
  );
}

function fallbackText(seg: Segment): string {
  const badge = audioBadge(seg);
  if (badge) {
    const title = seg.window_title?.trim();
    if (title && !title.startsWith("后台") && !isRawWindowTitle(title)) {
      return `${badge} · ${seg.app_name} ·「${title}」`;
    }
    return `${badge} · ${seg.app_name}`;
  }
  const label = seg.activity_label?.trim();
  if (label && !isRawWindowTitle(label)) {
    return `在 ${seg.app_name} · ${label}`;
  }
  const title = seg.window_title?.trim();
  if (title && !isRawWindowTitle(title)) {
    return `在 ${seg.app_name} ·「${title}」`;
  }
  return `在 ${seg.app_name} 里忙活呢~`;
}

function formatTimeRange(seg: Segment): string {
  const start = seg.started_at.slice(11, 19);
  const end = seg.ended_at?.slice(11, 19);
  if (end && end !== start) return `${start} – ${end}`;
  return start;
}

function mergeDescriptions(
  prev: TimelineAiEntry[],
  entries: TimelineAiEntry[],
): TimelineAiEntry[] {
  if (entries.length === 0) return prev;
  const map = new Map(prev.map((d) => [d.started_at, d]));
  for (const e of entries) {
    map.set(e.started_at, e);
  }
  return Array.from(map.values());
}

function prunePending(
  prev: Set<string>,
  entries: TimelineAiEntry[],
  clearAll: boolean,
): Set<string> {
  if (clearAll) return new Set();
  if (entries.length === 0) return prev;
  const next = new Set(prev);
  for (const e of entries) {
    next.delete(e.started_at);
  }
  return next;
}

export function TimelineView({ active }: TimelineViewProps) {
  const [items, setItems] = useState<Segment[]>([]);
  const [descriptions, setDescriptions] = useState<TimelineAiEntry[]>([]);
  const [workTypes, setWorkTypes] = useState<WorkType[]>([]);
  const [total, setTotal] = useState(0);
  const [offset, setOffset] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const [pendingDescribe, setPendingDescribe] = useState<Set<string>>(new Set());
  const [filter, setFilter] = useState<Filter>("today");
  const [copied, setCopied] = useState(false);
  const [describeError, setDescribeError] = useState<string | null>(null);
  const [logsPath, setLogsPath] = useState("");
  const [initialized, setInitialized] = useState(false);
  const [aiReady, setAiReady] = useState(false);
  const aiReadyRef = useRef(false);
  const loadGenRef = useRef(0);
  const pageRef = useRef({ limit: 50, offset: 0 });
  const filterRef = useRef(filter);
  filterRef.current = filter;

  const limit = 50;

  const refreshLogsPath = async () => {
    try {
      setLogsPath(await xiaohan.getTimelineAiLogsPath());
    } catch {
      try {
        const dbPath = await xiaohan.getDataPath();
        setLogsPath(dbPath.replace(/[/\\]xiaohan\.sqlite$/i, "/timeline-ai").replace(/\\/g, "/"));
      } catch {
        setLogsPath("");
      }
    }
  };

  const applyDescribeResult = useCallback(
    (entries: TimelineAiEntry[], gen: number, clearAll: boolean) => {
      if (entries.length > 0) {
        setDescriptions((prev) => mergeDescriptions(prev, entries));
      }
      setPendingDescribe((prev) =>
        prunePending(prev, entries, clearAll && gen === loadGenRef.current),
      );
    },
    [],
  );

  const applyChunk = useCallback((chunk: TimelineDescribeChunkEvent) => {
    if (chunk.offset !== pageRef.current.offset || chunk.limit !== pageRef.current.limit) {
      return;
    }
    applyDescribeResult(chunk.entries, loadGenRef.current, false);
  }, [applyDescribeResult]);

  useEffect(() => {
    refreshLogsPath();
  }, []);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;

    listen<TimelineDescribeChunkEvent>("timeline-describe-chunk", (event) => {
      if (!disposed) {
        applyChunk(event.payload);
      }
    }).then((fn) => {
      if (disposed) {
        fn();
      } else {
        unlisten = fn;
      }
    });

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [applyChunk]);

  const load = useCallback(async (off: number) => {
    const gen = ++loadGenRef.current;
    const sinceMinutes = filterToMinutes(filterRef.current);
    pageRef.current = { limit, offset: off };

    try {
      setError(null);
      setDescribeError(null);

      let textAiReady = aiReadyRef.current;
      try {
        textAiReady = await xiaohan.aiIsTextReady();
        aiReadyRef.current = textAiReady;
        setAiReady(textAiReady);
      } catch {
        textAiReady = false;
        aiReadyRef.current = false;
        setAiReady(false);
      }

      const [page, wt] = await Promise.all([
        xiaohan.getTimeline(limit, off, sinceMinutes),
        xiaohan.workTypesGet(),
      ]);
      if (gen !== loadGenRef.current) return;

      setItems(page.items);
      setTotal(page.total);
      setOffset(off);
      setWorkTypes(wt.types);

      if (!textAiReady) {
        setDescriptions([]);
        setPendingDescribe(new Set());
        return;
      }

      const cached = await xiaohan.timelineCached(limit, off, undefined, sinceMinutes);
      if (gen !== loadGenRef.current) return;

      setDescriptions(cached);
      const cachedKeys = new Set(cached.map((c) => c.started_at));
      const pending = page.items
        .filter((s) => !cachedKeys.has(s.started_at))
        .map((s) => s.started_at);
      setPendingDescribe(new Set(pending));

      if (pending.length === 0) {
        await refreshLogsPath();
        return;
      }

      try {
        const desc = await xiaohan.timelineDescribe(limit, off, undefined, sinceMinutes);
        applyDescribeResult(desc, gen, true);
        if (gen === loadGenRef.current) {
          setDescribeError(null);
          await refreshLogsPath();
        }
      } catch (e) {
        if (gen === loadGenRef.current) {
          setDescribeError(formatApiError(e, "时间线 AI 简介"));
          setPendingDescribe(new Set());
        }
      }
    } catch (e) {
      if (gen === loadGenRef.current) {
        setError(formatApiError(e, "加载时间线"));
      }
    }
  }, [applyDescribeResult, limit]);

  useEffect(() => {
    if (!active) return;
    setInitialized(true);
    load(0);
  }, [active, filter, load]);

  useEffect(() => {
    if (!active || !initialized) return;
    setPendingDescribe((prev) => {
      const next = new Set(prev);
      let changed = false;
      for (const d of descriptions) {
        if (next.delete(d.started_at)) {
          changed = true;
        }
      }
      return changed ? next : prev;
    });
  }, [active, initialized, descriptions]);

  const descByStarted = useMemo(() => {
    const map = new Map<string, TimelineAiEntry>();
    for (const d of descriptions) {
      map.set(d.started_at, d);
    }
    return map;
  }, [descriptions]);

  const enriched = useMemo(() => {
    return items.map((seg) => {
      const ai = aiReady ? descByStarted.get(seg.started_at) : undefined;
      const category = ai?.work_type ?? "其他";
      const text = aiReady && ai?.summary ? ai.summary : fallbackText(seg);
      const isPending =
        aiReady &&
        pendingDescribe.has(seg.started_at) &&
        !descByStarted.has(seg.started_at);
      return {
        seg,
        category,
        text,
        usedAi: aiReady && (ai?.used_ai ?? false),
        isPending,
        color: tagColor(category, workTypes),
      };
    });
  }, [items, descByStarted, workTypes, pendingDescribe, aiReady]);

  const pendingCount = useMemo(
    () => enriched.filter((e) => e.isPending).length,
    [enriched],
  );

  const copyLog = async () => {
    const lines = enriched.map(
      ({ seg, category, text }) =>
        `- ${formatTimeRange(seg)}【${category}】${text}`,
    );
    try {
      await navigator.clipboard.writeText(lines.join("\n"));
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      /* ignore */
    }
  };

  return (
    <div className="page-stack timeline-page">
      <div className="panel timeline-panel">
        <div className="panel-header timeline-panel-header">
          <div className="panel-title panel-title--with-icon">
            <span className="pulse-icon" />
            活动时间线
          </div>
          <div className="timeline-toolbar">
            <button type="button" className="btn-link" onClick={copyLog}>
              {copied ? "已复制" : "复制小记到剪切板"}
            </button>
            <div className="timeline-filters">
              {(
                [
                  ["30m", "近30分"],
                  ["1h", "近1小时"],
                  ["2h", "近2小时"],
                  ["today", "今天"],
                ] as [Filter, string][]
              ).map(([k, label]) => (
                <button
                  key={k}
                  type="button"
                  className={`filter-chip${filter === k ? " active" : ""}`}
                  onClick={() => setFilter(k)}
                >
                  {label}
                </button>
              ))}
            </div>
            {filter !== "today" && (
              <span className="hint-block timeline-filter-hint">
                已按最近 {filter === "30m" ? "30 分钟" : filter === "1h" ? "1 小时" : "2 小时"} 筛选
              </span>
            )}
          </div>
        </div>

        {aiReady && pendingCount > 0 && (
          <p className="timeline-describe-hint">
            小寒正在写简介…（{pendingCount} 条待生成）
          </p>
        )}
        {!aiReady && items.length > 0 && (
          <p className="timeline-describe-hint">
            未绑定 AI，时间线显示应用与窗口等原始活动描述
          </p>
        )}
        {describeError && (
          <div className="error">简介生成：{describeError}</div>
        )}
        {logsPath && (
          <p className="settings-field-hint" style={{ marginBottom: 12 }}>
            AI 原数据与回复 JSON：<code>{logsPath}</code>
          </p>
        )}

        {error && <div className="error">{error}</div>}

        {items.length === 0 && !error ? (
          <EmptyState
            message="今天还没有记录呢"
            hint="切换应用超过 1 分钟会记录；同一应用内换项目/换页面也会分开；后台持续听歌/看视频/聊天超过 30 秒也会记录"
          />
        ) : enriched.length === 0 && !error ? (
          <EmptyState
            message="当前时间范围内无记录"
            hint="试试切换到「今天」查看全天记录"
          />
        ) : (
          <div className="vtimeline">
            {enriched.map(({ seg, category, text, usedAi, isPending, color }, i) => {
              const audio = audioBadge(seg);
              return (
              <div className="vtimeline-item" key={`${seg.started_at}-${i}`}>
                <div className="vtimeline-time">{formatTimeRange(seg)}</div>
                <div className="vtimeline-axis">
                  <span className="vtimeline-dot" />
                  {i < enriched.length - 1 && <span className="vtimeline-line" />}
                </div>
                <div className="vtimeline-card">
                  <div className="vtimeline-card-head">
                    <AppIcon icon={seg.icon} name={seg.app_name} />
                    <span className="vtimeline-app">{seg.app_name}</span>
                    {seg.activity_label && (
                      <span className="vtimeline-activity">{seg.activity_label}</span>
                    )}
                  </div>
                  <p className={`vtimeline-text${isPending ? " vtimeline-text--loading" : ""}`}>
                    {isPending ? "思考中…" : text}
                  </p>
                  <div className="vtimeline-card-foot">
                    {audio && (
                      <span className="audio-badge">{audio}</span>
                    )}
                    <span className="wt-tag" style={{ background: `${color}22`, color, borderColor: `${color}55` }}>
                      {category}
                    </span>
                    <span className="vtimeline-meta">
                      {isPending
                        ? "生成中"
                        : aiReady
                          ? usedAi
                            ? "AI 简介"
                            : "本地简介"
                          : "活动记录"}{" "}
                      · {formatTimeRange(seg)} · {formatDuration(seg.duration_ms)}
                    </span>
                  </div>
                </div>
              </div>
            );
            })}
          </div>
        )}

        {total > limit && (
          <div className="pager">
            <button disabled={offset === 0} onClick={() => load(Math.max(0, offset - limit))}>
              上一页
            </button>
            <span>
              {offset + 1}–{Math.min(offset + limit, total)} / {total}
            </span>
            <button
              disabled={offset + limit >= total}
              onClick={() => load(offset + limit)}
            >
              下一页
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
