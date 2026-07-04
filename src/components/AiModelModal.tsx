import { useEffect, useMemo, useState } from "react";
import { SettingsFeedbackBanner } from "./SettingsFeedbackBanner";
import type { SettingsFeedback } from "../lib/apiErrorMessage";
import type { AiModelEntry } from "../lib/xiaohan";

export type AiModelKind = "text" | "vision" | "thinking";

const KIND_LABELS: Record<AiModelKind, string> = {
  text: "文本模型",
  vision: "多模态模型",
  thinking: "思考模型",
};

const KIND_HINTS: Record<AiModelKind, string> = {
  text: "用于时段总结、报告润色等日常对话任务",
  vision: "用于截图分析等多模态任务",
  thinking: "用于人设 Wiki 导入、结构化 JSON 解析等",
};

const VENDOR_HINTS: Record<string, string> = {
  ollama: "填写 ollama list 中的模型名；需先在本地 pull 对应模型。",
  volcano: "Agent Plan 常不返回模型列表，请填写控制台中的模型 ID。",
  opencode: "OpenCode GO 需填写套餐内模型 ID；周额度用尽时可换供应商。",
  deepseek: "如 deepseek-chat、deepseek-reasoner。",
  glm: "如 glm-4-flash、glm-4.7。",
  agens: "填写 Agnes 控制台提供的模型 ID。",
};

const VENDOR_EXAMPLES: Record<string, Partial<Record<AiModelKind, string[]>>> = {
  ollama: {
    text: ["llama3.2", "qwen2.5", "gemma3"],
    vision: ["llava", "llama3.2-vision"],
    thinking: ["deepseek-r1", "qwen2.5"],
  },
  volcano: {
    text: ["ark-code-latest"],
    vision: ["doubao-seedream-5.0-lite"],
    thinking: ["ark-code-latest"],
  },
  opencode: {
    text: ["deepseek-v4-pro", "kimi-k2.7-code"],
    thinking: ["deepseek-v4-pro", "kimi-k2.7-code"],
  },
  deepseek: {
    text: ["deepseek-chat"],
    thinking: ["deepseek-reasoner"],
  },
  glm: {
    text: ["glm-4-flash"],
    thinking: ["glm-4.7"],
  },
  agens: {
    text: ["gpt-4o-mini"],
  },
};

interface Props {
  open: boolean;
  kind: AiModelKind | null;
  vendorId: string;
  vendorName: string;
  existingCustom?: AiModelEntry[];
  feedback?: SettingsFeedback | null;
  onClose: () => void;
  onSubmit: (id: string, name: string) => void | Promise<void>;
}

function getExamples(vendorId: string, kind: AiModelKind): string[] {
  const vendor = VENDOR_EXAMPLES[vendorId];
  if (!vendor) return [];
  return vendor[kind] ?? vendor.text ?? [];
}

export function AiModelModal({
  open,
  kind,
  vendorId,
  vendorName,
  existingCustom = [],
  feedback,
  onClose,
  onSubmit,
}: Props) {
  const [id, setId] = useState("");
  const [name, setName] = useState("");
  const [submitting, setSubmitting] = useState(false);

  useEffect(() => {
    if (open) {
      setId("");
      setName("");
      setSubmitting(false);
    }
  }, [open, kind, vendorId]);

  const examples = useMemo(
    () => (kind ? getExamples(vendorId, kind) : []),
    [vendorId, kind],
  );

  const vendorHint = VENDOR_HINTS[vendorId];
  const kindHint = kind ? KIND_HINTS[kind] : "";

  const relatedCustom = useMemo(() => {
    if (!kind) return [];
    return existingCustom.filter((m) => m.vendor_id === vendorId && m.kind === kind);
  }, [existingCustom, vendorId, kind]);

  if (!open || !kind) return null;

  const submit = async () => {
    const trimmedId = id.trim();
    if (!trimmedId || submitting) return;
    setSubmitting(true);
    try {
      await onSubmit(trimmedId, name.trim() || trimmedId);
      onClose();
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div className="modal-dialog ai-model-modal" onClick={(e) => e.stopPropagation()}>
        <div className="modal-header">
          <div className="ai-model-modal-head">
            <h3 className="modal-title">手动添加模型</h3>
            <p className="ai-model-modal-subtitle">{KIND_LABELS[kind]}</p>
          </div>
          <button type="button" className="modal-close" onClick={onClose} aria-label="关闭">
            ×
          </button>
        </div>

        <div className="ai-model-modal-body">
          <div className="ai-model-modal-context">
            <span className="ai-model-modal-chip ai-model-modal-chip--kind">{KIND_LABELS[kind]}</span>
            <span className="ai-model-modal-chip">{vendorName}</span>
          </div>

          <p className="ai-model-modal-desc">{kindHint}</p>
          {vendorHint && <p className="ai-model-modal-vendor-hint">{vendorHint}</p>}

          {relatedCustom.length > 0 && (
            <div className="ai-model-modal-existing">
              <span className="ai-model-modal-existing-label">已添加的自定义模型</span>
              <div className="ai-model-modal-existing-list">
                {relatedCustom.map((m) => (
                  <button
                    key={m.id}
                    type="button"
                    className="ai-model-modal-example"
                    disabled={submitting}
                    onClick={() => {
                      setId(m.id);
                      if (!name.trim()) setName(m.name);
                    }}
                    title="点击填入表单"
                  >
                    {m.name !== m.id ? `${m.name} · ${m.id}` : m.id}
                  </button>
                ))}
              </div>
            </div>
          )}

          {examples.length > 0 && (
            <div className="ai-model-modal-examples">
              <span className="ai-model-modal-examples-label">常用 ID（点击填入）</span>
              <div className="ai-model-modal-examples-list">
                {examples.map((example) => (
                  <button
                    key={example}
                    type="button"
                    className="ai-model-modal-example"
                    disabled={submitting}
                    onClick={() => setId(example)}
                  >
                    {example}
                  </button>
                ))}
              </div>
            </div>
          )}

          <div className="ai-model-modal-form">
            <label className="vault-field">
              <span className="vault-field-label">模型 ID</span>
              <input
                className="vault-field-input"
                value={id}
                onChange={(e) => setId(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") void submit();
                }}
                placeholder="供应商文档中的模型 ID"
                autoFocus
                disabled={submitting}
              />
              <span className="vault-field-hint">必填，需与供应商 API 接受的 ID 完全一致</span>
            </label>

            <label className="vault-field">
              <span className="vault-field-label">显示名称</span>
              <input
                className="vault-field-input"
                value={name}
                onChange={(e) => setName(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") void submit();
                }}
                placeholder="可选，下拉列表中显示；留空则同 ID"
                disabled={submitting}
              />
            </label>
          </div>

          <SettingsFeedbackBanner feedback={feedback ?? null} compact />
        </div>

        <div className="ai-model-modal-foot modal-actions">
          <button type="button" className="btn-secondary" onClick={onClose} disabled={submitting}>
            取消
          </button>
          <button
            type="button"
            className="btn-primary"
            disabled={submitting || !id.trim()}
            onClick={() => void submit()}
          >
            {submitting ? "添加中…" : "添加并选用"}
          </button>
        </div>
      </div>
    </div>
  );
}
