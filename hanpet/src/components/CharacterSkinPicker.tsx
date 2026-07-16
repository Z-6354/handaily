import { useEffect, useId, useState } from "react";
import { SkinImportModal } from "./SkinImportModal";
import { SkinDeleteModal } from "./SkinDeleteModal";
import { Pagination } from "./Pagination";
import { useCharacterSkins } from "../hooks/useCharacterSkins";
import type { SettingsFeedback } from "../lib/apiErrorMessage";
import { type CharacterSkinInfo } from "../lib/xiaohan";

const BUILTIN_MODEL_IDS = new Set(["chaijun", "edu", "wushiling", "qiye", "tashigan"]);

interface Props {
  characterId: string;
  characterName?: string;
  englishName?: string;
  activeId: string;
  switchingId: string | null;
  disabled?: boolean;
  /** companion 默认 spine：点卡片切小人 */
  onSelect: (skinId: string, companion?: "auto" | "spine" | "kanmusu") => void;
  onDeleteSkin?: (skinId: string) => void | Promise<void>;
  canDeleteSkin?: boolean;
  layout?: "grid" | "compact";
  canImport?: boolean;
  activeModelId?: string;
  activeModelName?: string;
  activeSkinName?: string;
  switchUpdatesPet?: boolean;
  characterActive?: boolean;
  onImportComplete?: () => void | Promise<void>;
  setFeedback?: (f: SettingsFeedback | null) => void;
  refreshKey?: number;
}

function SkinCard({
  skin,
  isCurrentSkin,
  switching,
  disabled,
  canDelete,
  onSelect,
  onRequestDelete,
}: {
  skin: CharacterSkinInfo;
  isCurrentSkin: boolean;
  switching: boolean;
  disabled?: boolean;
  canDelete?: boolean;
  onSelect: () => void;
  onRequestDelete?: () => void;
}) {
  const spineReady = Boolean(skin.model_ready);
  if (!spineReady) {
    return (
      <div
        className="pet-model-card pet-model-card--incomplete"
        role="listitem"
        aria-disabled="true"
      >
        <span className="pet-model-card-name">{skin.name}</span>
        <span className="pet-model-card-badge pet-model-card-badge--incomplete">
          小人未就绪
        </span>
        {skin.kanmusu_dir ? (
          <span className="pet-model-card-badge pet-model-card-badge--muted">有舰娘资源</span>
        ) : null}
      </div>
    );
  }

  const highlight = switching;
  const equipped = isCurrentSkin && !switching;
  const hasKanmusu = Boolean(skin.kanmusu_ready);

  return (
    <div
      className={`pet-model-card-wrap${highlight ? " is-active" : ""}${equipped ? " is-equipped" : ""}`}
    >
      <button
        type="button"
        className={`pet-model-card${highlight ? " is-active" : ""}${equipped ? " is-equipped" : ""}${switching ? " is-switching" : ""}`}
        disabled={disabled || switching}
        aria-pressed={isCurrentSkin}
        aria-busy={switching}
        onClick={onSelect}
      >
        <span className="pet-model-card-name">
          {skin.name}
          {skin.english_name ? (
            <span className="pet-model-card-en"> · {skin.english_name}</span>
          ) : null}
        </span>
        <span className="pet-model-card-badge">
          {skin.model_name || skin.model_id || "小人"}
        </span>
        {hasKanmusu ? (
          <span className="pet-model-card-badge pet-model-card-badge--muted">舰娘</span>
        ) : null}
        {switching && <span className="pet-model-card-spinner" aria-hidden />}
      </button>
      {canDelete && onRequestDelete && (
        <button
          type="button"
          className="pet-model-card-delete"
          disabled={disabled || switching}
          aria-label={`删除皮肤 ${skin.name}`}
          title="删除皮肤及模型文件"
          onClick={(e) => {
            e.stopPropagation();
            onRequestDelete();
          }}
        >
          ×
        </button>
      )}
    </div>
  );
}

function SkinAddCard({ disabled, onClick }: { disabled?: boolean; onClick: () => void }) {
  return (
    <button
      type="button"
      className="pet-model-card pet-model-card--add"
      disabled={disabled}
      aria-label="导入新皮肤"
      onClick={onClick}
    >
      <span className="pet-model-card-add-icon" aria-hidden>
        +
      </span>
      <span className="pet-model-card-name">导入皮肤</span>
    </button>
  );
}

