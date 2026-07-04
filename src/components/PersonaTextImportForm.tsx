import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { SettingsFeedbackBanner } from "./SettingsFeedbackBanner";
import { parseApiError, successFeedback, type SettingsFeedback } from "../lib/apiErrorMessage";
import {
  xiaohan,
  type PersonaImportFile,
  type PersonaImportProgressEvent,
  type PersonaImportResult,
} from "../lib/xiaohan";

type Props = {
  mode: "create" | "update";
  personaId?: string;
  onSuccess: (result: PersonaImportResult) => void | Promise<void>;
  compact?: boolean;
};

const IMPORT_STEPS = [
  { id: "preprocess", label: "解析参考文本" },
  { id: "skill", label: "生成 Skill 文档" },
  { id: "save", label: "写入人设" },
] as const;

async function readFiles(fileList: FileList): Promise<PersonaImportFile[]> {
  const out: PersonaImportFile[] = [];
  for (const file of Array.from(fileList)) {
    const ext = file.name.split(".").pop()?.toLowerCase();
    if (ext !== "md" && ext !== "txt") continue;
    out.push({ filename: file.name, content: await file.text() });
  }
  return out;
}

export function PersonaTextImportForm({ mode, personaId, onSuccess, compact }: Props) {
  const fileRef = useRef<HTMLInputElement>(null);
  const [id, setId] = useState("");
  const [name, setName] = useState("");
  const [text, setText] = useState("");
  const [importing, setImporting] = useState(false);
  const [progressStep, setProgressStep] = useState(0);
  const [progressMessage, setProgressMessage] = useState("");
  const [feedback, setFeedback] = useState<SettingsFeedback | null>(null);

  useEffect(() => {
    if (!importing) return;
    let cancelled = false;
    const setup = async () => {
      const unlisten = await listen<PersonaImportProgressEvent>("persona-import-progress", (event) => {
        if (cancelled) return;
        setProgressStep(event.payload.step_index);
        setProgressMessage(event.payload.message);
      });
      return unlisten;
    };
    let unlisten: (() => void) | undefined;
    setup().then((fn) => {
      unlisten = fn;
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [importing]);

  const resetProgress = () => {
    setProgressStep(0);
    setProgressMessage("");
  };

  const submitText = async () => {
    if (!text.trim()) {
      setFeedback(parseApiError("请粘贴或输入文本", "导入人设"));
      return;
    }
    if (mode === "create" && !id.trim()) {
      setFeedback(parseApiError("请填写人设 ID", "导入人设"));
      return;
    }
    setImporting(true);
    resetProgress();
    setFeedback(null);
    try {
      const result = await xiaohan.personaImportText({
        personaId: mode === "update" ? personaId : null,
        id: mode === "create" ? id.trim() : null,
        name: mode === "create" && name.trim() ? name.trim() : null,
        text,
      });
      setFeedback(successFeedback(result.message));
      setText("");
      if (mode === "create") {
        setId("");
        setName("");
      }
      await onSuccess(result);
    } catch (e) {
      setFeedback(parseApiError(e, "导入人设"));
    } finally {
      setImporting(false);
      resetProgress();
    }
  };

  const onFilesSelected = async (list: FileList | null) => {
    if (!list?.length) return;
    const files = await readFiles(list);
    if (files.length === 0) {
      setFeedback(parseApiError("请选择 .txt 或 .md 文本文件", "导入人设"));
      return;
    }
    setImporting(true);
    resetProgress();
    setFeedback(null);
    try {
      const result = await xiaohan.personaImport(files);
      setFeedback(successFeedback(result.message));
      if (fileRef.current) fileRef.current.value = "";
      await onSuccess(result);
    } catch (e) {
      setFeedback(parseApiError(e, "导入人设"));
    } finally {
      setImporting(false);
      resetProgress();
    }
  };

  return (
    <div className={`persona-text-import${compact ? " persona-text-import--compact" : ""}`}>
      {mode === "create" && (
        <div className="persona-text-import-fields">
          <label className="persona-text-import-label">
            人设 ID
            <input
              type="text"
              className="persona-text-import-input"
              placeholder="例如 my-role"
              value={id}
              onChange={(e) => setId(e.target.value)}
              disabled={importing}
            />
          </label>
          <label className="persona-text-import-label">
            显示名称
            <input
              type="text"
              className="persona-text-import-input"
              placeholder="可选，也可从文本首行识别"
              value={name}
              onChange={(e) => setName(e.target.value)}
              disabled={importing}
            />
          </label>
        </div>
      )}

      <label className="persona-text-import-label">
        {mode === "create" ? "参考资料（非结构化文本）" : "粘贴新的参考资料"}
        <textarea
          className="persona-text-import-textarea"
          rows={compact ? 5 : 8}
          placeholder={
            mode === "create"
              ? "粘贴 Wiki、设定集、聊天记录等参考文本，AI 将解析为结构化资料并生成 Skill"
              : "粘贴新的参考文本，AI 将与现有资料合并后重新生成 Skill"
          }
          value={text}
          onChange={(e) => setText(e.target.value)}
          disabled={importing}
        />
      </label>

      {importing && (
        <div className="persona-import-progress" role="status" aria-live="polite">
          <p className="persona-import-progress-title">AI 处理中（约 1～3 分钟）</p>
          <ol className="persona-import-progress-steps">
            {IMPORT_STEPS.map((step, index) => {
              const stepNum = index + 1;
              const state =
                progressStep > stepNum ? "done" : progressStep === stepNum ? "active" : "pending";
              return (
                <li key={step.id} className={`persona-import-progress-step persona-import-progress-step--${state}`}>
                  <span className="persona-import-progress-step-dot" aria-hidden />
                  <span className="persona-import-progress-step-label">{step.label}</span>
                </li>
              );
            })}
          </ol>
          {progressMessage && (
            <p className="persona-import-progress-detail">{progressMessage}</p>
          )}
        </div>
      )}

      <div className="persona-text-import-actions">
        <button
          type="button"
          className="btn-primary btn-sm"
          disabled={importing}
          onClick={submitText}
        >
          {importing
            ? "处理中…"
            : mode === "create"
              ? "AI 处理并创建"
              : "AI 处理并更新"}
        </button>
        <button
          type="button"
          className="btn-secondary btn-sm"
          disabled={importing}
          onClick={() => fileRef.current?.click()}
        >
          选择文件
        </button>
        <input
          ref={fileRef}
          type="file"
          accept=".txt,.md"
          multiple={mode === "create"}
          className="persona-import-input"
          onChange={(e) => onFilesSelected(e.target.files)}
        />
      </div>

      {!compact && (
        <p className="persona-text-import-hint">
          支持 .txt / .md 参考文本；需配置思考模型。AI 将解析文本 → 结构化 JSON → 生成 Skill 文档。
        </p>
      )}

      <SettingsFeedbackBanner feedback={feedback} compact />
    </div>
  );
}
