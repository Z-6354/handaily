import { useCallback, useEffect, useId, useMemo, useState } from "react";
import { SkinImportModal } from "./SkinImportModal";
import { SkinDeleteModal } from "./SkinDeleteModal";
import { Pagination } from "./Pagination";
import { SKIN_PAGE_SIZE } from "../hooks/useCharacterSkins";
import type { SettingsFeedback } from "../lib/apiErrorMessage";
import {
  filterSkinsByKind,
  readStoredSkinKind,
  writeStoredSkinKind,
  type SkinKind,
} from "../lib/skinKindFilter";
import { xiaohan, type CharacterSkinInfo } from "../lib/xiaohan";

const BUILTIN_MODEL_IDS = new Set(["chaijun", "edu", "wushiling", "qiye", "tashigan"]);
const FETCH_LIMIT = 200;

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
  kind,
  isCurrentSkin,
  switching,
  disabled,
  canDelete,
  onSelect,
  onRequestDelete,
}: {
  skin: CharacterSkinInfo;
  kind: SkinKind;
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
          {kind === "kanmusu" ? "桌宠未就绪" : "小人未就绪"}
        </span>
      </div>
    );
  }

  const highlight = switching;
  const equipped = isCurrentSkin && !switching;

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
        {kind === "spine" ? (
          <span className="pet-model-card-badge">
            {skin.model_name && skin.model_name !== skin.model_id
              ? skin.model_name
              : "小人"}
          </span>
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
  const [kind, setKind] = useState<SkinKind>(() => readStoredSkinKind());
  const [allSkins, setAllSkins] = useState<CharacterSkinInfo[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const loadSkins = useCallback(async () => {
    if (!characterId) return;
    setLoading(true);
    setError(null);
    try {
      const result = await xiaohan.charactersSkinsPage(characterId, 0, FETCH_LIMIT);
      setAllSkins(result.items);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      setAllSkins([]);
    } finally {
      setLoading(false);
    }
  }, [characterId]);

  useEffect(() => {
    setPage(1);
    void loadSkins();
  }, [characterId, refreshKey, loadSkins]);

  useEffect(() => {
    setPage(1);
  }, [kind]);

  const filtered = useMemo(() => filterSkinsByKind(allSkins, kind), [allSkins, kind]);
  const total = filtered.length;
  const totalPages = Math.max(1, Math.ceil(total / SKIN_PAGE_SIZE));
  const skins = useMemo(() => {
    const start = (Math.max(1, page) - 1) * SKIN_PAGE_SIZE;
    return filtered.slice(start, start + SKIN_PAGE_SIZE);
  }, [filtered, page]);

  useEffect(() => {
    if (page > totalPages) setPage(totalPages);
  }, [page, totalPages]);

  const active =
    allSkins.find((s) => s.active) ?? allSkins.find((s) => s.id === activeId);
  const activeDisplayName =
    active?.name ?? activeSkinName ?? (activeId ? "当前皮肤" : "未选择");
  const allowDelete = canDeleteSkin && allSkins.length > 1 && Boolean(onDeleteSkin);

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

  const selectKind = (next: SkinKind) => {
    setKind(next);
    writeStoredSkinKind(next);
  };

  const pick = (id: string) => {
    const target = allSkins.find((s) => s.id === id);
    if (!target?.model_ready) return;
    if (switchingId || disabled) return;
    if (characterActive && id === activeId) return;
    onSelect(id, "spine");
    setOpen(false);
  };

  const importDisabled = disabled || Boolean(switchingId);

  const kindTabs = layout === "grid" && (
    <div className="pet-tab-bar pet-tab-bar--nested" role="tablist" aria-label="皮肤类型">
      {(
        [
          { id: "spine" as const, label: "桌宠" },
          { id: "kanmusu" as const, label: "舰娘" },
        ] as const
      ).map((tab) => (
        <button
          key={tab.id}
          type="button"
          role="tab"
          aria-selected={kind === tab.id}
          className={`pet-tab${kind === tab.id ? " is-active" : ""}`}
          onClick={() => selectKind(tab.id)}
        >
          <span className="pet-tab-label">{tab.label}</span>
        </button>
      ))}
    </div>
  );

  const cards = (
    <>
      <div className="pet-model-grid" role="listbox" id={listId} aria-label="人物皮肤">
        {loading && skins.length === 0 && (
          <p className="pet-model-grid-loading">加载皮肤列表…</p>
        )}
        {error && skins.length === 0 && <p className="pet-model-grid-error">{error}</p>}
        {!loading && !error && total === 0 && (
          <p className="pet-model-grid-empty">
            {kind === "kanmusu"
              ? "当前角色暂无舰娘皮肤资源"
              : "当前角色暂无桌宠皮肤"}
          </p>
        )}
        {skins.map((s) => (
          <SkinCard
            key={s.id}
            skin={s}
            kind={kind}
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
        {canImport && layout === "grid" && kind === "spine" && (
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
        await loadSkins();
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
          await loadSkins();
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
      {kindTabs}
      {cards}
      {importModal}
      {deleteModal}
    </section>
  );
}
