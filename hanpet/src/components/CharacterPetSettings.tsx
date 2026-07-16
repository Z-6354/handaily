import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { PetActionSettings, type PetActionLayout } from "./PetActionSettings";
import {
  parseApiError,
  successFeedback,
  type SettingsFeedback,
} from "../lib/apiErrorMessage";
import { xiaohan } from "../lib/xiaohan";
import { normalizeWikiBulkImportProgress } from "../lib/wikiBulkImportProgress";

type ActionSection = "actions" | "lines" | "lines-import" | "kanmusu";

type Props = {
  modelId: string;
  characterId: string;
  skinId: string;
  setFeedback: (f: SettingsFeedback | null) => void;
  onRefresh?: () => void | Promise<void>;
};

function SectionTitle({ children }: { children: React.ReactNode }) {
  return <h4 className="persona-section-title">{children}</h4>;
}

/** aidang_2 → aidang；与 Rust split_slug 对齐 */
function kanmusuCharIdFromDir(dir: string): string {
  const lower = dir.trim().toLowerCase();
  const idx = lower.lastIndexOf("_");
  if (idx <= 0) return lower;
  const suffix = lower.slice(idx + 1);
  if (suffix && [...suffix].every((c) => c >= "0" && c <= "9")) {
    return lower.slice(0, idx);
  }
  return lower;
}

type KanmusuMetaSummary = {
  idle: string;
  click: string;
  touchCount: number;
  animCount: number;
};

