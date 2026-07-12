import { useCallback, useEffect, useRef, useState } from "react";
import { CharacterAvatar } from "../components/CharacterAvatar";
import { PersonaAddModal } from "../components/PersonaAddModal";
import { PersonaDeleteModal } from "../components/PersonaDeleteModal";
import { PersonaDeleteSuccessModal } from "../components/PersonaDeleteSuccessModal";
import { PersonaDetailPanel } from "../components/PersonaDetailPanel";
import { SettingsFeedbackToast } from "../components/SettingsFeedbackToast";
import {
  loadingFeedback,
  parseApiError,
  successFeedback,
  type SettingsFeedback,
} from "../lib/apiErrorMessage";
import {
  xiaohan,
  type CharacterDetail,
  type CharacterBrief,
  type PersonaDetail,
  type PersonaInfo,
} from "../lib/xiaohan";
import { useCharacterFavorites } from "../hooks/useCharacterFavorites";
import {
  GRID_SLOTS,
  useCharacterRoster,
} from "../hooks/useCharacterRoster";
import { useSearchHistory } from "../hooks/useSearchHistory";
import { Pagination } from "../components/Pagination";
import { RosterPackImportButton } from "../components/RosterPackImportButton";
import { characterAccent, characterSkinTag } from "../lib/characterDisplay";

function toPersonaDetail(c: CharacterDetail): PersonaDetail {
  return {
    id: c.persona_id,
    name: c.name,
    source: c.source,
    description: c.description,
    active: c.active,
    skill_md: c.skill_md,
    profile_json: c.profile_json,
    is_builtin: c.is_builtin,
    profile_ai_updated: c.profile_ai_updated,
    profile_ai_updated_at: c.profile_ai_updated_at,
  };
}

function briefToDetailPlaceholder(brief: CharacterBrief): CharacterDetail {
  return {
    id: brief.id,
    name: brief.name,
    source: brief.source,
    description: brief.description,
    persona_id: brief.persona_id,
    active: brief.active,
    active_skin_id: brief.active_skin_id,
    active_skin_name: brief.active_skin_name,
    active_model_id: "",
    active_model_name: brief.active_skin_name,
    active_model_ready: false,
    skin_count: brief.skin_count,
    is_builtin: brief.is_builtin,
    faction: brief.faction,
    ship_type: brief.ship_type,
    rarity: brief.rarity,
    trait_summary: brief.trait_summary,
    avatar_path: brief.avatar_path,
    avatar_url: brief.avatar_url,
    skill_md: "",
    profile_json: {
      name: brief.name,
      source: brief.source,
      introduction: "",
      speech_style: "",
      personality: [],
      sample_lines: [],
      relationships: "",
      taboos: [],
      extra: {},
    },
    has_profile: false,
    profile_ai_updated: false,
    profile_ai_updated_at: null,
  };
}

