import { useEffect, useState } from "react";
import { SettingsFeedbackToast } from "../components/SettingsFeedbackToast";
import { SettingsSection } from "../components/SettingsSection";
import { SettingsToggle } from "../components/SettingsToggle";
import { PetDisplaySettings } from "../components/PetDisplaySettings";
import { WikiBulkImportSettings } from "../components/WikiBulkImportSettings";
import { useWikiBulkImportContext } from "../contexts/WikiBulkImportContext";
import { parseApiError, successFeedback, type SettingsFeedback } from "../lib/apiErrorMessage";
import { xiaohan } from "../lib/xiaohan";
import { isAutostartSupportedClient, isTauriRuntime } from "../lib/platform";

export function SettingsPanel() {
  const bulk = useWikiBulkImportContext();
  const [idleThreshold, setIdleThreshold] = useState(90);
  const [autostart, setAutostart] = useState(false);
  const [autostartSupported, setAutostartSupported] = useState(isAutostartSupportedClient);
  const [mcpApi, setMcpApi] = useState(false);
  const [dataPath, setDataPath] = useState("");
  const [saving, setSaving] = useState(false);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [saveError, setSaveError] = useState<SettingsFeedback | null>(null);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const [idle, autostartStatus, mcpStatus, path] = await Promise.all([
          xiaohan.getSetting("idle_threshold_secs"),
          xiaohan.autostartGetStatus().catch(() => null),
          xiaohan.mcpApiGetStatus().catch(() => null),
          xiaohan.getDataPath(),
        ]);
        if (cancelled) return;
        if (idle) setIdleThreshold(parseInt(idle, 10));
        if (mcpStatus) setMcpApi(mcpStatus.enabled);
        if (autostartStatus) {
          setAutostart(autostartStatus.enabled);
          setAutostartSupported(
            autostartStatus.supported || isAutostartSupportedClient(),
          );
        } else {
          setAutostart(false);
          setAutostartSupported(isAutostartSupportedClient());
        }
        setDataPath(path);
        setLoadError(null);
      } catch (e) {
        if (!cancelled) setLoadError(String(e));
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  const handleSaveIdle = async () => {
    setSaving(true);
    setSaveError(null);
    try {
      const secs = Math.max(30, Math.min(600, idleThreshold));
      await xiaohan.saveSetting("idle_threshold_secs", String(secs));
      setIdleThreshold(secs);
      setSaveError(successFeedback("空闲阈值已保存"));
    } catch (e) {
      setSaveError(parseApiError(e, "保存设置"));
    } finally {
      setSaving(false);
    }
  };

  const handleAutostart = async (enabled: boolean) => {
    setSaving(true);
    setSaveError(null);
    try {
      await xiaohan.autostartSetEnabled(enabled);
      setAutostart(enabled);
      setSaveError(successFeedback(enabled ? "已开启开机自启动" : "已关闭开机自启动"));
    } catch (e) {
      setSaveError(parseApiError(e, "自启动"));
    } finally {
      setSaving(false);
    }
  };

  const handleMcpApi = async (enabled: boolean) => {
    setSaving(true);
    setSaveError(null);
    try {
      await xiaohan.mcpApiSetEnabled(enabled);
      setMcpApi(enabled);
      setSaveError(
        successFeedback(
          enabled
            ? "已开启 Agent 控制接口，应用正在重启…"
            : "已关闭 Agent 控制接口，应用正在重启…",
        ),
      );
    } catch (e) {
      setSaveError(parseApiError(e, "Agent 控制接口"));
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="pref-shell">
      <div className="pref-shell__inner">
        {loadError && <div className="error">加载失败：{loadError}</div>}

        <SettingsSection flush title="桌宠显示">
          <PetDisplaySettings setFeedback={setSaveError} />
        </SettingsSection>

        <SettingsSection title="台词导入" accent>
          <WikiBulkImportSettings bulk={bulk} setFeedback={setSaveError} />
        </SettingsSection>

        <div className="pref-row">
          <SettingsSection flush title="桌宠行为">
            <div className="pref-inline">
              <div className="pref-inline__copy">
                <label className="pref-inline__label" htmlFor="idle-threshold">
                  空闲阈值
                </label>
              </div>
              <div className="settings-inline-input">
                <input
                  id="idle-threshold"
                  type="number"
                  min={30}
                  max={600}
                  className="settings-input settings-input--narrow"
                  value={idleThreshold}
                  disabled={saving}
                  onChange={(e) => setIdleThreshold(parseInt(e.target.value, 10) || 90)}
                />
                <span className="settings-unit">秒</span>
                <button
                  type="button"
                  className="btn-secondary btn-sm"
                  disabled={saving}
                  onClick={() => void handleSaveIdle()}
                >
                  保存
                </button>
              </div>
            </div>
          </SettingsSection>

          {isTauriRuntime() && (
            <SettingsSection flush title="启动">
              <SettingsToggle
                label="开机自启动"
                checked={autostart}
                disabled={saving || !autostartSupported}
                onChange={(v) => void handleAutostart(v)}
              />
              <SettingsToggle
                label="Agent 控制接口 (MCP)"
                checked={mcpApi}
                disabled={saving}
                onChange={(v) => void handleMcpApi(v)}
              />
              <p className="hint-block">
                开启后重启应用并在本机 127.0.0.1:19420 提供 HTTP 控制面，供 MCP / 自动化测试使用；默认关闭。
              </p>
            </SettingsSection>
          )}
        </div>

        <SettingsSection title="数据">
          <code className="pref-path">{dataPath || "—"}</code>
        </SettingsSection>
      </div>

      <SettingsFeedbackToast
        feedback={saveError}
        onDismiss={() => setSaveError(null)}
      />
    </div>
  );
}
