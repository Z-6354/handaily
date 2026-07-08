import { useMemo, useState } from "react";
import { CharacterAvatar } from "./CharacterAvatar";
import { CharacterPetSettings } from "./CharacterPetSettings";
import { CharacterSkinPicker } from "./CharacterSkinPicker";
import { PersonaEditForm } from "./PersonaEditForm";
import { PersonaTextImportForm } from "./PersonaTextImportForm";
import { PersonaRegenerateButton } from "./PersonaRegenerateButton";
import type { SettingsFeedback } from "../lib/apiErrorMessage";
import type { CharacterProfileData, PersonaDetail } from "../lib/xiaohan";
import { characterAccent } from "../lib/characterDisplay";

type Tab = "appearance" | "personality";

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
  onUpdated: () => void | Promise<void>;
  onSkinRefresh?: () => void | Promise<void>;
  setFeedback: (f: SettingsFeedback | null) => void;
  avatarPath?: string | null;
  characterIdForAvatar?: string;
  displayTags?: [string, string];
};

function SectionTitle({ children }: { children: React.ReactNode }) {
  return <h4 className="persona-section-title">{children}</h4>;
}

function FieldBlock({ label, children }: { label: string; children: React.ReactNode }) {
  if (!children || (typeof children === "string" && !children.trim())) return null;
  return (
    <div className="persona-field-block">
      <div className="persona-field-label">{label}</div>
      <div className="persona-field-value">{children}</div>
    </div>
  );
}

function profileNeedsAi(data: CharacterProfileData): boolean {
  return (
    !data.introduction.trim() ||
    (data.personality.length === 0 && !data.speech_style.trim())
  );
}

function profileUpdateStatus(detail: PersonaDetail): string {
  if (!detail.profile_ai_updated) {
    return "未更新";
  }
  if (detail.profile_ai_updated_at) {
    return `已更新 · ${detail.profile_ai_updated_at}`;
  }
  return "已更新";
}

function StructuredProfile({ data }: { data: CharacterProfileData }) {
  return (
    <div className="persona-structured">
      <FieldBlock label="介绍">{data.introduction}</FieldBlock>
      <FieldBlock label="说话风格">{data.speech_style}</FieldBlock>
      {data.personality.length > 0 && (
        <FieldBlock label="性格">
          <ul className="persona-tag-list">
            {data.personality.map((t) => (
              <li key={t}>{t}</li>
            ))}
          </ul>
        </FieldBlock>
      )}
      {data.sample_lines.length > 0 && (
        <FieldBlock label="台词示例">
          <ul className="persona-lines-list">
            {data.sample_lines.map((t) => (
              <li key={t}>「{t}」</li>
            ))}
          </ul>
        </FieldBlock>
      )}
      <FieldBlock label="关系">{data.relationships}</FieldBlock>
      {data.taboos.length > 0 && (
        <FieldBlock label="禁忌">
          <ul className="persona-tag-list persona-tag-list--muted">
            {data.taboos.map((t) => (
              <li key={t}>{t}</li>
            ))}
          </ul>
        </FieldBlock>
      )}
      {Object.keys(data.extra).length > 0 && (
        <FieldBlock label="其它">
          <dl className="persona-extra-dl">
            {Object.entries(data.extra).map(([k, v]) => (
              <div key={k} className="persona-extra-row">
                <dt>{k}</dt>
                <dd>{v}</dd>
              </div>
            ))}
          </dl>
        </FieldBlock>
      )}
    </div>
  );
}

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
  onUpdated,
  onSkinRefresh,
  setFeedback,
  avatarPath,
  characterIdForAvatar,
  displayTags,
}: Props) {
  const [tab, setTab] = useState<Tab>("appearance");

  const accent = characterAccent(characterId);
  const jsonText = useMemo(
    () => (detail ? JSON.stringify(detail.profile_json, null, 2) : ""),
    [detail],
  );

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
            <PersonaRegenerateButton
              personaId={detail.id}
              setFeedback={setFeedback}
              onSuccess={onUpdated}
            />
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
            {displayTags && (
              <div className="persona-card-tags persona-card-tags--detail">
                <span className="persona-detail-side-chip">{displayTags[0]}</span>
                <span className="persona-detail-side-chip persona-card-chip--trait">
                  {displayTags[1]}
                </span>
              </div>
            )}
            <p className="persona-detail-side-desc">{detail.description}</p>
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
            <div className="persona-detail-tabs">
              {(
                [
                  ["appearance", "皮肤 · 桌宠"],
                  ["personality", "性格"],
                ] as [Tab, string][]
              ).map(([k, label]) => (
                <button
                  key={k}
                  type="button"
                  className={`persona-detail-tab${tab === k ? " active" : ""}`}
                  onClick={() => setTab(k)}
                >
                  {label}
                </button>
              ))}
            </div>

            <div className="persona-detail-body">
              {tab === "appearance" && (
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
              )}

              {tab === "personality" && (
                <div className="persona-detail-sections">
                  <div className="persona-detail-section-head-row">
                    <SectionTitle>基本信息</SectionTitle>
                  </div>
                  {profileNeedsAi(detail.profile_json) && (
                    <p className="hint-block persona-profile-hint">
                      简介/介绍/说话风格/性格尚未完整生成。可点击右上角「AI 更新性格」，或使用下方「从 Wiki 更新」（需配置思考模型）。
                    </p>
                  )}
                  <FieldBlock label="性格 AI 更新">{profileUpdateStatus(detail)}</FieldBlock>
                  <FieldBlock label="简介">{detail.description || "（暂无）"}</FieldBlock>
                  <StructuredProfile data={detail.profile_json} />

                  <SectionTitle>性格 · Skill</SectionTitle>
                  <pre className="persona-md-preview">
                    {detail.skill_md || "（暂无 Skill 文档）"}
                  </pre>

                  <SectionTitle>导入资料</SectionTitle>
                  <p className="hint-block">
                    输入 BWIKI 舰娘名称即可更新简介、性格、Skill 与当前皮肤台词；也可粘贴文本或使用本地库。
                  </p>
                  <PersonaTextImportForm
                    mode="update"
                    personaId={detail.id}
                    characterId={characterId}
                    defaultWikiTitle={detail.name}
                    compact
                    onSuccess={async () => {
                      await onUpdated();
                      await onSkinRefresh?.();
                    }}
                  />

                  <SectionTitle>JSON</SectionTitle>
                  <pre className="persona-json-preview">{jsonText}</pre>

                  <SectionTitle>编辑</SectionTitle>
                  <PersonaEditForm detail={detail} onSaved={onUpdated} />
                </div>
              )}
            </div>
          </section>
        </div>
      ) : null}
    </div>
  );
}
