# 端云分离分支说明 (feat/client-server-split)

> 目标设计见 `main` 上 [questions/144-日报改造与三分支架构设计-20260709.md](questions/144-日报改造与三分支架构设计-20260709.md) 第三节。  
> 三分支总览见 [BRANCHES.md](BRANCHES.md)。

## 定位

- **本地采集端**：tracker + SQLite 缓冲 + 批量上报
- **服务器 Agent (2C2G)**：`analysis + report + ai + wechat`
- 微信应答由服务器常驻，用户无需开电脑

## 已落地

- `POST /api/ingest/segments`、`GET /api/report/today`（`agent_http.rs`）
- 已 merge main 日报改造

## 待办

- headless `xiaohan-agent` bin
- 本地上报客户端
- 设备 token 认证
