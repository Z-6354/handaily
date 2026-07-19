import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { PetActionSettings, type PetActionLayout } from "./PetActionSettings";
import { type SettingsFeedback } from "../lib/apiErrorMessage";
import { xiaohan } from "../lib/xiaohan";
import { normalizeWikiBulkImportProgress } from "../lib/wikiBulkImportProgress";

type ActionSection = "actions" | "lines" | "lines-import";

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

export function CharacterPetSettings({
  modelId,
  characterId: _characterId,
  skinId: _skinId,
  setFeedback,
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
  const modelIdRef = useRef(modelId);

  useEffect(() => {
    modelIdRef.current = modelId;
  }, [modelId]);

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
  }, []);

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
  ];

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
      </div>
    </div>
  );
}
