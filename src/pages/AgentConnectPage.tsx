export function AgentConnectPage() {
  const apiDoc = `# 小寒日报本地 API（即将推出）

服务地址：http://127.0.0.1:1421

使用前请先 GET / 获取最新 API 文档。

## 能力预览
- 查询今日工作概览
- 查询应用时长排行
- 查询时段热力图
- 查询 AI 时段总结

响应格式：{"code":0,"message":"success","data":...}`;

  const copy = async () => {
    try {
      await navigator.clipboard.writeText(apiDoc);
    } catch {
      /* ignore */
    }
  };

  return (
    <div className="page-stack agent-page">
      <div className="panel agent-status-panel">
        <div className="agent-status-head">
          <span className="agent-status-dot off" />
          <div>
            <div className="panel-title">本地 HTTP 服务</div>
            <p className="panel-desc">供外部 Agent 读取工作记录（开发中）</p>
          </div>
        </div>
        <label className="agent-toggle">
          <input type="checkbox" disabled />
          <span>启用本地服务</span>
        </label>
      </div>

      <div className="panel">
        <div className="panel-header">
          <div className="panel-title">Agent 接入配置文本</div>
          <button type="button" className="btn-primary btn-sm" onClick={copy}>
            一键复制
          </button>
        </div>
        <pre className="agent-code-block">{apiDoc}</pre>
      </div>
    </div>
  );
}