export function PersonaPanel() {
  const [detailId, setDetailId] = useState<string | null>(null);
  const [characterDetail, setCharacterDetail] = useState<CharacterDetail | null>(null);
  const [detailLoading, setDetailLoading] = useState(false);
  const [personaFeedback, setPersonaFeedback] = useState<SettingsFeedback | null>(null);
  const [skinRefreshKey, setSkinRefreshKey] = useState(0);
  const [addOpen, setAddOpen] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const [deleteTarget, setDeleteTarget] = useState<PersonaInfo | null>(null);
  const [deleteSuccessName, setDeleteSuccessName] = useState<string | null>(null);
  const [switchingSkinId, setSwitchingSkinId] = useState<string | null>(null);
  const [searchInput, setSearchInput] = useState("");
  const [activeQuery, setActiveQuery] = useState("");
  const [page, setPage] = useState(1);
  const [historyOpen, setHistoryOpen] = useState(false);
  const [favoritesOnly, setFavoritesOnly] = useState(false);
  const detailRequestRef = useRef(0);
  const { favoriteIds, toggleFavorite, isFavorite } = useCharacterFavorites();
  const { history, add: addSearchHistory, remove: removeSearchHistory } = useSearchHistory();

  const showAddCard = !favoritesOnly && !activeQuery;
  const pageSize = showAddCard ? GRID_SLOTS - 1 : GRID_SLOTS;

  const onAvatarsCached = useCallback((paths: Record<string, string>) => {
    if (detailId && paths[detailId]) {
      setCharacterDetail((prev) =>
        prev ? { ...prev, avatar_path: paths[detailId] } : prev
      );
    }
  }, [detailId]);

  const {
    characters,
    totalPages,
    loading,
    refreshing,
    error,
    refresh,
  } = useCharacterRoster({
    query: activeQuery,
    page,
    pageSize,
    favoritesOnly,
    favoriteIds,
    onAvatarsCached,
  });

  useEffect(() => {
    if (page > totalPages) setPage(totalPages);
  }, [page, totalPages]);

  const commitSearch = useCallback(
    (raw?: string) => {
      const q = (raw ?? searchInput).trim();
      setSearchInput(q);
      setActiveQuery(q);
      setPage(1);
      setHistoryOpen(false);
      if (q) addSearchHistory(q);
    },
    [searchInput, addSearchHistory]
  );

  useEffect(() => {
    const onDoc = (e: MouseEvent) => {
      if (!(e.target as Element).closest?.(".persona-roster-search-wrap")) {
        setHistoryOpen(false);
      }
    };
    document.addEventListener("mousedown", onDoc);
    return () => document.removeEventListener("mousedown", onDoc);
  }, []);

  const openDetail = useCallback(
    (characterId: string) => {
      setDetailId(characterId);
      const brief = characters.find((c) => c.id === characterId);
      setCharacterDetail(brief ? briefToDetailPlaceholder(brief) : null);
      setDetailLoading(true);
    },
    [characters]
  );

  const refreshDetail = useCallback(async (characterId: string) => {
    const requestId = ++detailRequestRef.current;
    setDetailLoading(true);
    try {
      const detail = await xiaohan.charactersGetDetail(characterId);
      if (detailRequestRef.current === requestId) {
        setCharacterDetail(detail);
      }
    } catch (e) {
      if (detailRequestRef.current === requestId) {
        setPersonaFeedback(parseApiError(e, "加载人物详情"));
      }
    } finally {
      if (detailRequestRef.current === requestId) {
        setDetailLoading(false);
      }
    }
  }, []);

  useEffect(() => {
    let lastRefresh = 0;
    const onVisible = () => {
      if (document.visibilityState !== "visible") return;
      const now = Date.now();
      if (now - lastRefresh < 30_000) return;
      lastRefresh = now;
      void refresh();
    };
    window.addEventListener("focus", onVisible);
    document.addEventListener("visibilitychange", onVisible);
    return () => {
      window.removeEventListener("focus", onVisible);
      document.removeEventListener("visibilitychange", onVisible);
    };
  }, [refresh]);

  useEffect(() => {
    if (detailId) {
      refreshDetail(detailId);
    } else {
      setCharacterDetail(null);
    }
  }, [detailId, refreshDetail]);

  const activateCharacter = async (characterId: string) => {
    try {
      await xiaohan.charactersSetActive(characterId);
      await refresh();
      if (detailId === characterId) {
        await refreshDetail(characterId);
        setSkinRefreshKey((k) => k + 1);
      }
    } catch (e) {
      setPersonaFeedback(parseApiError(e, "切换人物"));
    }
  };

  const switchSkin = async (characterId: string, skinId: string) => {
    if (switchingSkinId) return;
    setSwitchingSkinId(skinId);
    try {
      await xiaohan.charactersSetSkin(characterId, skinId);
      await refresh();
      if (detailId === characterId) {
        await refreshDetail(characterId);
        setSkinRefreshKey((k) => k + 1);
      }
    } catch (e) {
      setPersonaFeedback(parseApiError(e, "切换皮肤"));
    } finally {
      setSwitchingSkinId(null);
    }
  };

  const deleteSkin = async (characterId: string, skinId: string) => {
    if (switchingSkinId) return;
    setSwitchingSkinId(skinId);
    setPersonaFeedback(loadingFeedback("正在删除皮肤…"));
    try {
      await xiaohan.charactersRemoveSkin(characterId, skinId, true);
      setPersonaFeedback(successFeedback("已删除皮肤及模型文件"));
      await refresh();
      if (detailId === characterId) {
        await refreshDetail(characterId);
        setSkinRefreshKey((k) => k + 1);
      }
    } catch (e) {
      setPersonaFeedback(parseApiError(e, "删除皮肤"));
    } finally {
      setSwitchingSkinId(null);
    }
  };

  const requestDelete = (personaId: string) => {
    const target = characters.find((c) => c.persona_id === personaId);
    if (target?.is_builtin) return;
    if (target) {
      setDeleteTarget({
        id: target.persona_id,
        name: target.name,
        source: target.source,
        description: target.description,
        active: target.active,
        has_profile: true,
        is_builtin: false,
      });
      return;
    }
    if (characterDetail?.persona_id === personaId && !characterDetail.is_builtin) {
      setDeleteTarget({
        id: characterDetail.persona_id,
        name: characterDetail.name,
        source: characterDetail.source,
        description: characterDetail.description,
        active: characterDetail.active,
        has_profile: characterDetail.has_profile,
        is_builtin: false,
      });
    }
  };

  const confirmDelete = async () => {
    if (!deleteTarget || deleteTarget.is_builtin) return;
    const { id, name: label } = deleteTarget;
    setDeleting(true);
    try {
      await xiaohan.personaDelete(id);
      setDeleteSuccessName(label);
      setDeleteTarget(null);
      if (characterDetail?.persona_id === id) {
        setDetailId(null);
        setCharacterDetail(null);
      }
      await refresh();
    } catch (e) {
      setPersonaFeedback(parseApiError(e, "删除人物"));
    } finally {
      setDeleting(false);
    }
  };

  const deleteModal = (
    <>
      <PersonaDeleteModal
        open={deleteTarget !== null}
        target={deleteTarget}
        deleting={deleting}
        onClose={() => {
          if (!deleting) setDeleteTarget(null);
        }}
        onConfirm={confirmDelete}
      />
      <PersonaDeleteSuccessModal
        open={deleteSuccessName !== null}
        name={deleteSuccessName ?? ""}
        onClose={() => setDeleteSuccessName(null)}
      />
    </>
  );

  if (loading && characters.length === 0) {
    return <div className="persona-page persona-page--loading">加载人物…</div>;
  }

  if (detailId) {
    return (
      <div className="persona-page persona-page--detail">
        {error && <div className="error persona-page-error">{error}</div>}
        <PersonaDetailPanel
          characterId={detailId}
          detail={characterDetail ? toPersonaDetail(characterDetail) : null}
          loading={detailLoading}
          deleting={deleting}
          characters={characters.map((c) => ({ id: c.id, name: c.name }))}
          activeSkinId={characterDetail?.active_skin_id}
          activeSkinName={characterDetail?.active_skin_name}
          activeModelId={characterDetail?.active_model_id}
          activeModelName={characterDetail?.active_model_name}
          activeModelReady={characterDetail?.active_model_ready}
          switchingSkinId={switchingSkinId}
          skinRefreshKey={skinRefreshKey}
          onSkinSelect={
            characterDetail
              ? (skinId) => void switchSkin(characterDetail.id, skinId)
              : undefined
          }
          onDeleteSkin={
            characterDetail
              ? (skinId) => void deleteSkin(characterDetail.id, skinId)
              : undefined
          }
          onSelectCharacter={(id) => openDetail(id)}
          onActivate={() => {
            if (characterDetail) void activateCharacter(characterDetail.id);
          }}
          onDelete={() => {
            if (characterDetail) requestDelete(characterDetail.persona_id);
          }}
          onBack={() => setDetailId(null)}
          onSkinRefresh={async () => {
            await refresh();
            if (detailId) {
              await refreshDetail(detailId);
              setSkinRefreshKey((k) => k + 1);
            }
          }}
          setFeedback={setPersonaFeedback}
          avatarPath={characterDetail?.avatar_path}
          characterIdForAvatar={detailId}
          skinTag={
            characterDetail
              ? characterSkinTag({ skin_count: characterDetail.skin_count })
              : undefined
          }
        />
        <SettingsFeedbackToast
          feedback={personaFeedback}
          onDismiss={() => setPersonaFeedback(null)}
        />
        {deleteModal}
      </div>
    );
  }

  return (
    <div className="persona-page">
      {error && <div className="error persona-page-error">{error}</div>}

      <div className="persona-roster">
        <div className="persona-roster-toolbar">
          <div className="persona-roster-toolbar-left">
            <form
              className="persona-roster-search-wrap"
              onSubmit={(e) => {
                e.preventDefault();
                commitSearch();
              }}
            >
              <label className="persona-roster-search">
                <span className="persona-roster-search-icon" aria-hidden>
                  ⌕
                </span>
                <input
                  type="search"
                  className="persona-roster-search-input"
                  placeholder="搜索人物名称"
                  value={searchInput}
                  onChange={(e) => setSearchInput(e.target.value)}
                  onFocus={() => setHistoryOpen(true)}
                  aria-label="搜索人物"
                />
                {searchInput && (
                  <button
                    type="button"
                    className="persona-roster-search-clear"
                    onClick={() => {
                      setSearchInput("");
                      setActiveQuery("");
                      setPage(1);
                      setHistoryOpen(false);
                    }}
                    aria-label="清除搜索"
                  >
                    ×
                  </button>
                )}
              </label>
              <button type="submit" className="persona-roster-search-btn" disabled={loading}>
                搜索
              </button>
              {historyOpen && history.length > 0 && (
                <div className="persona-search-history" role="listbox" aria-label="搜索历史">
                  {history.map((item) => (
                    <div key={item} className="persona-search-history-row">
                      <button
                        type="button"
                        className="persona-search-history-item"
                        onClick={() => commitSearch(item)}
                      >
                        {item}
                      </button>
                      <button
                        type="button"
                        className="persona-search-history-remove"
                        aria-label={`删除搜索历史 ${item}`}
                        onClick={() => removeSearchHistory(item)}
                      >
                        ×
                      </button>
                    </div>
                  ))}
                </div>
              )}
            </form>
            <button
              type="button"
              className={`persona-roster-fav-filter${favoritesOnly ? " active" : ""}`}
              onClick={() => {
                setFavoritesOnly((v) => !v);
                setPage(1);
              }}
              aria-pressed={favoritesOnly}
              title="只看收藏"
            >
              <span className="persona-roster-fav-filter-icon" aria-hidden>
                {favoritesOnly ? "★" : "☆"}
              </span>
              收藏
            </button>
            <RosterPackImportButton
              disabled={loading || refreshing}
              onImported={() => void refresh()}
              setFeedback={setPersonaFeedback}
            />
          </div>
          <Pagination
            page={page}
            totalPages={totalPages}
            disabled={loading || refreshing}
            onPageChange={setPage}
            className="persona-roster-pagination"
          />
        </div>

        <div className="persona-roster-main">
          <div
            className={`persona-grid${refreshing ? " persona-grid--refreshing" : ""}`}
            role="list"
          >
            {characters.length === 0 && !loading && (
              <p className="persona-grid-empty">
                {favoritesOnly ? "当前没有收藏角色" : "没有匹配的人物"}
              </p>
            )}
            {characters.map((c) => {
              const accent = characterAccent(c.id);
              const skinTag = characterSkinTag(c);
              return (
                <article
                  key={c.id}
                  role="listitem"
                  className={`persona-card${c.active ? " persona-card--active" : ""}${isFavorite(c.id) ? " persona-card--fav" : ""}`}
                  style={{ "--persona-accent": accent } as React.CSSProperties}
                >
                  <button
                    type="button"
                    className={`persona-card-fav-btn${isFavorite(c.id) ? " active" : ""}`}
                    aria-label={isFavorite(c.id) ? `取消收藏 ${c.name}` : `收藏 ${c.name}`}
                    aria-pressed={isFavorite(c.id)}
                    onClick={(e) => {
                      e.stopPropagation();
                      void toggleFavorite(c.id);
                    }}
                  >
                    {isFavorite(c.id) ? "★" : "☆"}
                  </button>
                  <button
                    type="button"
                    className="persona-card-hit"
                    onClick={() => openDetail(c.id)}
                    aria-label={`查看 ${c.name} 详情`}
                  >
                    <div className="persona-card-cover" />
                    <div className="persona-card-avatar" aria-hidden>
                      <CharacterAvatar
                        name={c.name}
                        characterId={c.id}
                        avatarPath={c.avatar_path}
                        deferDownload
                      />
                    </div>
                    <div className="persona-card-body">
                      <h3 className="persona-card-name">{c.name}</h3>
                      <div className="persona-card-tags">
                        <span className="persona-card-chip">{skinTag}</span>
                      </div>
                    </div>
                  </button>
                  <div className="persona-card-foot">
                    {c.active ? (
                      <span className="persona-card-active-pill">使用中</span>
                    ) : (
                      <>
                        <button
                          type="button"
                          className="persona-card-action persona-card-action--primary"
                          onClick={() => activateCharacter(c.id)}
                        >
                          选用
                        </button>
                        <button
                          type="button"
                          className="persona-card-action"
                          onClick={() => openDetail(c.id)}
                        >
                          详情
                        </button>
                      </>
                    )}
                    {!c.is_builtin && (
                      <button
                        type="button"
                        className="persona-card-action persona-card-action--danger"
                        onClick={() => requestDelete(c.persona_id)}
                        disabled={deleting}
                        title={`删除 ${c.name}`}
                      >
                        删除
                      </button>
                    )}
                  </div>
                </article>
              );
            })}

            {showAddCard && (
              <article
                role="listitem"
                className="persona-card persona-card--add"
                style={{ "--persona-accent": "#22c55e" } as React.CSSProperties}
              >
                <button
                  type="button"
                  className="persona-card-hit"
                  onClick={() => setAddOpen(true)}
                  aria-label="新增人物"
                >
                  <div className="persona-card-cover" />
                  <div className="persona-card-avatar persona-card-avatar--add" aria-hidden>
                    +
                  </div>
                  <div className="persona-card-body">
                    <h3 className="persona-card-name">新增人物</h3>
                    <span className="persona-card-chip">自定义</span>
                  </div>
                </button>
                <div className="persona-card-foot">
                  <button
                    type="button"
                    className="persona-card-action persona-card-action--primary persona-card-action--full"
                    onClick={() => setAddOpen(true)}
                  >
                    创建
                  </button>
                </div>
              </article>
            )}

            {Array.from({
              length: Math.max(
                0,
                GRID_SLOTS - characters.length - (showAddCard ? 1 : 0)
              ),
            }).map((_, i) => (
              <div
                key={`slot-empty-${i}`}
                className="persona-grid-slot persona-grid-slot--empty"
                aria-hidden
              />
            ))}
          </div>
        </div>

        <SettingsFeedbackToast
          feedback={personaFeedback}
          onDismiss={() => setPersonaFeedback(null)}
        />
      </div>

      <PersonaAddModal
        open={addOpen}
        onClose={() => setAddOpen(false)}
        onCreated={() => void refresh()}
      />
      {deleteModal}
    </div>
  );
}
