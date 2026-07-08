import { useCallback, useEffect, useState } from "react";
import { SettingsFeedbackBanner } from "../components/SettingsFeedbackBanner";
import { parseApiError, successFeedback, type SettingsFeedback } from "../lib/apiErrorMessage";
import { xiaohan, type AgentStatus } from "../lib/xiaohan";

const API_DOC = `# 小寒日报 Agent API

服务地址：http://127.0.0.1:1421

在设置 → Agent 接入中启用本地服务后，Cursor 等外部工具可通过 HTTP 调用。

## 端点

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | / | API 说明 |
| GET | /api/status | 服务状态 |
| GET | /api/personas | 人物/人设列表 |
| POST | /api/personas/{id}/regenerate | 单人 AI 更新性格（手动调用） |

## 响应格式

成功：业务 JSON 或数组
失败：{"code":1,"message":"错误说明"}

## Cursor 配置示例

在 Cursor MCP 或自定义脚本中请求：
curl http://127.0.0.1:1421/api/personas
curl -X POST http://127.0.0.1:1421/api/personas/cheshire/regenerate
`;

export function AgentConnectPage() {
  const [status, setStatus] = useState<AgentStatus | null>(null);
  const [toggling, setToggling] = useState(false);
  const [feedback, setFeedback] = useState<SettingsFeedback | null>(null);

  const loadStatus = useCallback(async () => {
    try {
      const s = await xiaohan.agentGetStatus();
      setStatus(s);
    } catch (e) {
      setFeedback(parseApiError(e, "读取 Agent 状态"));
    }
  }, []);

  useEffect(() => {
    void loadStatus();
  }, [loadStatus]);

  const toggle = async (enabled: boolean) => {
    setToggling(true);
    setFeedback(null);
    try {
      const s = await xiaohan.agentSetEnabled(enabled);
      setStatus(s);
      setFeedback(
        successFeedback(enabled ? "本地 Agent 服务已启动" : "本地 Agent 服务已停止")
      );
    } catch (e) {
      setFeedback(parseApiError(e, "切换 Agent 服务"));
    } finally {
      setToggling(false);
    }
  };

  const copy = async () => {
    try {
      await navigator.clipboard.writeText(API_DOC);
      setFeedback(successFeedback("已复制 Agent 接入说明"));
    } catch {
      /* ignore */
    }
  };

  const running = status?.running ?? false;
  const enabled = status?.enabled ?? false;

  return (
    <div className="page-stack agent-page">
      <div className="panel agent-status-panel">
        <div className="agent-status-head">
          <span className={`agent-status-dot${running ? " on" : " off"}`} />
          <div>
            <div className="panel-title">本地 HTTP 服务</div>
            <p className="panel-desc">
              {running
                ? `运行中 · ${status?.base_url ?? "http://127.0.0.1:1421"}`
                : "供 Cursor 等外部 Agent 更新人物性格资料"}
            </p>
          </div>
        </div>
        <label className="agent-toggle">
          <input
            type="checkbox"
            checked={enabled}
            disabled={toggling}
            onChange={(e) => void toggle(e.target.checked)}
          />
          <span>{toggling ? "切换中…" : "启用本地 Agent 服务"}</span>
        </label>
      </div>

      <div className="panel">
        <div className="panel-header">
          <div className="panel-title">Agent 接入说明</div>
          <button type="button" className="btn-primary btn-sm" onClick={copy}>
            一键复制
          </button>
        </div>
        <pre className="agent-code-block">{API_DOC}</pre>
      </div>

      <SettingsFeedbackBanner feedback={feedback} />
    </div>
  );
}
