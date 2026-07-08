import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { SettingsToggle } from "./SettingsToggle";
import { PetActionSettings, type PetActionLayout } from "./PetActionSettings";
import { PetActionFrequency } from "./PetActionFrequency";
import { parseApiError, type SettingsFeedback } from "../lib/apiErrorMessage";
import { xiaohan } from "../lib/xiaohan";

type ActionSection = "actions" | "lines" | "lines-import";

type Props = {
  modelId: string;
  setFeedback: (f: SettingsFeedback | null) => void;
};

function SectionTitle({ children }: { children: React.ReactNode }) {
  return <h4 className="persona-section-title">{children}</h4>;
}

export function CharacterPetSettings({ modelId, setFeedback }: Props) {
  const [busy, setBusy] = useState(false);
  const [petActiveModelId, setPetActiveModelId] = useState<string | null>(null);
  const [petEnabled, setPetEnabled] = useState(true);
  const [petScale, setPetScale] = useState(0.8);
  const [petRemarkInterval, setPetRemarkInterval] = useState(300);
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
  const scaleSaveTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const modelIdRef = useRef(modelId);
  const patchActionLayoutRef = useRef<(partial: Partial<PetActionLayout>) => void>(() => {});

  useEffect(() => {
    modelIdRef.current = modelId;
  }, [modelId]);

  useEffect(() => {
    return () => {
      if (scaleSaveTimer.current) clearTimeout(scaleSaveTimer.current);
    };
  }, []);

  const refreshStatus = useCallback(async (): Promise<boolean> => {
    const mid = modelIdRef.current;
    const [status, petStatus] = await Promise.all([
      xiaohan.petGetModelStatus(mid),
      xiaohan.petGetStatus(),
    ]);
    if (modelIdRef.current !== mid) return false;
    setPetActiveModelId(petStatus.model_id);
    setPetEnabled(status.enabled);
    setPetScale(status.scale);
    setPetRemarkInterval(status.remark_interval_sec);
    const animations = status.animations ?? [];
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
      lines: status.lines ?? [],
    });
    return animations.length > 0;
  }, [modelId]);

  const applyLive = petActiveModelId === modelId;

  useEffect(() => {
    let cancelled = false;
    let timer: ReturnType<typeof setTimeout> | null = null;

    const poll = async (attempt = 0) => {
      let hasAnimations = false;
      try {
        hasAnimations = await refreshStatus();
      } catch {
        hasAnimations = false;
      }
      if (cancelled) return;
      if (hasAnimations || attempt >= 40) {
        setAnimationsLoading(false);
        return;
      }
      setAnimationsLoading(true);
      timer = setTimeout(() => void poll(attempt + 1), 500);
    };

    setAnimationsLoading(true);
    void poll(0);

    return () => {
      cancelled = true;
      if (timer) clearTimeout(timer);
    };
  }, [modelId, refreshStatus]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    void listen<string>("pet-model-meta-updated", (event) => {
      if (event.payload === modelIdRef.current) {
        void refreshStatus().then((hasAnimations) => {
          if (hasAnimations) setAnimationsLoading(false);
        });
      }
    }).then((fn) => {
      unlisten = fn;
    });
    return () => {
      unlisten?.();
    };
  }, [refreshStatus]);
  const saveModelSettings = async (settings: {
    scale?: number;
    remarkIntervalSec?: number;
  }): Promise<boolean> => {
    try {
      await xiaohan.petSaveModelSettings(modelIdRef.current, {
        ...settings,
        applyLive,
      });
      await refreshStatus();
      return true;
    } catch (e) {
      setFeedback(parseApiError(e, "保存桌宠设置"));
      return false;
    }
  };

  const actionTabs: { id: ActionSection; label: string; badge?: string }[] = [
    { id: "actions", label: "动作分配", badge: petAnimations.length ? String(petAnimations.length) : undefined },
    { id: "lines", label: "台词便签", badge: actionLayout.lines.length ? String(actionLayout.lines.length) : undefined },
    { id: "lines-import", label: "台词导入" },
  ];

  return (
    <div className="character-pet-settings">
      <SectionTitle>桌宠显示</SectionTitle>
      <div className="pet-tab-panel pet-overview-body">
        <SettingsToggle
          label="启用桌宠"
          hint="关闭将销毁桌宠窗口；再次开启或重启应用可恢复"
          checked={petEnabled}
          onChange={async (next) => {
            setPetEnabled(next);
            setFeedback(null);
            try {
              await xiaohan.petSetEnabled(next);
              await refreshStatus();
            } catch (e) {
              setPetEnabled(!next);
              setFeedback(parseApiError(e, "桌宠开关"));
            }
          }}
        />

        <div className="settings-field">
          <div className="settings-field-body">
            <div className="settings-field-label">角色大小</div>
            <div className="settings-field-hint">也可在桌宠右键「编辑范围」中用滚轮缩放</div>
          </div>
          <div className="settings-inline-input">
            <input
              type="range"
              min={0.4}
              max={1.5}
              step={0.05}
              value={petScale}
              onChange={(e) => {
                const v = parseFloat(e.target.value) || 0.8;
                setPetScale(v);
                if (scaleSaveTimer.current) clearTimeout(scaleSaveTimer.current);
                scaleSaveTimer.current = setTimeout(() => {
                  void (async () => {
                    const ok = await saveModelSettings({ scale: v });
                    if (!ok) await refreshStatus();
                  })();
                }, 350);
              }}
            />
            <span className="settings-unit">{Math.round(petScale * 100)}%</span>
          </div>
        </div>

        <div className="settings-field">
          <div className="settings-field-body">
            <div className="settings-field-label">气泡频率</div>
            <div className="settings-field-hint">定时气泡从 AI 台词、时间线与台词库中抽取</div>
          </div>
          <select
            className="settings-select"
            value={petRemarkInterval}
            onChange={async (e) => {
              const v = parseInt(e.target.value, 10);
              setPetRemarkInterval(v);
              await saveModelSettings({ remarkIntervalSec: v });
            }}
          >
            <option value={0}>关闭</option>
            <option value={300}>每 5 分钟</option>
            <option value={600}>每 10 分钟</option>
            <option value={900}>每 15 分钟</option>
          </select>
        </div>

        <PetActionFrequency
          randomMinSec={actionLayout.randomMinSec}
          randomMaxSec={actionLayout.randomMaxSec}
          randomAnimations={actionLayout.randomAnimations}
          busy={busy}
          onPatch={(patch) => patchActionLayoutRef.current(patch)}
        />
      </div>

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
          onPatchReady={(patch) => {
            patchActionLayoutRef.current = patch;
          }}
        />
      </div>
    </div>
  );
}
