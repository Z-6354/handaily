import { useCallback, useEffect, useRef, useState } from "react";
import { PageHeader } from "../components/PageHeader";
import { SettingsFeedbackToast } from "../components/SettingsFeedbackToast";
import { SettingsToggle } from "../components/SettingsToggle";
import { PetActionSettings, type PetActionLayout } from "../components/PetActionSettings";
import { PetActionFrequency } from "../components/PetActionFrequency";
import { PetModelImport } from "../components/PetModelImport";
import { PetModelPicker } from "../components/PetModelPicker";
import {
  parseApiError,
  successFeedback,
  type SettingsFeedback,
} from "../lib/apiErrorMessage";
import { xiaohan, type PetImportStagingPreview, type PetModelInfo } from "../lib/xiaohan";

type PetTab = "overview" | "actions" | "lines" | "lines-import" | "import";

const ACTION_TABS = new Set<PetTab>(["actions", "lines", "lines-import"]);

export function PetPanel() {
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [feedback, setFeedback] = useState<SettingsFeedback | null>(null);
  const [busy, setBusy] = useState(false);

  const [petEnabled, setPetEnabled] = useState(true);
  const [petScale, setPetScale] = useState(0.8);
  const [petRemarkInterval, setPetRemarkInterval] = useState(300);
  const [petModels, setPetModels] = useState<PetModelInfo[]>([]);
  const [petModelId, setPetModelId] = useState("chaijun");
  const [petAnimations, setPetAnimations] = useState<string[]>([]);
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
  const [importModelName, setImportModelName] = useState("");
  const [importStaging, setImportStaging] = useState<PetImportStagingPreview | null>(null);
  const [activeTab, setActiveTab] = useState<PetTab>("overview");
  const [switchingModelId, setSwitchingModelId] = useState<string | null>(null);
  const scaleSaveTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const petModelIdRef = useRef(petModelId);
  const patchActionLayoutRef = useRef<(partial: Partial<PetActionLayout>) => void>(() => {});

  useEffect(() => {
    petModelIdRef.current = petModelId;
  }, [petModelId]);

  useEffect(() => {
    return () => {
      if (scaleSaveTimer.current) clearTimeout(scaleSaveTimer.current);
    };
  }, []);

  const refreshStatus = useCallback(async () => {
    const status = await xiaohan.petGetStatus();
    setPetEnabled(status.enabled);
    setPetScale(status.scale);
    setPetRemarkInterval(status.remark_interval_sec);
    setPetModelId(status.model_id);
    setPetAnimations(status.animations ?? []);
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
    setPetModels(await xiaohan.petListModels());
  }, []);

  useEffect(() => {
    (async () => {
      try {
        setLoadError(null);
        await refreshStatus();
        try {
          setImportStaging(await xiaohan.petGetImportStaging());
        } catch {
          setImportStaging(null);
        }
      } catch (e) {
        setLoadError(String(e));
      } finally {
        setLoading(false);
      }
    })();
  }, [refreshStatus]);

  const saveModelSettings = async (settings: {
    scale?: number;
    remarkIntervalSec?: number;
  }): Promise<boolean> => {
    try {
      await xiaohan.petSaveModelSettings(petModelIdRef.current, settings);
      await xiaohan.petNudge();
      await refreshStatus();
      return true;
    } catch (e) {
      setFeedback(parseApiError(e, "保存桌宠设置"));
      return false;
    }
  };

  const switchModel = async (id: string) => {
    if (id === petModelId || switchingModelId) return;
    const targetName = petModels.find((m) => m.id === id)?.name ?? id;
    setSwitchingModelId(id);
    setFeedback({ tone: "loading", title: "切换模型", detail: "正在加载模型资源，请稍候…" });
    try {
      await xiaohan.petSetModel(id);
      await refreshStatus();
      setFeedback(successFeedback(`已切换至「${targetName}」`));
    } catch (err) {
      setFeedback(parseApiError(err, "切换模型"));
    } finally {
      setSwitchingModelId(null);
    }
  };

  const fileToBase64 = (file: File) =>
    new Promise<string>((resolve, reject) => {
      const reader = new FileReader();
      reader.onload = () => {
        const result = reader.result;
        if (typeof result !== "string") {
          reject(new Error("读取文件失败"));
          return;
        }
        const comma = result.indexOf(",");
        resolve(comma >= 0 ? result.slice(comma + 1) : result);
      };
      reader.onerror = () => reject(reader.error ?? new Error("读取文件失败"));
      reader.readAsDataURL(file);
    });

  const runPickFolder = async () => {
    setFeedback(null);
    try {
      const folder = await xiaohan.petPickModelFolder();
      if (!folder) return;
      setBusy(true);
      const preview = await xiaohan.petStageFolderImport(folder);
      setImportStaging(preview);
      setFeedback({
        tone: "success",
        title: "已缓存文件夹",
        detail: preview.config_generated
          ? "已检测到 Spine 三件套，并自动生成 config.json"
          : `已检测到 Spine 三件套，并缓存配置文件 ${preview.config_file ?? ""}`,
      });
    } catch (e) {
      setFeedback(parseApiError(e, "选择文件夹"));
    } finally {
      setBusy(false);
    }
  };

  const runStageFiles = async (files: File[]) => {
    if (files.length < 3) {
      setFeedback({
        tone: "error",
        title: "选择文件失败",
        detail: "请一次选择 .skel、.atlas、.png 三个文件",
      });
      return;
    }
    const skel = files.find((f) => f.name.toLowerCase().endsWith(".skel"));
    const atlas = files.find((f) => f.name.toLowerCase().endsWith(".atlas"));
    const png = files.find((f) => f.name.toLowerCase().endsWith(".png"));
    if (!skel || !atlas || !png) {
      setFeedback({
        tone: "error",
        title: "选择文件失败",
        detail: "缺少 .skel / .atlas / .png 之一",
      });
      return;
    }
    setBusy(true);
    setFeedback(null);
    try {
      const [skel_b64, atlas_b64, png_b64] = await Promise.all([
        fileToBase64(skel),
        fileToBase64(atlas),
        fileToBase64(png),
      ]);
      const preview = await xiaohan.petStageFilesImport({
        skel_b64,
        atlas_b64,
        png_b64,
        skel_name: skel.name,
        atlas_name: atlas.name,
        png_name: png.name,
      });
      setImportStaging(preview);
      setFeedback({
        tone: "success",
        title: "已缓存文件",
        detail: "三件套已写入本地缓存，点击「开始导入」完成导入",
      });
    } catch (e) {
      setFeedback(parseApiError(e, "缓存文件"));
    } finally {
      setBusy(false);
    }
  };

  const runCommitImport = async () => {
    const name = importModelName.trim();
    if (!name) {
      setFeedback({
        tone: "error",
        title: "无法导入",
        detail: "请先填写模型名称",
      });
      return;
    }
    if (!importStaging) {
      setFeedback({
        tone: "error",
        title: "无法导入",
        detail: "请先选择文件夹或文件并完成缓存",
      });
      return;
    }
    setBusy(true);
    setFeedback(null);
    try {
      const info = await xiaohan.petCommitImport(name);
      await refreshStatus();
      await xiaohan.petSetModel(info.id);
      const status = await xiaohan.petGetStatus();
      if (!status.enabled) {
        await xiaohan.petSetEnabled(true);
      }
      await refreshStatus();
      setImportModelName("");
      setImportStaging(null);
      setFeedback(successFeedback(`已导入「${info.name}」，已套用默认动作模板`));
    } catch (e) {
      setFeedback(parseApiError(e, "导入模型"));
    } finally {
      setBusy(false);
    }
  };

  const runClearStaging = async () => {
    setBusy(true);
    setFeedback(null);
    try {
      await xiaohan.petClearImportStaging();
      setImportStaging(null);
    } catch (e) {
      setFeedback(parseApiError(e, "清除缓存"));
    } finally {
      setBusy(false);
    }
  };

  const runDeleteModel = async () => {
    const model = petModels.find((m) => m.id === petModelId);
    if (!model || model.builtin) return;
    if (!window.confirm(`确定删除模型「${model.name}」？`)) return;
    setBusy(true);
    setFeedback(null);
    try {
      await xiaohan.petDeleteModel(petModelId);
      await refreshStatus();
      setFeedback(successFeedback("已删除模型"));
    } catch (e) {
      setFeedback(parseApiError(e, "删除模型"));
    } finally {
      setBusy(false);
    }
  };

  const currentModel = petModels.find((m) => m.id === petModelId);

  const modelPicker = (
    <PetModelPicker
      models={petModels}
      activeId={petModelId}
      switchingId={switchingModelId}
      disabled={loading || Boolean(loadError)}
      onSelect={(id) => void switchModel(id)}
      layout="compact"
    />
  );

  const overviewBadge = petEnabled ? `${Math.round(petScale * 100)}%` : "已关闭";

  const petTabs: { id: PetTab; label: string; badge?: string }[] = [
    { id: "overview", label: "概览", badge: overviewBadge },
    { id: "actions", label: "动作分配", badge: petAnimations.length ? String(petAnimations.length) : undefined },
    { id: "lines", label: "台词便签", badge: actionLayout.lines.length ? String(actionLayout.lines.length) : undefined },
    { id: "lines-import", label: "台词导入" },
    { id: "import", label: "导入模型", badge: importStaging ? "已缓存" : undefined },
  ];

  const actionSection: "actions" | "lines" | "lines-import" = ACTION_TABS.has(activeTab)
    ? (activeTab as "actions" | "lines" | "lines-import")
    : "actions";

  if (loading) {
    return (
      <>
        <PageHeader title="桌宠" actions={modelPicker} />
        <div className="panel">
          <p className="hint-block">加载桌宠设置…</p>
        </div>
      </>
    );
  }

  if (loadError) {
    return (
      <>
        <PageHeader title="桌宠" actions={modelPicker} />
        <div className="panel">
          <div className="error">加载失败：{loadError}</div>
        </div>
      </>
    );
  }

  return (
    <>
      <PageHeader title="桌宠" actions={modelPicker} />

      <SettingsFeedbackToast feedback={feedback} onDismiss={() => setFeedback(null)} />

      <div className="settings-stack">
        <div className="panel settings-card pet-panel-unified">
          <div className="pet-tab-bar" role="tablist" aria-label="桌宠设置">
            {petTabs.map((tab) => (
              <button
                key={tab.id}
                type="button"
                role="tab"
                aria-selected={activeTab === tab.id}
                className={`pet-tab${activeTab === tab.id ? " is-active" : ""}`}
                onClick={() => setActiveTab(tab.id)}
              >
                <span className="pet-tab-label">{tab.label}</span>
                {tab.badge && <span className="pet-tab-badge">{tab.badge}</span>}
              </button>
            ))}
          </div>

          <div className="pet-tab-panels">
            {activeTab === "overview" && (
              <div className="pet-tab-panel pet-overview-body" role="tabpanel">
                <PetModelPicker
                  models={petModels}
                  activeId={petModelId}
                  switchingId={switchingModelId}
                  disabled={Boolean(switchingModelId)}
                  onSelect={(id) => void switchModel(id)}
                  layout="grid"
                />

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
                    <div className="settings-field-hint">
                      也可在桌宠右键菜单「编辑范围」中用滚轮缩放
                    </div>
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
                    <div className="settings-field-hint">
                      已绑定 AI 时，定时气泡会从 AI 台词、工作时间线与台词库中随机抽取
                    </div>
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
                  busy={busy || Boolean(switchingModelId)}
                  onPatch={(patch) => patchActionLayoutRef.current(patch)}
                />
              </div>
            )}

            {activeTab === "import" && (
              <div className="pet-tab-panel" role="tabpanel">
                <PetModelImport
                  busy={busy}
                  importModelName={importModelName}
                  importStaging={importStaging}
                  petModelId={petModelId}
                  currentModel={currentModel}
                  onModelNameChange={setImportModelName}
                  onPickFolder={() => void runPickFolder()}
                  onStageFiles={(files) => void runStageFiles(files)}
                  onCommit={() => void runCommitImport()}
                  onClearStaging={() => void runClearStaging()}
                  onDeleteModel={() => void runDeleteModel()}
                />
              </div>
            )}

            <div
              className={ACTION_TABS.has(activeTab) ? "pet-tab-panel" : "pet-tab-panel-hidden"}
              role="tabpanel"
              hidden={!ACTION_TABS.has(activeTab)}
            >
              <PetActionSettings
                modelId={petModelId}
                animations={petAnimations}
                layout={actionLayout}
                busy={busy}
                section={actionSection}
                onLayoutChange={setActionLayout}
                onSaved={refreshStatus}
                setFeedback={setFeedback}
                setBusy={setBusy}
                onFocusImportTab={() => setActiveTab("lines-import")}
                onPatchReady={(patch) => {
                  patchActionLayoutRef.current = patch;
                }}
              />
            </div>
          </div>
        </div>
      </div>
    </>
  );
}
