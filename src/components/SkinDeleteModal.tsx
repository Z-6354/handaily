import { useEffect } from "react";

type Props = {
  open: boolean;
  skinName: string;
  modelName?: string;
  deleting: boolean;
  onClose: () => void;
  onConfirm: () => void | Promise<void>;
};

export function SkinDeleteModal({
  open,
  skinName,
  modelName,
  deleting,
  onClose,
  onConfirm,
}: Props) {
  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape" && !deleting) onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, deleting, onClose]);

  if (!open) return null;

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
        aria-labelledby="skin-delete-title"
        aria-describedby="skin-delete-desc"
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
          <h3 id="skin-delete-title" className="persona-delete-modal-title">
            删除皮肤
          </h3>
          <p id="skin-delete-desc" className="persona-delete-modal-lead">
            确定删除皮肤 <strong>{skinName}</strong>
            {modelName ? `（${modelName}）` : ""} 吗？
          </p>
          <ul className="persona-delete-modal-list">
            <li>关联的 Spine 模型文件将一并删除（若未被其他皮肤引用）</li>
            <li>此操作不可恢复</li>
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
