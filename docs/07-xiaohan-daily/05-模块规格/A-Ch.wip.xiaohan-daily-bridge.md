# xiaohan-daily-bridge 模块规格

**module-id**: `xiaohan-daily-bridge`  
**状态**: WIP  
**源码**: 项目根目录 `./`

## 职责

- Win32 前台窗口采集（`tracker/win32.rs`、`tracker/idle.rs`）
- 后台 2s 采样循环与 segment 合并/切分（`tracker/poller.rs`）
- SQLite 持久化与延迟 flush（`tracker/writer.rs`、`db/mod.rs`）
- 今日聚合内存缓存（`db/stats.rs`）
- Tauri IPC 命令（`ipc/commands.rs`）

## 边界

| 允许 | 禁止 |
|------|------|
| Win32 调用仅限 `tracker/win32.rs`、`tracker/idle.rs` | 浏览器 URL、截图、云端上传 |
| DB 访问仅限 `db/` | 前端直接访问 SQLite |
| 单 Windows 平台 | 跨平台抽象（Phase 2） |

## 数据流

```
ForegroundPoller (2s)
  → idle 检测 (默认 90s)
  → segment 合并/切分
  → writer (延迟 flush + 60s checkpoint)
  → SQLite + TodayAggregator
  → #[tauri::command] (async 只读 / spawn_blocking 时间线)
  → React 前端
```

## 数据目录

`%AppData%/xiaohan-daily/data/xiaohan.sqlite`

## IPC 命令

| 命令 | 说明 |
|------|------|
| `app_ping` | 版本探测 |
| `app_get_data_path` | SQLite 路径 |
| `tracking_get_status` | 采集开关 + 前台快照 + open segment |
| `tracking_set_enabled` | 暂停/恢复 |
| `settings_get` / `settings_save` | 设置读写 |
| `stats_today_overview` | 今日概览（读缓存） |
| `stats_app_breakdown` | 应用排行 |
| `stats_hourly_activity` | 小时分布 |
| `stats_timeline` | 时间线分页（唯一走 DB 的查询） |

## 关联模块

- **微信绑定与推送**：见 [`B-Ch.wip.xiaohan-daily-wechat.md`](B-Ch.wip.xiaohan-daily-wechat.md)（iLink ClawBot，入口：更多 → 微信绑定）

## 退出与崩溃兜底

- 启动：闭合孤儿 segment/会话并**保留已计时长**（`recover_orphan_*` + `close_open_session`）
- 运行：60s checkpoint flush open segment
- 退出：`stop_flag` → join 后台线程 → flush + WAL checkpoint
