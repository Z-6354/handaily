import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { SettingsToggle } from "./SettingsToggle";
import { PetActionFrequency } from "./PetActionFrequency";
import { parseApiError, type SettingsFeedback } from "../lib/apiErrorMessage";
import { xiaohan, type PetStatusChangedPayload } from "../lib/xiaohan";

type Props = {
  setFeedback: (f: SettingsFeedback | null) => void;
};

export function PetDisplaySettings({ setFeedback }: Props) {
  const [loading, setLoading] = useState(true);
  const [petEnabled, setPetEnabled] = useState(true);
  const [bubbleEnabled, setBubbleEnabled] = useState(true);
  const [alwaysOnTop, setAlwaysOnTop] = useState(true);
  const [petScale, setPetScale] = useState(0.8);
  const [petRemarkInterval, setPetRemarkInterval] = useState(300);
  const [randomMinSec, setRandomMinSec] = useState(30);
  const [randomMaxSec, setRandomMaxSec] = useState(120);
  const [randomAnimations, setRandomAnimations] = useState<string[]>([]);
  const [activeModelId, setActiveModelId] = useState<string | null>(null);
  const scaleSaveTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const freqSaveTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  const applyPetStatus = useCallback((petStatus: Awaited<ReturnType<typeof xiaohan.petGetStatus>>) => {
    setPetEnabled(petStatus.active);
    setBubbleEnabled(petStatus.bubble_enabled);
    setAlwaysOnTop(petStatus.always_on_top);
    setPetScale(petStatus.scale);
    setActiveModelId(petStatus.model_id || null);
    if (petStatus.model_id) {
      setPetRemarkInterval(petStatus.remark_interval_sec);
      setRandomMinSec(petStatus.random_min_sec ?? 30);
      setRandomMaxSec(petStatus.random_max_sec ?? 120);
      setRandomAnimations(petStatus.random_animations ?? []);
    }
  }, []);

  const refreshCore = useCallback(async () => {
    const petStatus = await xiaohan.petGetStatus();
    applyPetStatus(petStatus);
    return petStatus.model_id || null;
  }, [applyPetStatus]);

  const refresh = useCallback(async () => {
    try {
      await refreshCore();
    } catch (e) {
      setFeedback(parseApiError(e, "加载桌宠显示设置"));
    } finally {
      setLoading(false);
    }
  }, [refreshCore, setFeedback]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  useEffect(() => {
    let unlistenStatus: (() => void) | undefined;
    let unlistenMain: (() => void) | undefined;
    let unlistenBubble: (() => void) | undefined;
    void listen<PetStatusChangedPayload>("pet-status-changed", (ev) => {
      setPetEnabled(ev.payload.active);
      setBubbleEnabled(ev.payload.bubble_enabled);
      setAlwaysOnTop(ev.payload.always_on_top);
    }).then((fn) => {
      unlistenStatus = fn;
    });
    void listen<boolean>("pet-bubble-enabled-changed", (ev) => {
      if (typeof ev.payload === "boolean") {
        setBubbleEnabled(ev.payload);
      }
    }).then((fn) => {
      unlistenBubble = fn;
    });
    void listen<boolean>("main-window-visible", (ev) => {
      if (ev.payload) void refreshCore();
    }).then((fn) => {
      unlistenMain = fn;
    });
    return () => {
      unlistenStatus?.();
      unlistenBubble?.();
      unlistenMain?.();
      if (scaleSaveTimer.current) clearTimeout(scaleSaveTimer.current);
      if (freqSaveTimer.current) clearTimeout(freqSaveTimer.current);
    };
  }, [refreshCore]);

  const saveModelSettings = async (settings: { remarkIntervalSec?: number }) => {
    if (!activeModelId) return false;
    try {
      await xiaohan.petSaveModelSettings(activeModelId, {
        ...settings,
        applyLive: true,
      });
      return true;
    } catch (e) {
      setFeedback(parseApiError(e, "保存桌宠显示设置"));
      return false;
    }
  };

  const saveGlobalScale = async (scale: number) => {
    try {
      await xiaohan.petSetScale(scale);
    } catch (e) {
      setFeedback(parseApiError(e, "保存角色大小"));
      await refresh();
    }
  };

  const saveRandomFrequency = async (minSec: number, maxSec: number) => {
    if (!activeModelId) return;
    try {
      const modelStatus = await xiaohan.petGetModelStatus(activeModelId);
      await xiaohan.petSaveAnimationLayout({
        model_id: activeModelId,
        idle_animation: modelStatus.idle_animation ?? null,
        click_animation: modelStatus.click_animation ?? null,
        boot_animation: modelStatus.boot_animation ?? null,
        return_idle_animation: modelStatus.return_idle_animation ?? null,
        drag_animation: modelStatus.drag_animation ?? null,
        random_animations: modelStatus.random_animations ?? [],
        random_min_sec: minSec,
        random_max_sec: maxSec,
        lines: modelStatus.lines ?? [],
      });
      await xiaohan.petRefreshAnimations();
    } catch (e) {
      setFeedback(parseApiError(e, "保存随机动作频率"));
    }
  };

  if (loading) {
    return (
      <>
        <SettingsToggle label="启用桌宠" checked disabled onChange={() => {}} />
        <SettingsToggle label="台词气泡" checked disabled onChange={() => {}} />
        <SettingsToggle label="始终置顶" checked disabled onChange={() => {}} />
        <p className="hint-block empty--compact">加载桌宠设置…</p>
      </>
    );
  }

  return (
    <>
      <SettingsToggle
        label="启用桌宠"
        checked={petEnabled}
        onChange={async (next) => {
          setPetEnabled(next);
          setFeedback(null);
          try {
            await xiaohan.petSetEnabled(next);
          } catch (e) {
            setPetEnabled(!next);
            setFeedback(parseApiError(e, "桌宠开关"));
          }
        }}
      />

      <SettingsToggle
        label="台词气泡"
        checked={bubbleEnabled}
        onChange={async (next) => {
          setBubbleEnabled(next);
          setFeedback(null);
          try {
            await xiaohan.petSetBubbleEnabled(next);
          } catch (e) {
            setBubbleEnabled(!next);
            setFeedback(parseApiError(e, "台词气泡开关"));
          }
        }}
      />

      <SettingsToggle
        label="始终置顶"
        checked={alwaysOnTop}
        onChange={async (next) => {
          setAlwaysOnTop(next);
          setFeedback(null);
          try {
            await xiaohan.petSetAlwaysOnTop(next);
          } catch (e) {
            setAlwaysOnTop(!next);
            setFeedback(parseApiError(e, "始终置顶开关"));
          }
        }}
      />
      <p className="hint-block">
        开启后桌宠始终浮于最前；关闭后全屏游戏、视频等前台应用会自然遮挡桌宠（桌宠仍保持运行）。
      </p>

      <div className="settings-field">
        <div className="settings-field-body">
          <div className="settings-field-label">角色大小</div>
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
                void saveGlobalScale(v);
              }, 350);
            }}
          />
          <span className="settings-unit">{Math.round(petScale * 100)}%</span>
        </div>
      </div>

      {activeModelId ? (
        <>
          <div className="settings-field">
            <div className="settings-field-body">
              <div className="settings-field-label">气泡频率</div>
            </div>
            <select
              className="settings-select"
              value={petRemarkInterval}
              onChange={async (e) => {
                const v = parseInt(e.target.value, 10);
                setPetRemarkInterval(v);
                const ok = await saveModelSettings({ remarkIntervalSec: v });
                if (!ok) await refresh();
              }}
            >
              <option value={0}>关闭</option>
              <option value={300}>每 5 分钟</option>
              <option value={600}>每 10 分钟</option>
              <option value={900}>每 15 分钟</option>
            </select>
          </div>

          <PetActionFrequency
            randomMinSec={randomMinSec}
            randomMaxSec={randomMaxSec}
            randomAnimations={randomAnimations}
            busy={false}
            onPatch={(patch) => {
              const nextMin = patch.randomMinSec ?? randomMinSec;
              const nextMax = patch.randomMaxSec ?? randomMaxSec;
              setRandomMinSec(nextMin);
              setRandomMaxSec(nextMax);
              if (freqSaveTimer.current) clearTimeout(freqSaveTimer.current);
              freqSaveTimer.current = setTimeout(() => {
                void saveRandomFrequency(nextMin, nextMax);
              }, 400);
            }}
          />
        </>
      ) : (
        <p className="hint-block">请先在人物页选用角色与皮肤，再调整模型相关显示选项。</p>
      )}
    </>
  );
}
