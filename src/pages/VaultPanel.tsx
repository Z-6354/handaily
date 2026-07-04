import { useEffect, useState } from "react";
import { VaultEntryCard } from "../components/VaultEntryCard";
import { VaultEntryModal, type VaultEntryForm } from "../components/VaultEntryModal";
import { IconPlus } from "../components/Icons";
import { xiaohan, type VaultEntry, type VaultStatus } from "../lib/xiaohan";

const EMPTY_FORM: VaultEntryForm = { name: "", secret: "", website_url: "" };

export function VaultPanel() {
  const [status, setStatus] = useState<VaultStatus | null>(null);
  const [entries, setEntries] = useState<VaultEntry[]>([]);
  const [password, setPassword] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [revealed, setRevealed] = useState<Record<number, string>>({});

  const [modalOpen, setModalOpen] = useState(false);
  const [modalMode, setModalMode] = useState<"add" | "edit">("add");
  const [editingId, setEditingId] = useState<number | null>(null);
  const [modalInitial, setModalInitial] = useState<VaultEntryForm>(EMPTY_FORM);

  const refresh = async () => {
    try {
      setError(null);
      const st = await xiaohan.vaultGetStatus();
      setStatus(st);
      if (st.unlocked) {
        setEntries(await xiaohan.vaultList());
      } else {
        setEntries([]);
        setRevealed({});
      }
    } catch (e) {
      setError(String(e));
    }
  };

  useEffect(() => {
    refresh();
  }, []);

  const handleSetup = async (withPassword: boolean) => {
    try {
      await xiaohan.vaultSetup(withPassword ? password : undefined);
      setPassword("");
      await refresh();
    } catch (e) {
      setError(String(e));
    }
  };

  const handleUnlock = async () => {
    try {
      await xiaohan.vaultUnlock(status?.has_password ? password : undefined);
      setPassword("");
      await refresh();
    } catch (e) {
      setError(String(e));
    }
  };

  const openAddModal = () => {
    setModalMode("add");
    setEditingId(null);
    setModalInitial(EMPTY_FORM);
    setModalOpen(true);
  };

  const openEditModal = (entry: VaultEntry) => {
    setModalMode("edit");
    setEditingId(entry.id);
    setModalInitial({
      name: entry.name,
      secret: "",
      website_url: entry.website_url,
    });
    setModalOpen(true);
  };

  const handleSave = async (form: VaultEntryForm) => {
    const input = {
      name: form.name,
      website_url: form.website_url,
      secret: form.secret,
    };
    if (modalMode === "edit" && editingId !== null) {
      await xiaohan.vaultUpdate(editingId, input);
      setRevealed((r) => {
        const next = { ...r };
        delete next[editingId];
        return next;
      });
    } else {
      await xiaohan.vaultAdd(input);
    }
    await refresh();
  };

  const revealSecret = async (id: number) => {
    try {
      const secret = await xiaohan.vaultGetSecret(id);
      setRevealed((r) => ({ ...r, [id]: secret }));
    } catch (e) {
      setError(String(e));
      throw e;
    }
  };

  const hideSecret = (id: number) => {
    setRevealed((r) => {
      const next = { ...r };
      delete next[id];
      return next;
    });
  };

  const handleDelete = async (id: number) => {
    if (!window.confirm("确定删除此 API 密钥？")) return;
    try {
      await xiaohan.vaultDelete(id);
      hideSecret(id);
      await refresh();
    } catch (e) {
      setError(String(e));
    }
  };

  if (!status) {
    return <p className="empty">加载中…</p>;
  }

  if (!status.initialized) {
    return (
      <div className="panel vault-panel vault-panel--setup">
        <div className="panel-title">密码本</div>
        <p className="hint-block">
          用于安全存放 AI API 密钥。可选择设置主密码（AES-256-GCM + PBKDF2），
          或不设密码（使用 Windows DPAPI 本机保护）。
        </p>
        {error && <div className="error">{error}</div>}
        <label className="vault-field">
          <span className="vault-field-label">主密码（可选）</span>
          <input
            className="vault-field-input"
            type="password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            placeholder="留空则仅本机可解密"
          />
        </label>
        <div className="vault-actions">
          <button className="btn-on" onClick={() => handleSetup(!!password)}>
            初始化密码本
          </button>
          <button className="btn-off" onClick={() => handleSetup(false)}>
            不设密码
          </button>
        </div>
      </div>
    );
  }

  if (!status.unlocked) {
    return (
      <div className="panel vault-panel vault-panel--setup">
        <div className="panel-title">解锁密码本</div>
        {status.has_password ? (
          <>
            <label className="vault-field">
              <span className="vault-field-label">主密码</span>
              <input
                className="vault-field-input"
                type="password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                onKeyDown={(e) => e.key === "Enter" && handleUnlock()}
              />
            </label>
            <button className="btn-on" onClick={handleUnlock}>
              解锁
            </button>
          </>
        ) : (
          <button className="btn-on" onClick={handleUnlock}>
            解锁（本机保护）
          </button>
        )}
        {error && <div className="error">{error}</div>}
      </div>
    );
  }

  return (
    <div className="vault-panel">
      <div className="panel vault-list-panel">
        <div className="panel-header">
          <div>
            <div className="panel-title">API 密钥</div>
            <p className="vault-list-desc">左侧名称可跳转官网，中间查看密钥，右侧更新或删除</p>
          </div>
          <div className="panel-header-actions">
            <button className="btn-on btn-sm vault-add-btn" onClick={openAddModal}>
              <IconPlus size={14} />
              添加
            </button>
            <button className="btn-refresh" onClick={() => xiaohan.vaultLock().then(refresh)}>
              锁定
            </button>
          </div>
        </div>
        {error && <div className="error">{error}</div>}

        {entries.length === 0 ? (
          <div className="vault-empty">
            <p>暂无密钥</p>
            <button className="btn-on btn-sm" onClick={openAddModal}>
              <IconPlus size={14} />
              添加第一条
            </button>
          </div>
        ) : (
          <div className="vault-entry-list">
            <div className="vault-entry-header">
              <span>名称</span>
              <span>API 密钥</span>
              <span>操作</span>
            </div>
            {entries.map((e) => (
              <VaultEntryCard
                key={e.id}
                entry={e}
                secret={revealed[e.id]}
                onReveal={() => revealSecret(e.id)}
                onHide={() => hideSecret(e.id)}
                onEdit={() => openEditModal(e)}
                onDelete={() => handleDelete(e.id)}
              />
            ))}
          </div>
        )}
      </div>

      <VaultEntryModal
        open={modalOpen}
        title={modalMode === "add" ? "添加 API 密钥" : "更新 API 密钥"}
        initial={modalInitial}
        secretRequired={modalMode === "add"}
        secretPlaceholder={modalMode === "edit" ? "留空则不修改密钥" : "sk-..."}
        onClose={() => setModalOpen(false)}
        onSave={handleSave}
      />
    </div>
  );
}
