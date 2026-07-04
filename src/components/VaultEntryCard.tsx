import { useState } from "react";
import type { VaultEntry } from "../lib/xiaohan";
import { IconCopy, IconEdit, IconEye, IconEyeOff, IconTrash } from "./Icons";

interface Props {
  entry: VaultEntry;
  secret?: string;
  onReveal: () => Promise<void>;
  onHide: () => void;
  onEdit: () => void;
  onDelete: () => void;
}

function normalizeUrl(url: string): string {
  const t = url.trim();
  if (!t) return "";
  if (t.startsWith("http://") || t.startsWith("https://")) return t;
  return `https://${t}`;
}

export function VaultEntryCard({ entry, secret, onReveal, onHide, onEdit, onDelete }: Props) {
  const [loading, setLoading] = useState(false);
  const [copied, setCopied] = useState(false);
  const revealed = secret !== undefined;
  const href = entry.website_url ? normalizeUrl(entry.website_url) : "";

  const toggleReveal = async () => {
    if (revealed) {
      onHide();
      return;
    }
    setLoading(true);
    try {
      await onReveal();
    } finally {
      setLoading(false);
    }
  };

  const copySecret = async () => {
    if (!secret) return;
    try {
      await navigator.clipboard.writeText(secret);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch {
      /* ignore */
    }
  };

  return (
    <div className="vault-entry-card">
      <div className="vault-entry-name">
        {href ? (
          <a
            href={href}
            target="_blank"
            rel="noopener noreferrer"
            className="vault-name-link"
            title={href}
          >
            {entry.name}
          </a>
        ) : (
          <span className="vault-name-text">{entry.name}</span>
        )}
      </div>

      <div className="vault-entry-secret">
        <div className="vault-secret-value" title={revealed ? secret : undefined}>
          {loading ? (
            <span className="vault-secret-muted">加载中…</span>
          ) : revealed ? (
            <code>{secret}</code>
          ) : (
            <span className="masked">••••••••••••••••</span>
          )}
        </div>
        <div className="vault-secret-btns">
          <button
            type="button"
            className={`vault-icon-btn${revealed ? "" : " vault-icon-btn--reserved"}`}
            onClick={copySecret}
            disabled={!revealed}
            tabIndex={revealed ? 0 : -1}
            title="复制"
            aria-label="复制密钥"
            aria-hidden={!revealed}
          >
            <IconCopy />
          </button>
          <button
            type="button"
            className="vault-icon-btn"
            onClick={toggleReveal}
            disabled={loading}
            title={revealed ? "隐藏" : "查看"}
            aria-label={revealed ? "隐藏密钥" : "查看密钥"}
          >
            {revealed ? <IconEyeOff /> : <IconEye />}
          </button>
        </div>
        {copied && <span className="vault-copied-tip">已复制</span>}
      </div>

      <div className="vault-entry-actions">
        <button type="button" className="vault-action-btn" onClick={onEdit} title="更新密钥">
          <IconEdit size={15} />
          <span>更新</span>
        </button>
        <button
          type="button"
          className="vault-action-btn vault-action-btn--danger"
          onClick={onDelete}
          title="删除"
        >
          <IconTrash size={15} />
          <span>删除</span>
        </button>
      </div>
    </div>
  );
}
