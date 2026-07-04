import { useState } from "react";

interface Props {
  open: boolean;
  onClose: () => void;
  onSubmit: (id: string, name: string) => void | Promise<void>;
}

export function AiModelModal({ open, onClose, onSubmit }: Props) {
  const [id, setId] = useState("");
  const [name, setName] = useState("");
  const [submitting, setSubmitting] = useState(false);

  if (!open) return null;

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <div className="modal-title">手动添加模型</div>
        <div className="setting-row">
          <label>模型 ID</label>
          <input
            value={id}
            onChange={(e) => setId(e.target.value)}
            placeholder="供应商文档中的模型 ID"
          />
        </div>
        <div className="setting-row">
          <label>显示名称</label>
          <input
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="可选，默认同 ID"
          />
        </div>
        <div className="modal-actions">
          <button className="btn-secondary" onClick={onClose} disabled={submitting}>
            取消
          </button>
          <button
            className="btn-primary"
            disabled={submitting || !id.trim()}
            onClick={async () => {
              if (!id.trim() || submitting) return;
              setSubmitting(true);
              try {
                await onSubmit(id.trim(), name.trim() || id.trim());
                setId("");
                setName("");
                onClose();
              } finally {
                setSubmitting(false);
              }
            }}
          >
            {submitting ? "添加中…" : "添加"}
          </button>
        </div>
      </div>
    </div>
  );
}
