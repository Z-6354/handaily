import { useEffect } from "react";
import type { PersonaInfo } from "../lib/xiaohan";

type Props = {
  open: boolean;
  target: PersonaInfo | null;
  deleting: boolean;
  onClose: () => void;
  onConfirm: () => void | Promise<void>;
};

export function PersonaDeleteModal({ open, target, deleting, onClose, onConfirm }: Props) {
  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape" && !deleting) onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, deleting, onClose]);

  if (!open || !target) return null;

  return (
    <div
      className="modal-overlay persona-delete-overlay"
      role="presentation"
      onClick={deleting ? undefined : onClose}
    >
      <div
        className="modal-dialog persona-delete-modal"
        role="alertdialog"
        aria-modal="true"
        aria-labelledby="persona-delete-title"
        aria-describedby="persona-delete-desc"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="persona-delete-modal-icon" aria-hidden>
          <svg width="28" height="28" viewBox="0 0 24 24" fill="none">
            <path
              d="M12 9v4m0 4h.01M10.29 3.86 1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0Z"
              stroke="currentColor"
              strokeWidth="1.75"
              strokeLinecap="round"
              strokeLinejoin="round"
            />
          </svg>
        </div>

        <div className="persona-delete-modal-body">
          <h3 id="persona-delete-title" className="persona-delete-modal-title">
            删除人设
          </h3>
          <p id="persona-delete-desc" className="persona-delete-modal-lead">
            确定删除 <strong>{target.name}</strong>
            {target.source ? `（${target.source}）` : ""} 吗？
          </p>
          <ul className="persona-delete-modal-list">
            <li>Skill 文档与结构化资料将被永久删除</li>
            <li>此操作不可恢复</li>
            {target.active && <li>当前使用中，删除后将自动切回内置柴郡</li>}
          </ul>
        </div>

        <div className="persona-delete-modal-foot modal-actions">
          <button
            type="button"
            className="btn-secondary btn-sm"
            disabled={deleting}
            onClick={onClose}
          >
            取消
          </button>
          <button
            type="button"
            className="btn-danger btn-sm"
            disabled={deleting}
            onClick={() => void onConfirm()}
          >
            {deleting ? "删除中…" : "确认删除"}
          </button>
        </div>
      </div>
    </div>
  );
}
