import { PersonaTextImportForm } from "./PersonaTextImportForm";

type Props = {
  open: boolean;
  onClose: () => void;
  onCreated: () => void | Promise<void>;
};

export function PersonaAddModal({ open, onClose, onCreated }: Props) {
  if (!open) return null;

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div className="modal-dialog persona-add-modal" onClick={(e) => e.stopPropagation()}>
        <div className="modal-header">
          <h3 className="modal-title">导入角色文本</h3>
          <button type="button" className="modal-close" onClick={onClose} aria-label="关闭">
            ×
          </button>
        </div>
        <div className="persona-add-modal-body">
          <PersonaTextImportForm
            mode="create"
            onSuccess={async () => {
              await onCreated();
              onClose();
            }}
          />
        </div>
      </div>
    </div>
  );
}
