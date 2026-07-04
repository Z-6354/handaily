import { useCallback, useEffect, useState } from "react";
import { AiModelModal } from "../components/AiModelModal";
import { SettingsFeedbackBanner } from "../components/SettingsFeedbackBanner";
import { SettingsSection } from "../components/SettingsSection";
import { SettingsToggle } from "../components/SettingsToggle";
import {
  loadingFeedback,
  parseApiError,
  parseTestSuccess,
  successFeedback,
  type SettingsFeedback,
} from "../lib/apiErrorMessage";
import {
  xiaohan,
  normalizeAiConfig,
  type AiConfig,
  type AiModelOption,
  type VaultEntry,
  type WorkTypeConfig,
} from "../lib/xiaohan";

interface Props {
  onTrackingChange?: (enabled: boolean) => void;
}

type ModelKind = "text" | "vision" | "thinking";
type SettingsTab = "general" | "ai" | "worktypes" | "analysis";

const TABS: { id: SettingsTab; label: string }[] = [
  { id: "general", label: "通用" },
  { id: "ai", label: "AI 配置" },
  { id: "worktypes", label: "工作类型" },
  { id: "analysis", label: "智能分析" },
];

export function SettingsPanel({ onTrackingChange }: Props) {
  const [tab, setTab] = useState<SettingsTab>("general");
  const [idleThreshold, setIdleThreshold] = useState(90);
  const [tracking, setTracking] = useState(true);
  const [autostart, setAutostart] = useState(false);
  const [autostartSupported, setAutostartSupported] = useState(true);
  const [dataPath, setDataPath] = useState("");
  const [promptsPath, setPromptsPath] = useState("");
  const [vendorsPath, setVendorsPath] = useState("");
  const [saving, setSaving] = useState(false);

  const [hybridEnabled, setHybridEnabled] = useState(true);
  const [screenshotEnabled, setScreenshotEnabled] = useState(true);
  const [cpuThreshold, setCpuThreshold] = useState(75);
  const [screenshotInterval, setScreenshotInterval] = useState(120);
  const [visionEnabled, setVisionEnabled] = useState(false);
  const [analysisStats, setAnalysisStats] = useState("");

  const [aiConfig, setAiConfig] = useState<AiConfig | null>(null);
  const [vaultEntries, setVaultEntries] = useState<VaultEntry[]>([]);
  const [vaultUnlocked, setVaultUnlocked] = useState(false);
  const [textModels, setTextModels] = useState<AiModelOption[]>([]);
  const [visionModels, setVisionModels] = useState<AiModelOption[]>([]);
  const [thinkingModels, setThinkingModels] = useState<AiModelOption[]>([]);
  const [importFeedback, setImportFeedback] = useState<
    Partial<Record<ModelKind, SettingsFeedback>>
  >({});
  const [vendorFeedback, setVendorFeedback] = useState<
    Record<string, SettingsFeedback>
  >({});
  const [testingVendor, setTestingVendor] = useState<string | null>(null);
  const [customModal, setCustomModal] = useState<ModelKind | null>(null);
  const [workTypeConfig, setWorkTypeConfig] = useState<WorkTypeConfig | null>(null);
  const [newWtName, setNewWtName] = useState("");
  const [newWtColor, setNewWtColor] = useState("#722ed1");
  const [initLoading, setInitLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [saveError, setSaveError] = useState<SettingsFeedback | null>(null);

  const loadModels = useCallback(async (cfg: AiConfig) => {
    const [t, v, th] = await Promise.all([
      xiaohan.aiListModels(cfg.text_vendor_id, "text"),
      xiaohan.aiListModels(cfg.vision_vendor_id, "vision"),
      xiaohan.aiListModels(cfg.thinking_vendor_id || cfg.text_vendor_id, "thinking"),
    ]);
    setTextModels(t);
    setVisionModels(v);
    setThinkingModels(th);
  }, []);

  useEffect(() => {
    (async () => {
      try {
        setLoadError(null);
        const idle = await xiaohan.getSetting("idle_threshold_secs");
        if (idle) setIdleThreshold(parseInt(idle, 10));
        const status = await xiaohan.getStatus();
        setTracking(status.tracking);
        try {
          const autostartStatus = await xiaohan.autostartGetStatus();
          setAutostart(autostartStatus.enabled);
          setAutostartSupported(autostartStatus.supported);
        } catch {
          setAutostart(false);
          setAutostartSupported(false);
        }
        setDataPath(await xiaohan.getDataPath());
        try {
          setPromptsPath(await xiaohan.getPromptsPath());
        } catch {
          setPromptsPath("");
        }
        try {
          setVendorsPath(await xiaohan.getVendorsConfigPath());
        } catch {
          setVendorsPath("");
        }

        const hybrid = await xiaohan.getSetting("analysis_hybrid_enabled");
        setHybridEnabled(hybrid !== "0");
        const ss = await xiaohan.getSetting("analysis_screenshot_enabled");
        setScreenshotEnabled(ss !== "0");
        const cpu = await xiaohan.getSetting("analysis_cpu_threshold_percent");
        if (cpu) setCpuThreshold(parseInt(cpu, 10));
        const interval = await xiaohan.getSetting("analysis_screenshot_min_interval_secs");
        if (interval) setScreenshotInterval(parseInt(interval, 10));
        const vision = await xiaohan.getSetting("analysis_vision_enabled");
        setVisionEnabled(vision === "1");

        try {
          const stats = await xiaohan.analysisGetStatus();
          setAnalysisStats(
            `文本 ${stats.text_count} · 截图 ${stats.screenshot_count} · 跳过 ${stats.skipped_screenshot_count} · CPU ${stats.system_cpu_percent.toFixed(0)}%`,
          );
        } catch {
          setAnalysisStats("");
        }

        const cfg = normalizeAiConfig(await xiaohan.aiGetConfig());
        setAiConfig(cfg);
        await loadModels(cfg);
        setWorkTypeConfig(await xiaohan.workTypesGet());

        const vs = await xiaohan.vaultGetStatus();
        setVaultUnlocked(vs.unlocked);
        if (vs.unlocked) {
          setVaultEntries(await xiaohan.vaultList());
        }
      } catch (e) {
        setLoadError(String(e));
      } finally {
        setInitLoading(false);
      }
    })();
  }, [loadModels]);

  const saveSetting = async (key: string, value: string) => {
    setSaving(true);
    setSaveError(null);
    try {
      await xiaohan.saveSetting(key, value);
    } catch (e) {
      setSaveError(parseApiError(e, "保存设置"));
    } finally {
      setSaving(false);
    }
  };

  const saveAi = async (next: AiConfig) => {
    if (!aiConfig) return;
    const payload = normalizeAiConfig({ ...aiConfig, ...next });
    setSaving(true);
    setSaveError(null);
    try {
      await xiaohan.aiSaveConfig(payload);
      setAiConfig(payload);
      await loadModels(payload);
    } catch (e) {
      setSaveError(parseApiError(e, "保存 AI 配置"));
    } finally {
      setSaving(false);
    }
  };

  const updateVendor = async (vendorId: string, patch: Partial<AiConfig["vendors"][0]>) => {
    if (!aiConfig) return;
    const vendors = aiConfig.vendors.map((v) =>
      v.id === vendorId ? { ...v, ...patch } : v,
    );
    await saveAi({ ...aiConfig, vendors });
  };

  const testVendor = async (vendorId: string) => {
    setTestingVendor(vendorId);
    setVendorFeedback((prev) => ({
      ...prev,
      [vendorId]: loadingFeedback("正在测试连接…"),
    }));
    try {
      const result = await xiaohan.aiTestVendor(vendorId);
      setVendorFeedback((prev) => ({
        ...prev,
        [vendorId]: result.ok
          ? parseTestSuccess(result.message, result.imported_text, result.imported_vision)
          : parseApiError(result.message, "连接测试"),
      }));
      if (result.ok && aiConfig) {
        const cfg = normalizeAiConfig(await xiaohan.aiGetConfig());
        setAiConfig(cfg);
        await loadModels(cfg);
      }
    } catch (e) {
      setVendorFeedback((prev) => ({
        ...prev,
        [vendorId]: parseApiError(e, "连接测试"),
      }));
    } finally {
      setTestingVendor(null);
    }
  };

  const importModels = async (kind: ModelKind) => {
    if (!aiConfig) return;
    const vendorId =
      kind === "text"
        ? aiConfig.text_vendor_id
        : kind === "vision"
          ? aiConfig.vision_vendor_id
          : aiConfig.thinking_vendor_id || aiConfig.text_vendor_id;
    const label =
      kind === "text" ? "文本模型" : kind === "vision" ? "多模态模型" : "思考模型";
    setImportFeedback((prev) => ({
      ...prev,
      [kind]: loadingFeedback(`正在导入${label}…`),
    }));
    try {
      await xiaohan.aiImportModels(vendorId, kind);
      let cfg = normalizeAiConfig(await xiaohan.aiGetConfig());
      cfg = await syncModelSelection(cfg, kind, vendorId);
      setAiConfig(cfg);
      await loadModels(cfg);
      setImportFeedback((prev) => ({
        ...prev,
        [kind]: successFeedback(`${label}导入成功`, "已从 API 拉取并保存到本地，可在上方下拉框中选择。"),
      }));
    } catch (e) {
      setImportFeedback((prev) => ({
        ...prev,
        [kind]: parseApiError(e, `${label}导入`),
      }));
    }
  };

  const syncModelSelection = async (
    cfg: AiConfig,
    kind: ModelKind,
    vendorId: string,
  ): Promise<AiConfig> => {
    const models = await xiaohan.aiListModels(vendorId, kind);
    const field =
      kind === "text" ? "text_model" : kind === "vision" ? "vision_model" : "thinking_model";
    const current = cfg[field];
    const next = models.some((m) => m.id === current)
      ? current
      : models[0]?.id ?? "";
    if (next === current) return cfg;
    const updated = { ...cfg, [field]: next };
    await xiaohan.aiSaveConfig(updated);
    return updated;
  };

  const addCustomModel = async (kind: ModelKind, id: string, name: string) => {
    if (!aiConfig) return;
    const vendorId =
      kind === "text"
        ? aiConfig.text_vendor_id
        : kind === "vision"
          ? aiConfig.vision_vendor_id
          : aiConfig.thinking_vendor_id || aiConfig.text_vendor_id;
    await xiaohan.aiAddCustomModel(vendorId, kind, id, name);
    const cfg = normalizeAiConfig(await xiaohan.aiGetConfig());
    setAiConfig(cfg);
    await loadModels(cfg);
  };

  const saveWorkTypes = async (next: WorkTypeConfig, prev: WorkTypeConfig) => {
    setWorkTypeConfig(next);
    setSaveError(null);
    try {
      await xiaohan.workTypesSave(next);
    } catch (e) {
      setWorkTypeConfig(prev);
      setSaveError(parseApiError(e, "保存工作类型"));
    }
  };

  if (initLoading) {
    return <div className="settings-layout settings-layout--loading">加载设置…</div>;
  }

  if (loadError || !aiConfig) {
    return (
      <div className="settings-layout settings-layout--loading">
        <div className="error">加载设置失败：{loadError ?? "未知错误"}</div>
      </div>
    );
  }

  return (
    <div className="settings-layout">
      <nav className="settings-nav">
        {TABS.map((t) => (
          <button
            key={t.id}
            type="button"
            className={`settings-nav-item${tab === t.id ? " active" : ""}`}
            onClick={() => setTab(t.id)}
          >
            {t.label}
          </button>
        ))}
        {saving && <span className="settings-saving-badge">保存中</span>}
      </nav>

      <div className="settings-main">
        {saveError && <SettingsFeedbackBanner feedback={saveError} />}
        {tab === "general" && (
          <div className="panel settings-card">
            <SettingsSection title="采集与存储" description="控制后台记录行为与数据存放位置">
              <SettingsToggle
                label="后台采集"
                hint="关闭后暂停记录前台应用与输入活动"
                checked={tracking}
                onChange={async (next) => {
                  const prev = tracking;
                  setTracking(next);
                  onTrackingChange?.(next);
                  try {
                    await xiaohan.setEnabled(next);
                  } catch (e) {
                    setTracking(prev);
                    onTrackingChange?.(prev);
                    setSaveError(parseApiError(e, "切换采集状态"));
                  }
                }}
              />
              <div className="settings-field">
                <div className="settings-field-body">
                  <div className="settings-field-label">空闲阈值</div>
                  <div className="settings-field-hint">超过此时间无键鼠操作则记为空闲</div>
                </div>
                <div className="settings-inline-input">
                  <input
                    type="number"
                    min={10}
                    max={600}
                    value={idleThreshold}
                    onChange={(e) => {
                      const v = parseInt(e.target.value, 10) || 90;
                      setIdleThreshold(v);
                      saveSetting("idle_threshold_secs", String(v));
                    }}
                  />
                  <span className="settings-unit">秒</span>
                </div>
              </div>
              <div className="settings-field settings-field--stack">
                <div className="settings-field-label">数据目录</div>
                <code className="settings-path">{dataPath || "加载中…"}</code>
              </div>
            </SettingsSection>
          </div>
        )}

        {tab === "general" && (
          <div className="panel settings-card">
            <SettingsSection
              title="启动"
              description="控制应用是否随 Windows 登录自动运行"
            >
              <SettingsToggle
                label="开机自启动"
                hint={
                  autostartSupported
                    ? "开启后登录 Windows 时自动启动小寒日报（托盘常驻）"
                    : "当前系统暂不支持开机自启动"
                }
                checked={autostart}
                disabled={!autostartSupported}
                onChange={async (next) => {
                  const prev = autostart;
                  setAutostart(next);
                  setSaveError(null);
                  try {
                    await xiaohan.autostartSetEnabled(next);
                  } catch (e) {
                    setAutostart(prev);
                    setSaveError(parseApiError(e, "开机自启动"));
                  }
                }}
              />
            </SettingsSection>
          </div>
        )}

        {tab === "ai" && (
          <div className="settings-stack">
            <div className="panel settings-card">
              <SettingsSection
                title="供应商 API 密钥"
                description={
                  vaultUnlocked
                    ? "在密码本保存密钥后在此关联；Ollama 本地无需密钥"
                    : "请先在密码本解锁，再选择各供应商密钥"
                }
              >
                <div className="vendor-cards">
                  {aiConfig.vendors.map((v) => (
                    <div className="vendor-card" key={v.id}>
                      <div className="vendor-card-head">
                        <div>
                          <div className="vendor-card-name">{v.name}</div>
                          <div className="vendor-card-url">{v.base_url}</div>
                        </div>
                        {v.api_style === "ollama" && (
                          <span className="vendor-badge">本地</span>
                        )}
                      </div>
                      <div className="vendor-card-actions">
                        <button
                          type="button"
                          className="btn-secondary btn-sm"
                          disabled={
                            v.api_style !== "ollama" &&
                            (!vaultUnlocked || testingVendor === v.id)
                          }
                          onClick={() => testVendor(v.id)}
                        >
                          {testingVendor === v.id ? "测试中…" : "测试"}
                        </button>
                        {v.api_style === "ollama" ? (
                          <span className="hint">无需密钥</span>
                        ) : (
                          <select
                            value={v.vault_entry_id ?? ""}
                            disabled={!vaultUnlocked}
                            onChange={(e) =>
                              updateVendor(v.id, {
                                vault_entry_id: e.target.value
                                  ? parseInt(e.target.value, 10)
                                  : null,
                              })
                            }
                          >
                            <option value="">未配置密钥</option>
                            {vaultEntries.map((ent) => (
                              <option key={ent.id} value={ent.id}>
                                {ent.name}
                              </option>
                            ))}
                          </select>
                        )}
                      </div>
                      <SettingsFeedbackBanner feedback={vendorFeedback[v.id]} compact />
                    </div>
                  ))}
                </div>
              </SettingsSection>
            </div>

            <div className="panel settings-card">
              <SettingsSection
                title="AI 模型"
                description="模型需通过「导入模型」或「手动添加」配置，均会保存到本地"
              >
                <div className="model-pick-grid">
                  <div className="model-pick-block">
                    <div className="model-pick-title">文本模型</div>
                    <label className="model-pick-label">供应商</label>
                    <select
                      value={aiConfig.text_vendor_id}
                      onChange={async (e) => {
                        const vendorId = e.target.value;
                        let next = { ...aiConfig, text_vendor_id: vendorId };
                        next = await syncModelSelection(next, "text", vendorId);
                        await saveAi(next);
                      }}
                    >
                      {aiConfig.vendors.map((v) => (
                        <option key={v.id} value={v.id}>
                          {v.name}
                        </option>
                      ))}
                    </select>
                    <label className="model-pick-label">模型</label>
                    <select
                      value={aiConfig.text_model || ""}
                      onChange={async (e) => saveAi({ ...aiConfig, text_model: e.target.value })}
                      disabled={textModels.length === 0}
                    >
                      {textModels.length === 0 ? (
                        <option value="">请先导入或手动添加</option>
                      ) : (
                        textModels.map((m) => (
                          <option key={m.id} value={m.id}>
                            {m.name}
                            {m.custom ? " (自定义)" : m.name === m.id ? " (默认)" : " (已导入)"}
                          </option>
                        ))
                      )}
                    </select>
                    <div className="model-pick-actions">
                      <button className="btn-secondary btn-sm" onClick={() => importModels("text")}>
                        导入模型
                      </button>
                      <button
                        className="btn-secondary btn-sm model-pick-add-btn"
                        onClick={() => {
                          setImportFeedback((prev) => ({ ...prev, text: undefined }));
                          setCustomModal("text");
                        }}
                      >
                        手动添加
                      </button>
                    </div>
                    <SettingsFeedbackBanner feedback={importFeedback.text} compact />
                  </div>

                  <div className="model-pick-block">
                    <div className="model-pick-title">多模态模型</div>
                    <label className="model-pick-label">供应商</label>
                    <select
                      value={aiConfig.vision_vendor_id}
                      onChange={async (e) => {
                        const vendorId = e.target.value;
                        let next = { ...aiConfig, vision_vendor_id: vendorId };
                        next = await syncModelSelection(next, "vision", vendorId);
                        await saveAi(next);
                      }}
                    >
                      {aiConfig.vendors.map((v) => (
                        <option key={v.id} value={v.id}>
                          {v.name}
                        </option>
                      ))}
                    </select>
                    <label className="model-pick-label">模型</label>
                    <select
                      value={aiConfig.vision_model || ""}
                      onChange={async (e) => saveAi({ ...aiConfig, vision_model: e.target.value })}
                      disabled={visionModels.length === 0}
                    >
                      {visionModels.length === 0 ? (
                        <option value="">请先导入或手动添加</option>
                      ) : (
                        visionModels.map((m) => (
                          <option key={m.id} value={m.id}>
                            {m.name}
                            {m.custom ? " (自定义)" : m.name === m.id ? " (默认)" : " (已导入)"}
                          </option>
                        ))
                      )}
                    </select>
                    <div className="model-pick-actions">
                      <button
                        className="btn-secondary btn-sm"
                        onClick={() => importModels("vision")}
                      >
                        导入模型
                      </button>
                      <button
                        className="btn-secondary btn-sm model-pick-add-btn"
                        onClick={() => {
                          setImportFeedback((prev) => ({ ...prev, vision: undefined }));
                          setCustomModal("vision");
                        }}
                      >
                        手动添加
                      </button>
                    </div>
                    <SettingsFeedbackBanner feedback={importFeedback.vision} compact />
                  </div>

                  <div className="model-pick-block">
                    <div className="model-pick-title">思考模型</div>
                    <p className="settings-field-hint" style={{ marginBottom: 8 }}>
                      用于人设工坊等结构化文本任务；未配置时回退到文本模型
                    </p>
                    <label className="model-pick-label">供应商</label>
                    <select
                      value={aiConfig.thinking_vendor_id || aiConfig.text_vendor_id}
                      onChange={async (e) => {
                        const vendorId = e.target.value;
                        let next = { ...aiConfig, thinking_vendor_id: vendorId };
                        next = await syncModelSelection(next, "thinking", vendorId);
                        await saveAi(next);
                      }}
                    >
                      {aiConfig.vendors.map((v) => (
                        <option key={v.id} value={v.id}>
                          {v.name}
                        </option>
                      ))}
                    </select>
                    <label className="model-pick-label">模型</label>
                    <select
                      value={aiConfig.thinking_model || ""}
                      onChange={async (e) => saveAi({ ...aiConfig, thinking_model: e.target.value })}
                      disabled={thinkingModels.length === 0}
                    >
                      {thinkingModels.length === 0 ? (
                        <option value="">请先导入或手动添加</option>
                      ) : (
                        thinkingModels.map((m) => (
                          <option key={m.id} value={m.id}>
                            {m.name}
                            {m.custom ? " (自定义)" : m.name === m.id ? " (默认)" : " (已导入)"}
                          </option>
                        ))
                      )}
                    </select>
                    <div className="model-pick-actions">
                      <button
                        className="btn-secondary btn-sm"
                        onClick={() => importModels("thinking")}
                      >
                        导入模型
                      </button>
                      <button
                        className="btn-secondary btn-sm model-pick-add-btn"
                        onClick={() => {
                          setImportFeedback((prev) => ({ ...prev, thinking: undefined }));
                          setCustomModal("thinking");
                        }}
                      >
                        手动添加
                      </button>
                    </div>
                    <SettingsFeedbackBanner feedback={importFeedback.thinking} compact />
                  </div>
                </div>
              </SettingsSection>
            </div>

            {vendorsPath && (
              <div className="panel settings-card settings-card--muted">
                <SettingsSection
                  title="供应商目录"
                  description="供应商定义来自 JSON 配置，修改后重启应用生效；用户选择与密钥仍保存在本地数据库"
                >
                  <code className="settings-path">{vendorsPath}</code>
                  <p className="settings-field-hint">
                    编辑 <code>vendors.json</code> 可增删供应商或调整 Base URL、适配器类型（
                    <code>ollama</code> / <code>openai</code>），无需改代码。删除该文件后重启可恢复内置默认。
                  </p>
                </SettingsSection>
              </div>
            )}

            {promptsPath && (
              <div className="panel settings-card settings-card--muted">
                <SettingsSection
                  title="提示词模板"
                  description="AI 提示词以 Markdown 存放，修改后下次分析即生效"
                >
                  <code className="settings-path">{promptsPath}</code>
                  <p className="settings-field-hint">
                    编辑该目录下的 <code>vision-screenshot.md</code>、
                    <code>period-analysis.md</code> 等文件。删除文件后重启可恢复默认。
                  </p>
                </SettingsSection>
              </div>
            )}
          </div>
        )}

        {tab === "worktypes" && workTypeConfig && (
          <div className="panel settings-card">
            <SettingsSection
              title="工作类型"
              description="AI 时段分析将从以下类型中选择；内置类型不可删除"
            >
              <div className="work-type-list">
                {workTypeConfig.types.map((t, i) => (
                  <div className="work-type-chip-row" key={t.id}>
                    <input
                      className="work-type-name"
                      value={t.name}
                      disabled={t.builtin}
                      onChange={async (e) => {
                        const types = [...workTypeConfig.types];
                        types[i] = { ...t, name: e.target.value };
                        await saveWorkTypes({ types }, workTypeConfig);
                      }}
                    />
                    <input
                      type="color"
                      className="work-type-color"
                      value={t.color}
                      onChange={async (e) => {
                        const types = [...workTypeConfig.types];
                        types[i] = { ...t, color: e.target.value };
                        await saveWorkTypes({ types }, workTypeConfig);
                      }}
                    />
                    <span className={`work-type-tag${t.builtin ? "" : " custom"}`}>
                      {t.builtin ? "内置" : "自定义"}
                    </span>
                    {!t.builtin && (
                      <button
                        type="button"
                        className="btn-secondary btn-sm"
                        onClick={async () => {
                          const next = {
                            types: workTypeConfig.types.filter((x) => x.id !== t.id),
                          };
                          await saveWorkTypes(next, workTypeConfig);
                        }}
                      >
                        删除
                      </button>
                    )}
                  </div>
                ))}
              </div>
              <div className="work-type-add">
                <input
                  placeholder="新类型名称"
                  value={newWtName}
                  onChange={(e) => setNewWtName(e.target.value)}
                />
                <input
                  type="color"
                  value={newWtColor}
                  onChange={(e) => setNewWtColor(e.target.value)}
                />
                <button
                  type="button"
                  className="btn-secondary btn-sm"
                  onClick={async () => {
                    if (!newWtName.trim()) return;
                    const id = `custom_${Date.now()}`;
                    const next = {
                      types: [
                        ...workTypeConfig.types,
                        { id, name: newWtName.trim(), color: newWtColor, builtin: false },
                      ],
                    };
                    await saveWorkTypes(next, workTypeConfig);
                    setNewWtName("");
                  }}
                >
                  添加
                </button>
              </div>
            </SettingsSection>
          </div>
        )}

        {tab === "analysis" && (
          <div className="panel settings-card">
            <SettingsSection
              title="智能分析"
              description="默认读窗口标题和应用名；信息不够时再瞄一眼截图"
            >
              <SettingsToggle
                label="智能分析"
                checked={hybridEnabled}
                onChange={async (next) => {
                  setHybridEnabled(next);
                  await saveSetting("analysis_hybrid_enabled", next ? "1" : "0");
                }}
              />
              <SettingsToggle
                label="自动截图"
                checked={screenshotEnabled}
                onChange={async (next) => {
                  setScreenshotEnabled(next);
                  await saveSetting("analysis_screenshot_enabled", next ? "1" : "0");
                }}
              />
              <SettingsToggle
                label="视觉 AI"
                hint="使用 AI 配置中的多模态模型"
                checked={visionEnabled}
                onChange={async (next) => {
                  setVisionEnabled(next);
                  await saveSetting("analysis_vision_enabled", next ? "1" : "0");
                }}
              />
              <div className="settings-field">
                <div className="settings-field-body">
                  <div className="settings-field-label">CPU 阈值</div>
                  <div className="settings-field-hint">超过此占用时不截图</div>
                </div>
                <div className="settings-inline-input">
                  <input
                    type="number"
                    min={40}
                    max={95}
                    value={cpuThreshold}
                    onChange={async (e) => {
                      const v = parseInt(e.target.value, 10) || 75;
                      setCpuThreshold(v);
                      await saveSetting("analysis_cpu_threshold_percent", String(v));
                    }}
                  />
                  <span className="settings-unit">%</span>
                </div>
              </div>
              <div className="settings-field">
                <div className="settings-field-body">
                  <div className="settings-field-label">截图间隔</div>
                  <div className="settings-field-hint">两次截图之间的最短间隔</div>
                </div>
                <div className="settings-inline-input">
                  <input
                    type="number"
                    min={30}
                    max={600}
                    value={screenshotInterval}
                    onChange={async (e) => {
                      const v = parseInt(e.target.value, 10) || 120;
                      setScreenshotInterval(v);
                      await saveSetting("analysis_screenshot_min_interval_secs", String(v));
                    }}
                  />
                  <span className="settings-unit">秒</span>
                </div>
              </div>
              {analysisStats && (
                <div className="settings-stats-bar">
                  <span className="settings-stats-label">今日</span>
                  {analysisStats}
                </div>
              )}
            </SettingsSection>
          </div>
        )}
      </div>

      <AiModelModal
        open={customModal !== null}
        kind={customModal}
        vendorId={
          customModal === "text"
            ? aiConfig.text_vendor_id
            : customModal === "vision"
              ? aiConfig.vision_vendor_id
              : aiConfig.thinking_vendor_id || aiConfig.text_vendor_id
        }
        vendorName={
          aiConfig.vendors.find(
            (v) =>
              v.id ===
              (customModal === "text"
                ? aiConfig.text_vendor_id
                : customModal === "vision"
                  ? aiConfig.vision_vendor_id
                  : aiConfig.thinking_vendor_id || aiConfig.text_vendor_id),
          )?.name ?? "当前供应商"
        }
        existingCustom={aiConfig.custom_models}
        feedback={customModal ? importFeedback[customModal] ?? null : null}
        onClose={() => setCustomModal(null)}
        onSubmit={async (id, name) => {
          if (!customModal) return;
          try {
            await addCustomModel(customModal, id, name);
            setImportFeedback((prev) => ({
              ...prev,
              [customModal]: successFeedback(`已添加模型「${name || id}」`),
            }));
          } catch (e) {
            setImportFeedback((prev) => ({
              ...prev,
              [customModal]: parseApiError(e, "添加模型"),
            }));
            throw e;
          }
        }}
      />
    </div>
  );
}
