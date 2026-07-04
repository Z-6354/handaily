# stats_today_overview not allowed / Command not found

## 问题

主界面加载失败：`stats_today_overview not allowed. Command not found`

## 根因

新增 `permissions/pet-ipc.toml` 后 Tauri 启用**应用级 ACL**（`has_app_acl = true`）。此后所有自定义 `#[tauri::command]` 必须在 capability 中显式授权。

`default` capability 仅有 `core:default`，未包含 `stats_today_overview` 等应用 command → invoke 被拒。

## 修复

- 新增 `permissions/app-commands.toml`：`allow-app-commands` 列出 `lib.rs` 中全部 invoke handler
- `capabilities/default.json` 增加 `allow-app-commands`（主窗口）
- `capabilities/pet.json` 改为引用同一权限（替换原 `allow-pet-ipc`）
- 删除仅含桌宠子集的 `pet-ipc.toml`

## 维护

新增 `#[tauri::command]` 时须同步写入 `permissions/app-commands.toml`。

## 相关文件

- `src-tauri/permissions/app-commands.toml`
- `src-tauri/capabilities/default.json`
- `src-tauri/capabilities/pet.json`
