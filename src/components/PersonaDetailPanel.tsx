import { CharacterAvatar } from "./CharacterAvatar";
import { CharacterPetSettings } from "./CharacterPetSettings";
import { CharacterSkinPicker } from "./CharacterSkinPicker";
import type { SettingsFeedback } from "../lib/apiErrorMessage";
import type { PersonaDetail } from "../lib/xiaohan";
import { characterAccent } from "../lib/characterDisplay";

type CharacterOption = {
  id: string;
  name: string;
};

type Props = {
  characterId: string;
  detail: PersonaDetail | null;
  loading: boolean;
  deleting?: boolean;
  characters: CharacterOption[];
  activeSkinId?: string;
  activeSkinName?: string;
  activeModelId?: string;
  activeModelName?: string;
  activeModelReady?: boolean;
  skinRefreshKey?: number;
  switchingSkinId?: string | null;
  onSkinSelect?: (skinId: string) => void;
  onDeleteSkin?: (skinId: string) => void;
  onSelectCharacter: (characterId: string) => void;
  onActivate: () => void;
  onDelete: () => void | Promise<void>;
  onBack: () => void;
  onSkinRefresh?: () => void | Promise<void>;
  setFeedback: (f: SettingsFeedback | null) => void;
  avatarPath?: string | null;
  characterIdForAvatar?: string;
  skinTag?: string;
};

export function PersonaDetailPanel({
  characterId,
  detail,
  loading,
  deleting = false,
  characters,
  activeSkinId,
  activeSkinName,
  activeModelId,
  activeModelName,
  activeModelReady,
  skinRefreshKey = 0,
  switchingSkinId,
  onSkinSelect,
  onDeleteSkin,
  onSelectCharacter,
  onActivate,
  onDelete,
  onBack,
  onSkinRefresh,
  setFeedback,
  avatarPath,
  characterIdForAvatar,
  skinTag,
}: Props) {
  const accent = characterAccent(characterId);

  return (
    <div
      className="persona-detail-shell"
      style={{ "--persona-accent": accent } as React.CSSProperties}
    >
      <div className="persona-detail-top">
        <div className="persona-detail-top-left">
          <button type="button" className="persona-detail-back" onClick={onBack}>
            ← 返回
          </button>
          {characters.length > 1 && detail && (
            <select
              className="persona-detail-switcher"
              value={characterId}
              onChange={(e) => onSelectCharacter(e.target.value)}
            >
              {characters.map((c) => (
                <option key={c.id} value={c.id}>
                  {c.name}
                </option>
              ))}
            </select>
          )}
        </div>
        {detail && (
          <div className="persona-detail-actions">
            {!detail.active && (
              <button type="button" className="btn-primary btn-sm" onClick={onActivate}>
                选用此人物
              </button>
            )}
            {detail.active && (
              <span className="persona-detail-active-pill">当前使用</span>
            )}
            {!detail.is_builtin && (
              <button
                type="button"
                className="btn-secondary btn-sm persona-detail-delete"
                disabled={deleting}
                onClick={onDelete}
              >
                {deleting ? "删除中…" : "删除"}
              </button>
            )}
          </div>
        )}
      </div>

      {loading && !detail ? (
        <p className="hint-block persona-detail-loading">加载详情…</p>
      ) : detail ? (
        <div className="persona-detail-layout">
          <aside className="persona-detail-side">
            <div className="persona-detail-side-cover" />
            <div className="persona-detail-side-avatar" aria-hidden>
              <CharacterAvatar
                name={detail.name}
                characterId={characterIdForAvatar ?? characterId}
                avatarPath={avatarPath}
                deferDownload
              />
            </div>
            <h3 className="persona-detail-side-name">{detail.name}</h3>
            {skinTag && (
              <div className="persona-card-tags persona-card-tags--detail">
                <span className="persona-detail-side-chip">{skinTag}</span>
              </div>
            )}
            {activeModelName && (
              <p className="persona-detail-side-meta">
                皮肤 · {activeModelName}
                {activeModelId && (
                  <>
                    <br />
                    <span className="persona-detail-model-id">模型 {activeModelId}</span>
                  </>
                )}
              </p>
            )}
            <div className="persona-detail-badges">
              {detail.is_builtin && <span className="persona-badge">内置</span>}
            </div>
          </aside>

          <section className="persona-detail-main">
            <div className="persona-detail-body">
              <div className="persona-detail-sections">
                {onSkinSelect ? (
                  <CharacterSkinPicker
                    characterId={characterId}
                    activeId={activeSkinId ?? ""}
                    refreshKey={skinRefreshKey}
                    switchingId={switchingSkinId ?? null}
                    disabled={Boolean(switchingSkinId)}
                    onSelect={onSkinSelect}
                    onDeleteSkin={onDeleteSkin}
                    canDeleteSkin={!detail.is_builtin}
                    characterActive={detail.active}
                    switchUpdatesPet={true}
                    activeModelId={activeModelId}
                    activeModelName={activeModelName}
                    activeSkinName={activeSkinName}
                    onImportComplete={onSkinRefresh}
                    setFeedback={setFeedback}
                  />
                ) : null}
                {activeModelId && activeModelReady !== false && (
                  <CharacterPetSettings
                    modelId={activeModelId}
                    setFeedback={setFeedback}
                  />
                )}
              </div>
            </div>
          </section>
        </div>
      ) : null}
    </div>
  );
}
