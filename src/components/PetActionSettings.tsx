import { useCallback, useEffect, useMemo, useRef, useState, type Dispatch, type SetStateAction } from "react";
import { listen } from "@tauri-apps/api/event";
import { parseApiError, successFeedback, type SettingsFeedback } from "../lib/apiErrorMessage";
import { isLikelyIdleName } from "../lib/petAnimationNames";
import { xiaohan, type PetLinesImportProgressEvent, type PetRemarkLine } from "../lib/xiaohan";

export interface PetActionLayout {
  idleAnimation: string;
  clickAnimation: string;
 bootAnimation: string;
 returnIdleAnimation: string;
 dragAnimation: string;
 randomAnimations: string[];
  randomMinSec: number;
  randomMaxSec: number;
  lines: PetRemarkLine[];
}

interface PetActionSettingsProps {
  modelId: string;
  animations: string[];
  animationsLoading?: boolean;
  layout: PetActionLayout;
  busy: boolean;
  section: "actions" | "lines" | "lines-import";
  applyLive?: boolean;
  onLayoutChange: Dispatch<SetStateAction<PetActionLayout>>;
  onSaved: () => Promise<void>;
  setFeedback: (f: SettingsFeedback | null) => void;
  setBusy: (v: boolean) => void;
  onPatchReady?: (patch: (partial: Partial<PetActionLayout>) => void) => void;
  onFocusImportTab?: () => void;
}

const LINE_IMPORT_STEPS = [
  { id: "preprocess", label: "预处理文本" },
  { id: "extract", label: "清洗提取台词" },
  { id: "validate", label: "校验完整性" },
  { id: "save", label: "写入台词库" },
] as const;

const WIKI_IMPORT_STEPS = [
  { id: "fetch", label: "爬取网页" },
  { id: "parse", label: "解析页面" },
  { id: "extract", label: "清洗提取" },
  { id: "validate", label: "校验完整性" },
  { id: "save", label: "写入台词库" },
] as const;

function layoutFromMeta(meta: {
  idle_animation?: string | null;
  click_animation?: string | null;
  boot_animation?: string | null;
  return_idle_animation?: string | null;
  drag_animation?: string | null;
  random_animations?: string[];
  random_min_sec?: number;
  random_max_sec?: number;
  lines?: PetRemarkLine[];
}): PetActionLayout {
  return {
    idleAnimation: meta.idle_animation ?? "",
    clickAnimation: meta.click_animation ?? "",
    bootAnimation: meta.boot_animation ?? meta.idle_animation ?? "",
    returnIdleAnimation: meta.return_idle_animation ?? meta.idle_animation ?? "",
    dragAnimation: meta.drag_animation ?? "",
    randomAnimations: meta.random_animations ?? [],
    randomMinSec: meta.random_min_sec ?? 30,
    randomMaxSec: meta.random_max_sec ?? 120,
    lines: meta.lines ?? [],
  };
}

