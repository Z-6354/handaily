import { useEffect, useState, type FormEvent } from "react";

export interface VaultEntryForm {
  name: string;
  secret: string;
  website_url: string;
}

interface Props {
  open: boolean;
  title: string;
  initial: VaultEntryForm;
  secretRequired?: boolean;
  secretPlaceholder?: string;
  onClose: () => void;
  onSave: (form: VaultEntryForm) => Promise<void>;
}

export function VaultEntryModal({
  open,
  title,
  initial,
  secretRequired = true,
  secretPlaceholder = "API Key",
  onClose,
  onSave,
}: Props) {
  const [form, setForm] = useState(initial);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (open) {
      setForm(initial);
      setError(null);
      setSaving(false);
    }
  }, [open, initial]);

  if (!open) return null;

  const canSave =
    form.name.trim().length > 0 &&
    (secretRequired ? form.secret.trim().length > 0 : true);

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault();
    if (!canSave || saving) return;
    try {
      setSaving(true);
      setError(null);
      await onSave({
        name: form.name.trim(),
        secret: form.secret,
        website_url: form.website_url.trim(),
      });
      onClose();
    } catch (err) {
      setError(String(err));
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="modal-overlay vault-modal-overlay" onClick={onClose}>
      <div className="modal-dialog vault-modal" onClick={(e) => e.stopPropagation()}>
        <div className="modal-header">
          <h2 className="modal-title">{title}</h2>
          <button type="button" className="modal-close" onClick={onClose} aria-label="关闭">
            ×
          </button>
        </div>
        <form className="vault-modal-form" onSubmit={handleSubmit}>
          {error && <div className="error">{error}</div>}

          <label className="vault-field">
            <span className="vault-field-label">名称</span>
            <input
              className="vault-field-input"
              value={form.name}
              onChange={(e) => setForm({ ...form, name: e.target.value })}
              placeholder="如 OpenAI 主密钥"
              autoFocus
            />
          </label>

          <label className="vault-field">
            <span className="vault-field-label">API 密钥</span>
            <input
              className="vault-field-input vault-field-input--mono"
              type="password"
              value={form.secret}
              onChange={(e) => setForm({ ...form, secret: e.target.value })}
              placeholder={secretPlaceholder}
            />
          </label>

          <label className="vault-field">
            <span className="vault-field-label">
              所在网址
              <span className="vault-field-optional">可选</span>
            </span>
            <input
              className="vault-field-input"
              type="url"
              value={form.website_url}
              onChange={(e) => setForm({ ...form, website_url: e.target.value })}
              placeholder="https://platform.openai.com/api-keys"
            />
            <span className="vault-field-hint">点击左侧名称可跳转此网址</span>
          </label>

          <div className="modal-actions">
            <button type="button" className="btn-off" onClick={onClose} disabled={saving}>
              取消
            </button>
            <button type="submit" className="btn-on" disabled={!canSave || saving}>
              {saving ? "保存中…" : "保存"}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
