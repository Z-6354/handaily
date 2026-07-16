# hantransfer Agent API + MCP + AI 管理面板

> 状态：已批准（2026-07-16）  
> 范围：PC 桌面端 localhost API、Cursor MCP、管理页 AI 状态区

## 目标

让 Cursor Agent 能：

1. 查看已连接 / 待确认 / 已拒绝设备与收件队列  
2. 将本机文件推送到已信任手机  
3. 代为接受电脑侧待收文件  

管理页同步展示同一快照，便于人工与 Agent 共用。

## 方案

**Agent 聚合 API（localhost-only）+ 轻量 MCP + 管理页 AI 面板。**

## API

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/v1/agent/snapshot` | 信任/拒绝/pending、收件队列、outbox 摘要 |
| POST | `/api/v1/agent/push` | `{ "device_id", "paths": ["..."] }` 推送到手机 |
| POST | `/api/v1/agent/receive/accept` | `{ "id"? }` 缺省则 accept-all |

均要求 `ConnectInfo` 为 loopback，与现有 trust 管理一致。

## MCP

服务名：`hantransfer`（stdio 或脚本包装 `curl`/`fetch` 调 `127.0.0.1:7822`）。

工具：`snapshot`、`list_devices`、`push_files`、`accept_receive`。

## 管理页

- 顶部「AI 状态」卡片：设备数、待收、待确认  
- 可选只读 JSON（同源 snapshot）  
- 保留现有人工推送 / 信任 UI  

## 非目标

- 不改手机 App 协议  
- 不做公网鉴权（仅本机局域网既有模型）  
- 不实现远程任意路径读取沙箱以外目录（仅本机绝对路径，由桌面端读文件）
