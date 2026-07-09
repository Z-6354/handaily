# xiaohan-daily-wechat 模块规格

**module-id**: `xiaohan-daily-wechat`  
**状态**: WIP（核心绑定与定时推送已落地）  
**源码**: `src-tauri/src/wechat/`、`src/pages/WeChatBindPage.tsx`

## 职责

- 微信 **iLink ClawBot** 扫码绑定（独立 Bot 凭证，不依赖 HANAGENT / MCP）
- `context_token` 会话管理与待发队列
- 后台 `getupdates` 长轮询 + 定时推送调度
- 启动通知、整点上一小时小结、昨日日报

## 边界

| 允许 | 禁止 |
|------|------|
| iLink HTTP API（`ilinkai.weixin.qq.com`） | PC 微信 UI 自动化（`wechat-messenger` MCP 方式） |
| 小寒日报本地 Bot 凭证 | 与 HANAGENT 共用 Bot 轮询（会抢 `getupdates`） |
| 用户显式开启推送 | 启动时批量补发 24 小时历史小结 |

## 架构

```
WeChatBindPage（更多 → 微信绑定）
  → IPC wechat_start_qr / wechat_poll_qr
  → ilink.rs（get_bot_qrcode / get_qrcode_status）

scheduler.rs
  ├─ poll_loop：getupdates 长轮询，入站消息更新 context_token + flush_pending
  └─ scheduler_loop：启动消息 / 小时小结 / 日报

push.rs
  → send_text（Delivered | Queued | Skipped）
  → 定时推送 QueuePolicy::Deny（限流/未激活不入队）
  → 测试发送 QueuePolicy::Allow（可入待发队列）
```

## 数据目录

`%AppData%/xiaohan-daily/data/wechat/`

| 文件 | 说明 |
|------|------|
| `account.json` | `bot_token`、`account_id`、`user_id`、`source`（`qr`） |
| `session.json` | `owner_wx_user_id`、`last_context_token` |
| `pending.json` | 待发文本队列（最多 5 条，去重） |
| `sync-buf.txt` | getupdates 同步游标 |

## 设置键（`app_settings`）

| Key | 说明 |
|-----|------|
| `wechat_push_enabled` | `1` 开启推送，默认关 |
| `wechat_hour:YYYY-MM-DDTHH` | 按小时独立去重（每小时最多推 1 次） |
| `wechat_last_daily_report` | 昨日日报去重（`YYYY-MM-DD`） |
| `wechat_startup:YYYY-MM-DD` | 当日启动消息去重 |

## IPC 命令

| 命令 | 说明 |
|------|------|
| `wechat_get_status` | 绑定/通道/待发/提示文案 |
| `wechat_start_qr` | 获取绑定二维码 |
| `wechat_poll_qr` | 长轮询扫码状态（约 65s/次） |
| `wechat_prepare_rebind` | 清除本地凭证，准备重新扫码 |
| `wechat_logout` | 解绑并关闭推送 |
| `wechat_set_push_enabled` | 开关推送（开启时标记过去 24h 为已处理，避免补发洪峰） |
| `wechat_test_send` | 手动测试（可入待发队列） |

## 推送策略

| 类型 | 触发 | 去重 | 失败行为 |
|------|------|------|----------|
| 启动消息 | 调度器首次 tick | 每天 1 条 | 跳过，不入队 |
| 小时小结 | 整点后 0–9 分钟内，推**上一小时** | 按小时 key | 限流跳过，下轮重试 |
| 昨日日报 | 每天 00:00–00:03 | 按日期 key | 跳过，不入队 |
| 测试发送 | 用户点击 | 无 | 可入待发队列 |

**待发队列**：通道激活后每次 `flush_pending` 最多发 1 条；启动时若积压 >1 条会清除。

## 用户操作流程

1. **更多 → 微信绑定 → 开始绑定**，微信扫 ClawBot 二维码并确认
2. 在 ClawBot 中**发任意一条消息**，激活 `context_token`（推送通道 → 已激活）
3. 开启 **启用微信推送**，点 **测试发送** 验证
4. 若 Bot 已绑定其它 Agent：点 **重新扫码绑定**（勿使用外部导入）

## 已知限制

- `context_token` 会过期（`errcode:-14`），过期后需用户在 ClawBot 再发消息
- 微信 API 限流（`ret=-2`）：定时推送不入队；测试发送可排队稍后补发
- 需本机可访问 `https://ilinkai.weixin.qq.com`（支持系统代理）
- 扫码轮询为长连接，请等待约 1 分钟，勿重复点击

## 测试

```bash
cargo test --lib wechat --manifest-path src-tauri/Cargo.toml
```

## 未完成（v2）

- 报告生成后可选自动推送（`wechat_push_on_report`）
- 桌宠重要提醒同步微信
- QQ 绑定（架构可复用 iLink 模式）
