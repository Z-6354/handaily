import { useCallback, useEffect, useState } from "react";
import { PersonaAddModal } from "../components/PersonaAddModal";
import { PersonaDeleteModal } from "../components/PersonaDeleteModal";
import { PersonaDetailPanel } from "../components/PersonaDetailPanel";
import { SettingsFeedbackBanner } from "../components/SettingsFeedbackBanner";
import {
  loadingFeedback,
  parseApiError,
  successFeedback,
  type SettingsFeedback,
} from "../lib/apiErrorMessage";
import { xiaohan, type PersonaDetail, type PersonaInfo } from "../lib/xiaohan";

const PERSONA_ACCENT: Record<string, string> = {
  cheshire: "#f59e0b",
  edu: "#8b5cf6",
  wushiling: "#06b6d4",
  qiye: "#64748b",
  tashigan: "#3b82f6",
};

function personaInitial(name: string) {
  const t = name.trim();
  return t ? t.charAt(0) : "?";
}

export function PersonaPanel() {
  const [personas, setPersonas] = useState<PersonaInfo[]>([]);
  const [detailId, setDetailId] = useState<string | null>(null);
  const [detail, setDetail] = useState<PersonaDetail | null>(null);
  const [detailLoading, setDetailLoading] = useState(false);
  const [personaFeedback, setPersonaFeedback] = useState<SettingsFeedback | null>(null);
  const [testingPersona, setTestingPersona] = useState(false);
  const [addOpen, setAddOpen] = useState(false);
  const [loading, setLoading] = useState(true);
  const [deleting, setDeleting] = useState(false);
  const [deleteTarget, setDeleteTarget] = useState<PersonaInfo | null>(null);
  const [error, setError] = useState<string | null>(null);

  const loadPersonas = async () => {
    try {
      setError(null);
      setPersonas(await xiaohan.personaList());
    } catch (e) {
      setError(String(e));
      setPersonas([]);
    } finally {
      setLoading(false);
    }
  };

  const refreshDetail = useCallback(async (id: string) => {
    setDetailLoading(true);
    try {
      setDetail(await xiaohan.personaGetDetail(id));
    } catch (e) {
      setPersonaFeedback(parseApiError(e, "加载人设详情"));
      setDetail(null);
    } finally {
      setDetailLoading(false);
    }
  }, []);

  useEffect(() => {
    loadPersonas();
  }, []);

  useEffect(() => {
    if (detailId) {
      refreshDetail(detailId);
    } else {
      setDetail(null);
    }
  }, [detailId, refreshDetail]);

  const testPersona = async () => {
    setTestingPersona(true);
    setPersonaFeedback(loadingFeedback("正在测试人设与模型…"));
    try {
      const result = await xiaohan.aiTestPersona();
      if (result.ok) {
        setPersonaFeedback({
          tone: "success",
          title: result.message,
          detail: result.reply ?? undefined,
        });
      } else {
        setPersonaFeedback(parseApiError(result.message, "人设测试"));
      }
    } catch (e) {
      setPersonaFeedback(parseApiError(e, "人设测试"));
    } finally {
      setTestingPersona(false);
    }
  };

  const activatePersona = async (id: string) => {
    try {
      await xiaohan.personaSetActive(id);
      const list = await xiaohan.personaList();
      setPersonas(list);
      if (detailId === id) {
        await refreshDetail(id);
      }
    } catch (e) {
      setPersonaFeedback(parseApiError(e, "切换人设"));
    }
  };

  const requestDelete = (id: string) => {
    const target = personas.find((p) => p.id === id);
    if (target?.is_builtin) return;
    if (target) {
      setDeleteTarget(target);
      return;
    }
    if (detail?.id === id && !detail.is_builtin) {
      setDeleteTarget({
        id: detail.id,
        name: detail.name,
        source: detail.source,
        description: detail.description,
        active: detail.active,
        has_profile: true,
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
      setPersonaFeedback(successFeedback(`已删除人设「${label}」`));
      setDeleteTarget(null);
      if (detailId === id) {
        setDetailId(null);
        setDetail(null);
      }
      await loadPersonas();
    } catch (e) {
      setPersonaFeedback(parseApiError(e, "删除人设"));
    } finally {
      setDeleting(false);
    }
  };

  const deleteModal = (
    <PersonaDeleteModal
      open={deleteTarget !== null}
      target={deleteTarget}
      deleting={deleting}
      onClose={() => {
        if (!deleting) setDeleteTarget(null);
      }}
      onConfirm={confirmDelete}
    />
  );

  if (loading) {
    return <div className="persona-page persona-page--loading">加载人设…</div>;
  }

  if (detailId) {
    return (
      <div className="persona-page persona-page--detail">
        {error && <div className="error persona-page-error">{error}</div>}
        <PersonaDetailPanel
          detail={detail}
          loading={detailLoading}
          deleting={deleting}
          personas={personas}
          onSelectPersona={(id) => setDetailId(id)}
          onActivate={activatePersona}
          onDelete={requestDelete}
          onBack={() => setDetailId(null)}
          onUpdated={async () => {
            await loadPersonas();
            if (detailId) await refreshDetail(detailId);
          }}
        />
        <SettingsFeedbackBanner feedback={personaFeedback} compact />
        {deleteModal}
      </div>
    );
  }

  return (
    <div className="persona-page">
      {error && <div className="error persona-page-error">{error}</div>}

      <div className="persona-roster">
        <div className="persona-roster-bar">
          <button
            type="button"
            className="persona-roster-test"
            disabled={testingPersona}
            onClick={testPersona}
          >
            {testingPersona ? "测试中…" : "测试连通"}
          </button>
        </div>

        <div className="persona-grid" role="list">
          {personas.map((p) => {
            const accent = PERSONA_ACCENT[p.id] ?? "#722ed1";
            return (
              <article
                key={p.id}
                role="listitem"
                className={`persona-card${p.active ? " persona-card--active" : ""}`}
                style={{ "--persona-accent": accent } as React.CSSProperties}
              >
                <button
                  type="button"
                  className="persona-card-hit"
                  onClick={() => setDetailId(p.id)}
                  aria-label={`查看 ${p.name} 详情`}
                >
                  <div className="persona-card-cover" />
                  <div className="persona-card-avatar" aria-hidden>
                    {personaInitial(p.name)}
                  </div>
                  <div className="persona-card-body">
                    <h3 className="persona-card-name">{p.name}</h3>
                    {p.source && <span className="persona-card-chip">{p.source}</span>}
                    <p className="persona-card-desc">{p.description}</p>
                  </div>
                </button>
                <div className="persona-card-foot">
                  {p.active ? (
                    <span className="persona-card-active-pill">使用中</span>
                  ) : (
                    <>
                      <button
                        type="button"
                        className="persona-card-action persona-card-action--primary"
                        onClick={() => activatePersona(p.id)}
                      >
                        选用
                      </button>
                      <button
                        type="button"
                        className="persona-card-action"
                        onClick={() => setDetailId(p.id)}
                      >
                        详情
                      </button>
                    </>
                  )}
                  {!p.is_builtin && (
                    <button
                      type="button"
                      className="persona-card-action persona-card-action--danger"
                      onClick={() => requestDelete(p.id)}
                      disabled={deleting}
                      title={`删除 ${p.name}`}
                    >
                      删除
                    </button>
                  )}
                </div>
              </article>
            );
          })}

          <article
            role="listitem"
            className="persona-card persona-card--add"
            style={{ "--persona-accent": "#22c55e" } as React.CSSProperties}
          >
            <button
              type="button"
              className="persona-card-hit"
              onClick={() => setAddOpen(true)}
              aria-label="新增角色"
            >
              <div className="persona-card-cover" />
              <div className="persona-card-avatar persona-card-avatar--add" aria-hidden>
                +
              </div>
              <div className="persona-card-body">
                <h3 className="persona-card-name">新增角色</h3>
                <span className="persona-card-chip">自定义</span>
                <p className="persona-card-desc">粘贴 Wiki 链接或文本，AI 自动生成人设</p>
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
        </div>

        <SettingsFeedbackBanner feedback={personaFeedback} compact />
      </div>

      <PersonaAddModal
        open={addOpen}
        onClose={() => setAddOpen(false)}
        onCreated={loadPersonas}
      />
      {deleteModal}
    </div>
  );
}