export function CharacterSkinPicker({
  characterId,
  characterName,
  englishName,
  activeId,
  switchingId,
  disabled,
  onSelect,
  onDeleteSkin,
  canDeleteSkin = false,
  layout = "grid",
  canImport = true,
  activeModelId,
  activeModelName,
  activeSkinName,
  switchUpdatesPet = false,
  characterActive = true,
  onImportComplete,
  setFeedback,
  refreshKey = 0,
}: Props) {
  const listId = useId();
  const [open, setOpen] = useState(false);
  const [importOpen, setImportOpen] = useState(false);
  const [deleteTarget, setDeleteTarget] = useState<{
    id: string;
    name: string;
    modelName: string;
  } | null>(null);
  const [deleting, setDeleting] = useState(false);
  const [page, setPage] = useState(1);
  const { skins, total, totalPages, loading, error, refresh } = useCharacterSkins(
    characterId,
    page
  );
  const active = skins.find((s) => s.active) ?? skins.find((s) => s.id === activeId);
  const activeDisplayName =
    active?.name ?? activeSkinName ?? (activeId ? "当前皮肤" : "未选择");
  const allowDelete = canDeleteSkin && total > 1 && Boolean(onDeleteSkin);

  useEffect(() => {
    setPage(1);
  }, [characterId, refreshKey]);

  useEffect(() => {
    if (page > totalPages) setPage(totalPages);
  }, [page, totalPages]);

  useEffect(() => {
    if (!open) return;
    const onDoc = (e: MouseEvent) => {
      if (!(e.target as Element).closest?.(".pet-model-picker-compact")) setOpen(false);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false);
    };
    document.addEventListener("mousedown", onDoc);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDoc);
      document.removeEventListener("keydown", onKey);
    };
  }, [open]);

  const pick = (id: string) => {
    const target = skins.find((s) => s.id === id);
    if (!target?.model_ready) return;
    if (switchingId || disabled) return;
    if (characterActive && id === activeId) return;
    onSelect(id, "spine");
    setOpen(false);
  };

  const importDisabled = disabled || Boolean(switchingId);

  const cards = (
    <>
      <div className="pet-model-grid" role="listbox" id={listId} aria-label="人物皮肤">
        {loading && skins.length === 0 && (
          <p className="pet-model-grid-loading">加载皮肤列表…</p>
        )}
        {error && skins.length === 0 && <p className="pet-model-grid-error">{error}</p>}
        {skins.map((s) => (
          <SkinCard
            key={s.id}
            skin={s}
            isCurrentSkin={s.id === activeId}
            switching={s.id === switchingId}
            disabled={disabled || Boolean(switchingId)}
            canDelete={allowDelete && !BUILTIN_MODEL_IDS.has(s.model_id)}
            onSelect={() => pick(s.id)}
            onRequestDelete={
              onDeleteSkin
                ? () =>
                    setDeleteTarget({
                      id: s.id,
                      name: s.name,
                      modelName: s.model_name || s.kanmusu_dir || s.id,
                    })
                : undefined
            }
          />
        ))}
        {canImport && layout === "grid" && (
          <SkinAddCard disabled={importDisabled} onClick={() => setImportOpen(true)} />
        )}
      </div>
      <Pagination
        page={page}
        totalPages={totalPages}
        disabled={loading}
        onPageChange={setPage}
        className="pet-model-pagination-nav"
      />
    </>
  );

  const importModal = setFeedback ? (
    <SkinImportModal
      open={importOpen}
      characterId={characterId}
      modelId={activeModelId || characterId}
      modelName={activeModelName}
      onClose={() => setImportOpen(false)}
      onImported={async () => {
        await refresh();
        await onImportComplete?.();
      }}
      setFeedback={setFeedback}
    />
  ) : null;

  const deleteModal = (
    <SkinDeleteModal
      open={deleteTarget !== null}
      skinName={deleteTarget?.name ?? ""}
      modelName={deleteTarget?.modelName}
      deleting={deleting}
      onClose={() => {
        if (!deleting) setDeleteTarget(null);
      }}
      onConfirm={async () => {
        if (!deleteTarget || !onDeleteSkin) return;
        setDeleting(true);
        try {
          await onDeleteSkin(deleteTarget.id);
          setDeleteTarget(null);
        } finally {
          setDeleting(false);
        }
      }}
    />
  );

  if (layout === "compact") {
    return (
      <div className="pet-model-picker-compact">
        <button
          type="button"
          className={`pet-model-picker-trigger${open ? " is-open" : ""}${switchingId ? " is-busy" : ""}`}
          disabled={disabled || (total === 0 && !loading)}
          aria-haspopup="listbox"
          aria-expanded={open}
          aria-controls={listId}
          onClick={() => setOpen((v) => !v)}
        >
          <span className="pet-model-picker-trigger-label">皮肤</span>
          <span className="pet-model-picker-trigger-name">
            {switchingId ? "切换中…" : activeDisplayName}
          </span>
          {switchingId && <span className="pet-model-card-spinner" aria-hidden />}
          <span className="pet-model-picker-chevron" aria-hidden />
        </button>
        {open && <div className="pet-model-picker-popover">{cards}</div>}
        {importModal}
        {deleteModal}
      </div>
    );
  }

  return (
    <section className="pet-model-section" aria-labelledby={`${listId}-label`}>
      <div className="pet-model-section-head" id={`${listId}-label`}>
        <span className="pet-model-section-title">选择皮肤</span>
        <span className="pet-model-section-meta">
          {characterName && (
            <>
              {characterName}
              {englishName ? ` · ${englishName}` : ""}
              {" · "}
            </>
          )}
          {total > 0 ? `共 ${total} 套` : loading ? "加载中…" : ""}
          {totalPages > 1 ? ` · 第 ${page}/${totalPages} 页` : ""}
        </span>
        {switchUpdatesPet && switchingId && (
          <span className="pet-model-section-hint">正在切换模型…</span>
        )}
      </div>
      {cards}
      {importModal}
      {deleteModal}
    </section>
  );
}