export function CharacterPetSettings({
  modelId,
  characterId,
  skinId,
  setFeedback,
  onRefresh,
}: Props) {
  const [busy, setBusy] = useState(false);
  const [petActiveModelId, setPetActiveModelId] = useState<string | null>(null);
  const [petAnimations, setPetAnimations] = useState<string[]>([]);
  const [animationsLoading, setAnimationsLoading] = useState(false);
  const [actionLayout, setActionLayout] = useState<PetActionLayout>({
    idleAnimation: "",
    clickAnimation: "",
    bootAnimation: "",
    returnIdleAnimation: "",
    dragAnimation: "",
    randomAnimations: [],
    randomMinSec: 30,
    randomMaxSec: 120,
    lines: [],
  });
  const [actionSection, setActionSection] = useState<ActionSection>("actions");
  const [kanmusuDir, setKanmusuDir] = useState<string | null>(null);
  const [kanmusuReady, setKanmusuReady] = useState(false);
  const [kanmusuBusy, setKanmusuBusy] = useState(false);
  const [kanmusuMeta, setKanmusuMeta] = useState<KanmusuMetaSummary | null>(null);
  const modelIdRef = useRef(modelId);

  useEffect(() => {
    modelIdRef.current = modelId;
  }, [modelId]);

  const refreshKanmusu = useCallback(async () => {
    if (!characterId || !skinId) {
      setKanmusuDir(null);
      setKanmusuReady(false);
      setKanmusuMeta(null);
      return;
    }
    try {
      const skin = await xiaohan.charactersGetSkin(characterId, skinId);
      const dir = skin.kanmusu_dir?.trim() || null;
      setKanmusuDir(dir);
      setKanmusuReady(Boolean(skin.kanmusu_ready));

      // 验收 #4：idle / click / 触区摘要（读 kanmusu detail / meta）
      let meta: KanmusuMetaSummary | null = null;
      if (dir) {
        const candidates = [characterId, kanmusuCharIdFromDir(dir)].filter(
          (id, i, arr) => id && arr.indexOf(id) === i,
        );
        for (const cid of candidates) {
          try {
            const detail = await xiaohan.kanmusuGetDetail(cid);
            const km =
              detail.skins.find((s) => s.model_dir === dir || s.id === dir) ??
              detail.skins.find((s) => s.id === skinId);
            if (km) {
              meta = {
                idle: (km.idle_animation ?? "").trim() || "—",
                click: (km.click_animation ?? "").trim() || "—",
                touchCount: km.touch_area_count ?? 0,
                animCount: km.animations?.length ?? 0,
              };
              break;
            }
          } catch {
            /* try next id */
          }
        }
      }
      setKanmusuMeta(meta);
    } catch {
      setKanmusuDir(null);
      setKanmusuReady(false);
      setKanmusuMeta(null);
    }
  }, [characterId, skinId]);

  useEffect(() => {
    void refreshKanmusu();
  }, [refreshKanmusu]);

  const refreshStatus = useCallback(async (): Promise<{
    hasAnimations: boolean;
    hasLines: boolean;
  }> => {
    const mid = modelIdRef.current;
    const [status, petStatus] = await Promise.all([
      xiaohan.petGetModelStatus(mid),
      xiaohan.petGetStatus(),
    ]);
    if (modelIdRef.current !== mid) return { hasAnimations: false, hasLines: false };
    setPetActiveModelId(petStatus.model_id);
    const animations = status.animations ?? [];
    const lines = status.lines ?? [];
    setPetAnimations(animations);
    setActionLayout({
      idleAnimation: status.idle_animation ?? "",
      clickAnimation: status.click_animation ?? "",
      bootAnimation: status.boot_animation ?? status.idle_animation ?? "",
      returnIdleAnimation: status.return_idle_animation ?? status.idle_animation ?? "",
      dragAnimation: status.drag_animation ?? "",
      randomAnimations: status.random_animations ?? [],
      randomMinSec: status.random_min_sec ?? 30,
      randomMaxSec: status.random_max_sec ?? 120,
      lines,
    });
    return { hasAnimations: animations.length > 0, hasLines: lines.length > 0 };
  }, [modelId]);

  const applyLive = petActiveModelId === modelId;

  useEffect(() => {
    let cancelled = false;
    let timer: ReturnType<typeof setTimeout> | null = null;

    const poll = async (attempt = 0) => {
      let ready = { hasAnimations: false, hasLines: false };
      try {
        ready = await refreshStatus();
      } catch {
        ready = { hasAnimations: false, hasLines: false };
      }
      if (cancelled) return;
      const hasContent = ready.hasAnimations || ready.hasLines;
      if (hasContent || attempt >= 20) {
        setAnimationsLoading(false);
        return;
      }
      setAnimationsLoading(true);
      timer = setTimeout(() => void poll(attempt + 1), Math.min(1500, 500 + attempt * 100));
    };

    setAnimationsLoading(true);
    void poll(0);

    return () => {
      cancelled = true;
      if (timer) clearTimeout(timer);
    };
  }, [modelId, refreshStatus]);

  useEffect(() => {
    let unlistenMeta: (() => void) | undefined;
    let unlistenBulk: (() => void) | undefined;
    void listen<string>("pet-model-meta-updated", (event) => {
      if (event.payload === modelIdRef.current) {
        void refreshStatus().then((ready) => {
          if (ready.hasAnimations || ready.hasLines) setAnimationsLoading(false);
        });
      }
    }).then((fn) => {
      unlistenMeta = fn;
    });
    void listen<Record<string, unknown>>("pet-wiki-bulk-import-progress", (event) => {
      const phase = normalizeWikiBulkImportProgress(event.payload).phase;
      if (phase === "done" || phase === "error") {
        void refreshStatus().then((ready) => {
          if (ready.hasAnimations || ready.hasLines) setAnimationsLoading(false);
        });
      }
    }).then((fn) => {
      unlistenBulk = fn;
    });
    return () => {
      unlistenMeta?.();
      unlistenBulk?.();
    };
  }, [refreshStatus]);

  const actionTabs: { id: ActionSection; label: string; badge?: string }[] = [
    {
      id: "actions",
      label: "动作分配",
      badge: petAnimations.length ? String(petAnimations.length) : undefined,
    },
    {
      id: "lines",
      label: "台词",
      badge: actionLayout.lines.length ? String(actionLayout.lines.length) : undefined,
    },
    { id: "lines-import", label: "台词导入" },
    {
      id: "kanmusu",
      label: "舰娘皮肤",
      badge: kanmusuReady ? "就绪" : kanmusuDir ? "未就绪" : undefined,
    },
  ];

  const syncKanmusu = async () => {
    setKanmusuBusy(true);
    setFeedback(null);
    try {
      const result = await xiaohan.kanmusuSyncFromUnpacked();
      await refreshKanmusu();
      await onRefresh?.();
      setFeedback(
        successFeedback(
          result.message || "已同步舰娘模型",
          `新增角色 ${result.added_characters} · 皮肤 ${result.added_skins}`,
        ),
      );
    } catch (e) {
      setFeedback(parseApiError(e, "同步舰娘"));
    } finally {
      setKanmusuBusy(false);
    }
  };

  const openKanmusuDesktop = async () => {
    if (!characterId || !skinId) return;
    setKanmusuBusy(true);
    setFeedback(null);
    try {
      // 顶替桌宠窗显示 Cubism（不走独立预览窗）
      await xiaohan.charactersSetSkin(characterId, skinId, "kanmusu");
      await onRefresh?.();
      setFeedback(successFeedback("已用舰娘顶替桌宠"));
    } catch (e) {
      setFeedback(parseApiError(e, "舰娘上桌"));
    } finally {
      setKanmusuBusy(false);
    }
  };

  const openKanmusuPreview = async () => {
    if (!characterId || !skinId) return;
    setKanmusuBusy(true);
    setFeedback(null);
    try {
      await xiaohan.kanmusuPlayerLoad(characterId, skinId);
      setFeedback(successFeedback("已打开舰娘预览窗"));
    } catch (e) {
      // 人物 id 与 kanmusu slug 不一致时，用目录名前缀重试
      try {
        if (!kanmusuDir) throw e;
        const cid = kanmusuCharIdFromDir(kanmusuDir);
        await xiaohan.kanmusuPlayerLoad(cid, kanmusuDir);
        setFeedback(successFeedback("已打开舰娘预览窗"));
      } catch (e2) {
        setFeedback(parseApiError(e2, "舰娘预览"));
      }
    } finally {
      setKanmusuBusy(false);
    }
  };

  return (
    <div className="character-pet-settings">
      <SectionTitle>动作与台词</SectionTitle>
      <div className="pet-tab-bar pet-tab-bar--nested" role="tablist" aria-label="动作与台词">
        {actionTabs.map((tab) => (
          <button
            key={tab.id}
            type="button"
            role="tab"
            aria-selected={actionSection === tab.id}
            className={`pet-tab${actionSection === tab.id ? " is-active" : ""}`}
            onClick={() => setActionSection(tab.id)}
          >
            <span className="pet-tab-label">{tab.label}</span>
            {tab.badge && <span className="pet-tab-badge">{tab.badge}</span>}
          </button>
        ))}
      </div>
      <div className="pet-tab-panel">
        {actionSection === "kanmusu" ? (
          <div className="pet-tab-section pet-lines-import-body">
            <section className="pet-lines-import-block">
              <div className="pet-lines-import-block-head">
                <span className="pet-lines-import-block-badge">Cubism</span>
                <div>
                  <h4 className="pet-lines-import-block-title">当前皮肤的舰娘模型</h4>
                  <p className="pet-lines-import-block-desc">
                    小人每套皮肤都有；舰娘可选。同步后用「舰娘上桌」在桌宠窗顶替小人显示 Cubism。
                  </p>
                </div>
              </div>
              <p className="hint-block" style={{ marginTop: 8 }}>
                {kanmusuDir
                  ? kanmusuReady
                    ? `已绑定 ${kanmusuDir}（就绪）`
                    : `已绑定 ${kanmusuDir}（资源未就绪，请重新同步）`
                  : "当前皮肤尚未绑定舰娘模型"}
              </p>
              {kanmusuMeta && kanmusuReady && (
                <p className="hint-block" style={{ marginTop: 6 }}>
                  待机 <code>{kanmusuMeta.idle}</code>
                  {" · "}
                  点击 <code>{kanmusuMeta.click}</code>
                  {" · "}
                  触区 {kanmusuMeta.touchCount}
                  {" · "}
                  动作 {kanmusuMeta.animCount}
                </p>
              )}
              <div className="pet-lines-import-row" style={{ marginTop: 12, gap: 8 }}>
                <button
                  type="button"
                  className="btn-secondary btn-sm"
                  disabled={kanmusuBusy}
                  onClick={() => void syncKanmusu()}
                >
                  {kanmusuBusy ? "处理中…" : "从解包同步舰娘"}
                </button>
                <button
                  type="button"
                  className="btn-secondary btn-sm"
                  disabled={kanmusuBusy || !kanmusuReady}
                  onClick={() => void openKanmusuPreview()}
                >
                  舰娘预览
                </button>
                <button
                  type="button"
                  className="btn-primary btn-sm"
                  disabled={kanmusuBusy || !kanmusuReady}
                  onClick={() => void openKanmusuDesktop()}
                >
                  舰娘上桌
                </button>
              </div>
            </section>
          </div>
        ) : (
          <PetActionSettings
            modelId={modelId}
            animations={petAnimations}
            animationsLoading={animationsLoading}
            layout={actionLayout}
            busy={busy}
            section={actionSection}
            applyLive={applyLive}
            onLayoutChange={setActionLayout}
            onSaved={async () => {
              await refreshStatus();
            }}
            setFeedback={setFeedback}
            setBusy={setBusy}
            onFocusImportTab={() => setActionSection("lines-import")}
          />
        )}
      </div>
    </div>
  );
}
