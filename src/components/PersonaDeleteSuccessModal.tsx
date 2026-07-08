import { useEffect } from "react";

type Props = {
  open: boolean;
  name: string;
  onClose: () => void;
};

export function PersonaDeleteSuccessModal({ open, name, onClose }: Props) {
  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, onClose]);

  if (!open) return null;

  return (
    <div
      className="modal-overlay persona-delete-overlay"
      role="presentation"
      onClick={onClose}
    >
      <div
        className="modal-dialog persona-delete-modal persona-delete-success-modal"
        role="alertdialog"
        aria-modal="true"
        aria-labelledby="persona-delete-success-title"
        aria-describedby="persona-delete-success-desc"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="persona-delete-success-modal-icon" aria-hidden>
          <svg width="28" height="28" viewBox="0 0 24 24" fill="none">
            <path
              d="M20 6 9 17l-5-5"
              stroke="currentColor"
              strokeWidth="2"
              strokeLinecap="round"
              strokeLinejoin="round"
            />
          </svg>
        </div>

        <div className="persona-delete-modal-body">
          <h3 id="persona-delete-success-title" className="persona-delete-modal-title">
            删除成功
          </h3>
          <p id="persona-delete-success-desc" className="persona-delete-modal-lead">
            已删除人物 <strong>{name}</strong>
          </p>
        </div>

        <div className="persona-delete-modal-foot modal-actions">
          <button type="button" className="btn-primary btn-sm" onClick={onClose}>
            确定
          </button>
        </div>
      </div>
    </div>
  );
}