export function PetActionSettings({
  modelId,
  animations,
  animationsLoading = false,
  layout,
  busy,
  section,
  applyLive = true,
  onLayoutChange,
  onSaved,
  setFeedback,
  setBusy,
  onPatchReady,
  onFocusImportTab,
}: PetActionSettingsProps) {
  const [importText, setImportText] = useState("");
  const [wikiUrl, setWikiUrl] = useState("");
  const [lineImporting, setLineImporting] = useState(false);
  const [importMode, setImportMode] = useState<"paste" | "wiki">("paste");
  const [importProgressStep, setImportProgressStep] = useState(0);
  const [importProgressMessage, setImportProgressMessage] = useState("");
  const [newLineText, setNewLineText] = useState("");
  const [newLineAnim, setNewLineAnim] = useState("");
  const autoSaveTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const saveSerial = useRef(0);
  const modelIdRef = useRef(modelId);
  const importPanelRef = useRef<HTMLDivElement>(null);
  const importFeedbackTitleRef = useRef("台词导入中");

  useEffect(() => {
    modelIdRef.current = modelId;
    if (autoSaveTimer.current) {
      clearTimeout(autoSaveTimer.current);
      autoSaveTimer.current = null;
    }
  }, [modelId]);

  useEffect(() => {
    return () => {
      if (autoSaveTimer.current) clearTimeout(autoSaveTimer.current);
    };
  }, []);

  const ensureImportPanelVisible = () => {
    onFocusImportTab?.();
  };

  const waitForProgressPaint = () =>
    new Promise<void>((resolve) => {
      requestAnimationFrame(() => {
        requestAnimationFrame(() => resolve());
      });
    });

  const beginLineImport = (mode: "paste" | "wiki", step: number, message: string, title: string) => {
    ensureImportPanelVisible();
    importFeedbackTitleRef.current = title;
    setImportMode(mode);
    setLineImporting(true);
    setBusy(true);
    setImportProgressStep(step);
    setImportProgressMessage(message);
    setFeedback({ tone: "loading", title, detail: message });
  };

  const attachImportProgressListener = () =>
    listen<PetLinesImportProgressEvent>("pet-lines-import-progress", (event) => {
      const { step_index, message } = event.payload;
      setImportProgressStep(step_index);
      setImportProgressMessage(message);
      setFeedback({
        tone: "loading",
        title: importFeedbackTitleRef.current,
        detail: message,
      });
    });

  const persistLayout = useCallback(
    async (layoutToSave: PetActionLayout, mid: string) => {
      const serial = ++saveSerial.current;
      try {
        const meta = await xiaohan.petSaveAnimationLayout({
          model_id: mid,
          idle_animation: layoutToSave.idleAnimation || null,
          click_animation: layoutToSave.clickAnimation || null,
          boot_animation: layoutToSave.bootAnimation || null,
          return_idle_animation: layoutToSave.returnIdleAnimation || null,
          drag_animation: layoutToSave.dragAnimation || null,
          random_animations: layoutToSave.randomAnimations,
          random_min_sec: layoutToSave.randomMinSec,
          random_max_sec: layoutToSave.randomMaxSec,
          lines: layoutToSave.lines,
        });
        if (serial !== saveSerial.current || mid !== modelIdRef.current) return;
        if (applyLive) {
          await xiaohan.petRefreshAnimations();
        }
        onLayoutChange(layoutFromMeta(meta));
        await onSaved();
        setFeedback(successFeedback("动作与台词已自动保存"));
      } catch (e) {
        if (serial === saveSerial.current) {
          setFeedback(parseApiError(e, "自动保存动作配置"));
        }
      }
    },
    [applyLive, onLayoutChange, onSaved, setFeedback],
  );

  const scheduleAutoSave = useCallback(
    (nextLayout: PetActionLayout) => {
      if (autoSaveTimer.current) clearTimeout(autoSaveTimer.current);
      autoSaveTimer.current = setTimeout(() => {
        autoSaveTimer.current = null;
        void persistLayout(nextLayout, modelIdRef.current);
      }, 500);
    },
    [persistLayout],
  );

  const mergeLayout = useCallback(
    (builder: (prev: PetActionLayout) => PetActionLayout) => {
      onLayoutChange((prev) => {
        const next = builder(prev);
        scheduleAutoSave(next);
        return next;
      });
    },
    [onLayoutChange, scheduleAutoSave],
  );

  const update = useCallback(
    (patch: Partial<PetActionLayout>) => {
      onLayoutChange((prev) => {
        const next = { ...prev, ...patch };
        scheduleAutoSave(next);
        return next;
      });
    },
    [onLayoutChange, scheduleAutoSave],
  );

  useEffect(() => {
    onPatchReady?.(update);
  }, [onPatchReady, update]);

  const setSingle = useCallback(
    (field: "idleAnimation" | "clickAnimation" | "bootAnimation" | "returnIdleAnimation" | "dragAnimation", name: string) => {
      onLayoutChange((prev) => {
        const next: PetActionLayout = { ...prev, [field]: name };
        if (field === "idleAnimation") {
          next.randomAnimations = prev.randomAnimations.filter((n) => n !== name);
          next.returnIdleAnimation = name;
        }
        scheduleAutoSave(next);
        return next;
      });
    },
    [onLayoutChange, scheduleAutoSave],
  );

  const toggleRandom = useCallback(
    (name: string, checked: boolean) => {
      onLayoutChange((prev) => {
        const nextRandom = checked
          ? prev.randomAnimations.includes(name)
            ? prev.randomAnimations
            : [...prev.randomAnimations, name]
          : prev.randomAnimations.filter((n) => n !== name);
        const next = { ...prev, randomAnimations: nextRandom };
        scheduleAutoSave(next);
        return next;
      });
    },
    [onLayoutChange, scheduleAutoSave],
  );

  const submitNewLine = useCallback(() => {
    const text = newLineText.trim();
    if (!text || busy) return;
    mergeLayout((prev) => ({
      ...prev,
      lines: [...prev.lines, { text, animation: newLineAnim || null }],
    }));
    setNewLineText("");
    setNewLineAnim("");
  }, [busy, mergeLayout, newLineAnim, newLineText]);

 const previewAnimation = useCallback(
   async (name: string) => {
     if (!applyLive) {
       setFeedback({
         tone: "info",
         title: "预览不可用",
         detail: "当前桌宠未加载此模型，无法在桌宠上预览动作。",
       });
       return;
     }
     try {
        // 不调 petShow：show_pet 末尾会发 pet-reload 触发 SpinePet 整体重建，
        // 与紧随其后的预览事件竞态，previewPlay 会作用在即将销毁的旧实例上导致渲染碎块。
        // petPreviewAnimation 内部已 ensure+show 窗口，路径与普通点击一致。
       const loopPreview =
         isLikelyIdleName(name) &&
         (name === layout.idleAnimation || name === layout.returnIdleAnimation);
       await xiaohan.petPreviewAnimation(name, loopPreview);
     } catch (e) {
       setFeedback(parseApiError(e, "预览动作"));
     }
   },
   [applyLive, layout.idleAnimation, layout.returnIdleAnimation, setFeedback],
 );

  const parseImportLines = (raw: string): PetRemarkLine[] => {
    const trimmed = raw.trim();
    if (!trimmed) return [];
    try {
      const parsed = JSON.parse(trimmed) as unknown;
      if (Array.isArray(parsed)) {
        return parsed
          .map((item) => {
            if (typeof item === "string") return { text: item.trim(), animation: null };
            if (item && typeof item === "object") {
              const o = item as Record<string, unknown>;
              const text = String(o.text ?? "").trim();
              if (!text) return null;
              const animation = o.animation ? String(o.animation).trim() : null;
              return { text, animation: animation || null };
            }
            return null;
          })
          .filter(Boolean) as PetRemarkLine[];
      }
    } catch {
      /* plain text */
    }
    return trimmed
      .split(/\r?\n/)
      .map((line) => line.trim())
      .filter(Boolean)
      .map((text) => ({ text, animation: null }));
  };

  const runAiImport = async (mode: "append" | "replace") => {
    const raw = importText.trim();
    if (!raw) {
      setFeedback({ tone: "error", title: "导入失败", detail: "请先粘贴要导入的文本" });
      return;
    }
    beginLineImport("paste", 1, "正在预处理文本…", "AI 台词导入中");
    const unlisten = await attachImportProgressListener();
    await waitForProgressPaint();
    try {
      const extracted = await xiaohan.petAiImportLines(modelId, raw);
      setImportProgressStep(4);
      setImportProgressMessage("正在写入台词库…");
      setFeedback({
        tone: "loading",
        title: importFeedbackTitleRef.current,
        detail: "正在写入台词库…",
      });
      if (mode === "append") {
        mergeLayout((prev) => ({
          ...prev,
          lines: [...prev.lines, ...extracted],
        }));
      } else {
        update({ lines: extracted });
      }
      setImportText("");
      setFeedback(successFeedback(`AI 已导入 ${extracted.length} 条台词（自动保存中）`));
    } catch (e) {
      setFeedback(parseApiError(e, "AI 导入台词"));
    } finally {
      unlisten();
      setLineImporting(false);
      setBusy(false);
    }
  };

  const runWikiImport = async (mode: "append" | "replace") => {
    const url = wikiUrl.trim();
    if (!url) {
      setFeedback({ tone: "error", title: "导入失败", detail: "请输入 Wiki 链接" });
      return;
    }
    beginLineImport("wiki", 1, "正在爬取网页…", "Wiki 台词导入中");
    const unlisten = await attachImportProgressListener();
    await waitForProgressPaint();
    try {
      const extracted = await xiaohan.petWikiImportLines(modelId, url);
      setImportProgressStep(5);
      setImportProgressMessage("正在写入台词库…");
      setFeedback({
        tone: "loading",
        title: importFeedbackTitleRef.current,
        detail: "正在写入台词库…",
      });
      if (mode === "append") {
        mergeLayout((prev) => ({
          ...prev,
          lines: [...prev.lines, ...extracted],
        }));
      } else {
        update({ lines: extracted });
      }
      setFeedback(successFeedback(`Wiki 已导入 ${extracted.length} 条台词（自动保存中）`));
    } catch (e) {
      setFeedback(parseApiError(e, "Wiki 导入台词"));
    } finally {
      unlisten();
      setLineImporting(false);
      setBusy(false);
    }
  };

  const importSteps = importMode === "wiki" ? WIKI_IMPORT_STEPS : LINE_IMPORT_STEPS;

  const importProgressView = lineImporting ? (
    <div className="persona-import-progress pet-lines-import-progress" role="status" aria-live="polite">
      <p className="persona-import-progress-title">
        {importMode === "wiki" ? "Wiki 导入处理中" : "AI 导入处理中"}
      </p>
      <ol className="persona-import-progress-steps">
        {importSteps.map((step, index) => {
          const stepNum = index + 1;
          const activeStep = Math.max(importProgressStep, 1);
          const state =
            activeStep > stepNum ? "done" : activeStep === stepNum ? "active" : "pending";
          return (
            <li
              key={step.id}
              className={`persona-import-progress-step persona-import-progress-step--${state}`}
            >
              <span className="persona-import-progress-step-dot" aria-hidden />
              <span className="persona-import-progress-step-label">{step.label}</span>
            </li>
          );
        })}
      </ol>
      {importProgressMessage && (
        <p className="persona-import-progress-detail">{importProgressMessage}</p>
      )}
    </div>
  ) : null;

  const lineRows = useMemo(
    () =>
      layout.lines.map((line, idx) => (
        <li
          key={`${line.text}-${idx}`}
          className="pet-lines-note"
          style={{ ["--note-rot" as string]: `${((idx % 5) - 2) * 0.6}deg` }}
        >
          <button
            type="button"
            className="pet-lines-note-remove"
            disabled={busy}
            aria-label="删除台词"
            onClick={() =>
              mergeLayout((prev) => ({
                ...prev,
                lines: prev.lines.filter((_, i) => i !== idx),
              }))
            }
          >
            ×
          </button>
          <span className="pet-lines-note-no">{idx + 1}</span>
          <p className="pet-lines-note-text">{line.text}</p>
          <span
            className={`pet-lines-note-bind${line.animation ? "" : " is-general"}`}
            title={line.animation ? `绑定：${line.animation}` : "通用台词"}
          >
            {line.animation ? `↳ ${line.animation}` : "通用"}
          </span>
        </li>
      )),
    [busy, layout.lines, mergeLayout],
  );

  if (section === "actions" && animations.length === 0) {
    if (animationsLoading) {
      return <p className="hint-block">正在加载动作与台词…</p>;
    }
    return <p className="hint-block">暂无动作数据，请确认桌宠已启用且模型文件完整。</p>;
  }

  if (section === "actions") {
    return (
      <div className="pet-tab-section">
        <div className="pet-action-table-wrap">
          <table className="pet-action-table">
            <thead>
              <tr>
                <th>动作</th>
                <th>待机</th>
                <th>点击</th>
                <th>开机</th>
                <th>回待机</th>
                <th>拖拽</th>
                <th>随机</th>
              </tr>
            </thead>
            <tbody>
              {animations.map((name) => {
                const baseAnim = isLikelyIdleName(name);
                return (
                  <tr key={name} className={baseAnim ? "pet-action-row--base" : undefined}>
                    <td>
                      <button
                        type="button"
                        className={`pet-action-preview${baseAnim ? " pet-action-preview--base" : ""}`}
                        disabled={busy}
                        title={baseAnim ? "待机类动作（循环预览）" : "叠加动作（一次性预览）"}
                        onClick={() => void previewAnimation(name)}
                      >
                        {name}
                      </button>
                    </td>
                    <td>
                      <input
                        type="radio"
                        name="pet-idle"
                        checked={layout.idleAnimation === name}
                        onChange={() => setSingle("idleAnimation", name)}
                      />
                    </td>
                    <td>
                      <input
                        type="radio"
                        name="pet-click"
                        checked={layout.clickAnimation === name}
                        onChange={() => setSingle("clickAnimation", name)}
                      />
                    </td>
                    <td>
                      <input
                        type="radio"
                        name="pet-boot"
                        checked={layout.bootAnimation === name}
                        onChange={() => setSingle("bootAnimation", name)}
                      />
                    </td>
                    <td>
                      <input
                        type="radio"
                        name="pet-return-idle"
                        checked={layout.returnIdleAnimation === name}
                        onChange={() => setSingle("returnIdleAnimation", name)}
                      />
                    </td>
                    <td>
                      <input
                        type="radio"
                        name="pet-drag"
                        checked={layout.dragAnimation === name}
                        onChange={() => setSingle("dragAnimation", name)}
                      />
                    </td>
                    <td>
                      <input
                        type="checkbox"
                        checked={layout.randomAnimations.includes(name)}
                        disabled={busy || name === layout.idleAnimation}
                        onChange={(e) => toggleRandom(name, e.target.checked)}
                      />
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      </div>
    );
  }

  if (section === "lines") {
    return (
      <div className="pet-tab-section pet-tab-section--board">
        <div className="pet-lines-board">
          {lineRows.length > 0 ? (
            <ul className="pet-lines-grid">{lineRows}</ul>
          ) : (
            <div className="pet-lines-empty">
              <p className="pet-lines-empty-msg">还没有台词便签</p>
              <p className="pet-lines-empty-hint">切换到「台词导入」写一句或批量导入</p>
            </div>
          )}
        </div>
      </div>
    );
  }

  return (
    <div ref={importPanelRef} className="pet-tab-section pet-lines-import-body">
          {importProgressView}

          <section className="pet-lines-import-block">
            <div className="pet-lines-import-block-head">
              <span className="pet-lines-import-block-badge pet-lines-import-block-badge--add">
                添加
              </span>
              <div>
                <h4 className="pet-lines-import-block-title">写一句新台词</h4>
                <p className="pet-lines-import-block-desc">快速贴一条到便签墙，可绑定动作</p>
              </div>
            </div>
            <div className="pet-lines-composer-row">
              <input
                id="pet-new-line"
                className="pet-lines-composer-input"
                placeholder="输入台词，回车快速贴上去…"
                value={newLineText}
                disabled={busy}
                onChange={(e) => setNewLineText(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter" && newLineText.trim() && !busy) {
                    e.preventDefault();
                    submitNewLine();
                  }
                }}
              />
              <select
                className="pet-lines-composer-select"
                value={newLineAnim}
                disabled={busy}
                onChange={(e) => setNewLineAnim(e.target.value)}
                title="绑定动作"
                aria-label="绑定动作"
              >
                <option value="">通用</option>
                {animations.map((n) => (
                  <option key={n} value={n}>
                    {n}
                  </option>
                ))}
              </select>
              <button
                type="button"
                className="btn-primary btn-sm pet-lines-composer-btn"
                disabled={busy || !newLineText.trim()}
                onClick={submitNewLine}
              >
                贴上去
              </button>
            </div>
          </section>

          <div className="pet-lines-import-divider" aria-hidden>
            <span>或</span>
          </div>

          <section className="pet-lines-import-block">
            <div className="pet-lines-import-block-head">
              <span className="pet-lines-import-block-badge">Wiki</span>
              <div>
                <h4 className="pet-lines-import-block-title">从 Wiki 爬取</h4>
                <p className="pet-lines-import-block-desc">
                  粘贴舰娘 Wiki 页面链接，自动提取「舰船台词」并导入
                </p>
              </div>
            </div>
            <div className="pet-lines-wiki-row">
              <input
                id="pet-wiki-url"
                type="url"
                className="pet-lines-wiki-input"
                placeholder="https://wiki.biligame.com/blhx/柴郡"
                value={wikiUrl}
                disabled={busy || lineImporting}
                onChange={(e) => setWikiUrl(e.target.value)}
                aria-label="Wiki 链接"
              />
              <div className="pet-lines-wiki-actions">
                <button
                  type="button"
                  className="btn-primary btn-sm"
                  disabled={busy || lineImporting || !wikiUrl.trim()}
                  onClick={() => void runWikiImport("append")}
                >
                  {lineImporting && importMode === "wiki" ? "导入中…" : "开始导入"}
                </button>
                <button
                  type="button"
                  className="btn-secondary btn-sm"
                  disabled={busy || lineImporting || !wikiUrl.trim()}
                  onClick={() => void runWikiImport("replace")}
                >
                  覆盖
                </button>
              </div>
            </div>
          </section>

          <div className="pet-lines-import-divider" aria-hidden>
            <span>或</span>
          </div>

          <section className="pet-lines-import-block">
            <div className="pet-lines-import-block-head">
              <span className="pet-lines-import-block-badge pet-lines-import-block-badge--text">
                文本
              </span>
              <div>
                <h4 className="pet-lines-import-block-title">粘贴文本导入</h4>
                <p className="pet-lines-import-block-desc">
                  支持混杂文本、逐行、JSON；AI 会清洗提取并保留原文措辞
                </p>
              </div>
            </div>
            <div className="pet-lines-import-row">
              <textarea
                className="pet-lines-import"
                placeholder={
                  "每行一条台词，或 JSON：[{\"text\":\"…\",\"animation\":\"dance\"}]\n" +
                  "也可粘贴含章节、说明的混杂文本，由 AI 逐条提取"
                }
                value={importText}
                disabled={busy || lineImporting}
                onChange={(e) => setImportText(e.target.value)}
                rows={4}
              />
              <aside className="pet-lines-import-side">
                <div className="pet-lines-actions-group">
                  <span className="pet-lines-actions-label">规则解析</span>
                  <div className="pet-lines-actions">
                    <button
                      type="button"
                      className="btn-secondary btn-sm"
                      disabled={busy || lineImporting || !importText.trim()}
                      onClick={() => {
                        const incoming = parseImportLines(importText);
                        if (incoming.length === 0) {
                          setFeedback({
                            tone: "error",
                            title: "导入失败",
                            detail: "未解析到有效台词",
                          });
                          return;
                        }
                        mergeLayout((prev) => ({
                          ...prev,
                          lines: [...prev.lines, ...incoming],
                        }));
                        setImportText("");
                        setFeedback(
                          successFeedback(`已导入 ${incoming.length} 条台词（自动保存中）`),
                        );
                      }}
                    >
                      追加
                    </button>
                    <button
                      type="button"
                      className="btn-secondary btn-sm"
                      disabled={busy || lineImporting || !importText.trim()}
                      onClick={() => {
                        const incoming = parseImportLines(importText);
                        if (incoming.length === 0) {
                          setFeedback({
                            tone: "error",
                            title: "导入失败",
                            detail: "未解析到有效台词",
                          });
                          return;
                        }
                        update({ lines: incoming });
                        setImportText("");
                        setFeedback(
                          successFeedback(`已替换为 ${incoming.length} 条台词（自动保存中）`),
                        );
                      }}
                    >
                      覆盖
                    </button>
                  </div>
                </div>
                <div className="pet-lines-actions-group">
                  <span className="pet-lines-actions-label">AI 处理</span>
                  <div className="pet-lines-actions">
                    <button
                      type="button"
                      className="btn-secondary btn-sm"
                      disabled={busy || lineImporting || !importText.trim()}
                      onClick={() => void runAiImport("append")}
                    >
                      {lineImporting && importMode === "paste" ? "导入中…" : "智能追加"}
                    </button>
                    <button
                      type="button"
                      className="btn-secondary btn-sm"
                      disabled={busy || lineImporting || !importText.trim()}
                      onClick={() => void runAiImport("replace")}
                    >
                      智能覆盖
                    </button>
                    <button
                      type="button"
                      className="btn-secondary btn-sm"
                      disabled={busy || lineImporting}
                      onClick={async () => {
                        setBusy(true);
                        setFeedback(null);
                        try {
                          const suggested = await xiaohan.petAiSuggestLines(modelId, 8);
                          mergeLayout((prev) => ({
                            ...prev,
                            lines: [...prev.lines, ...suggested],
                          }));
                          setFeedback(
                            successFeedback(`AI 已生成 ${suggested.length} 条台词（自动保存中）`),
                          );
                        } catch (e) {
                          setFeedback(parseApiError(e, "AI 生成台词"));
                        } finally {
                          setBusy(false);
                        }
                      }}
                    >
                      一键生成
                    </button>
                  </div>
                </div>
              </aside>
            </div>
          </section>
    </div>
  );
}
