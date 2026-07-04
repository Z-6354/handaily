import { useMemo, useState } from "react";
import { SettingsFeedbackBanner } from "../components/SettingsFeedbackBanner";
import {
  loadingFeedback,
  parseApiError,
  type SettingsFeedback,
} from "../lib/apiErrorMessage";
import { xiaohan, type ReportGenerateResult } from "../lib/xiaohan";

const TEMPLATES = [
  {
    id: "period-summary",
    name: "时段总结",
    desc: "把各时段小结串起来，像翻日记本",
    emoji: "✨",
    snippets: [
      { time: "14:00", tag: "编程", text: "新功能写到一半，渐入佳境" },
      { time: "15:30", tag: "休息", text: "刷会儿视频放松一下" },
    ],
  },
  {
    id: "activity-log",
    name: "完成记录",
    desc: "今天在哪干啥、待了多久 — 生活手账风",
    emoji: "📝",
    snippets: [
      { time: "14:00", tag: "VS Code", text: "写代码大约 1.5 小时" },
      { time: "16:00", tag: "浏览器", text: "看教程大约 40 分钟" },
    ],
  },
] as const;

type TemplateId = (typeof TEMPLATES)[number]["id"];

function todayIso() {
  return new Date().toISOString().slice(0, 10);
}

function formatRangeLabel(from: string, to: string) {
  if (from === to) return from.replaceAll("-", "/");
  return `${from.replaceAll("-", "/")} – ${to.replaceAll("-", "/")}`;
}

export function ReportGeneratePage() {
  const [templateId, setTemplateId] = useState<TemplateId>("period-summary");
  const [dateFrom, setDateFrom] = useState(todayIso);
  const [dateTo, setDateTo] = useState(todayIso);
  const [generating, setGenerating] = useState(false);
  const [feedback, setFeedback] = useState<SettingsFeedback | null>(null);
  const [result, setResult] = useState<ReportGenerateResult | null>(null);

  const selected = TEMPLATES.find((t) => t.id === templateId)!;
  const rangeLabel = useMemo(() => formatRangeLabel(dateFrom, dateTo), [dateFrom, dateTo]);

  const generate = async () => {
    setGenerating(true);
    setFeedback(loadingFeedback("正在整理记录，稍等一下下…"));
    try {
      const res = await xiaohan.reportGenerate(templateId, dateFrom, dateTo);
      setResult(res);
      setFeedback({
        tone: "success",
        title: res.used_ai ? "好啦，AI 帮你写好啦～" : "拼好啦（本地数据版）",
        detail: res.used_ai
          ? "已按当前人设润色，去「历史报告」还能再翻"
          : "配置文本模型后会更像手账；现在先用本地记录拼的",
      });
    } catch (e) {
      setFeedback(parseApiError(e, "生成报告"));
    } finally {
      setGenerating(false);
    }
  };

  const previewContent = result?.content;
  const previewTitle = result?.title ?? `${selected.emoji} ${selected.name}`;

  return (
    <div className="report-generate">
      <div className="report-generate-main">
        <div className="panel report-config-panel">
          <div className="panel-header">
            <div>
              <div className="panel-title">整理一下今天</div>
              <p className="panel-desc">
                选个模板、圈个日期，小寒帮你把屏幕前的碎片收成一篇小记
              </p>
            </div>
            <div className="report-config-actions">
              <button
                type="button"
                className="btn-primary"
                disabled={generating}
                onClick={generate}
              >
                {generating ? "生成中…" : "生成小记"}
              </button>
            </div>
          </div>

          <div className="report-config-fields">
            <div className="report-field">
              <label>想翻哪几天？</label>
              <div className="report-date-row">
                <input
                  type="date"
                  className="date-input"
                  value={dateFrom}
                  max={dateTo}
                  onChange={(e) => setDateFrom(e.target.value)}
                />
                <span className="date-sep">到</span>
                <input
                  type="date"
                  className="date-input"
                  value={dateTo}
                  min={dateFrom}
                  onChange={(e) => setDateTo(e.target.value)}
                />
              </div>
              <p className="settings-field-hint">默认是今天；选多天可以写「这几天都干了啥」</p>
            </div>
          </div>
          <SettingsFeedbackBanner feedback={feedback} compact />
        </div>

        <div className="panel">
          <div className="panel-title">选个模板</div>
          <p className="panel-desc" style={{ marginBottom: 12 }}>
            两种风格，都是给自己看的生活记录，不是给老板交差的那种
          </p>
          <div className="template-grid template-grid--compact">
            {TEMPLATES.map((t) => (
              <button
                key={t.id}
                type="button"
                className={`template-card${templateId === t.id ? " selected" : ""}`}
                onClick={() => {
                  setTemplateId(t.id);
                  setResult(null);
                  setFeedback(null);
                }}
              >
                <div className="template-card-head">
                  <span className="template-name">
                    {t.emoji} {t.name}
                  </span>
                  {templateId === t.id && <span className="template-check">✓</span>}
                </div>
                <p className="template-desc">{t.desc}</p>
              </button>
            ))}
          </div>
        </div>
      </div>

      <div className="panel report-preview-panel">
        <div className="panel-title">预览</div>
        <div className="report-preview">
          <h3>{previewTitle}</h3>
          <p className="report-preview-date">{rangeLabel}</p>
          <div className="report-preview-body report-preview-markdown">
            {previewContent ? (
              <pre className="report-markdown-pre">{previewContent}</pre>
            ) : (
              <>
                {selected.snippets.map((snippet) => (
                  <div key={snippet.time} className="report-preview-snippet">
                    <span className="report-preview-snippet-time">{snippet.time}</span>
                    <span className="report-preview-snippet-tag">{snippet.tag}</span>
                    <p className="report-preview-snippet-text">{snippet.text}</p>
                  </div>
                ))}
                <p className="report-preview-note">点「生成小记」后这里会显示正文～</p>
              </>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
