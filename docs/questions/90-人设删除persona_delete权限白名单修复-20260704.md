# 90 · 人设删除 persona_delete 权限白名单修复

**日期**：2026-07-04  
**类型**：Bug 修复

## 现象

删除人设报错：`persona_delete not allowed. Command not found`

## 根因

Tauri 2 需在 `hanpet/src-tauri/permissions/app-commands.toml` 白名单注册 IPC。`persona_delete` 已在 `lib.rs` 注册但未加入 permissions。

## 修复

- `app-commands.toml` 增加 `"persona_delete"`
- 详情页补充删除结果反馈条；删除中禁用按钮

## 使用

**需重新编译/重启应用**（权限变更在 Rust 侧），然后 AI 人设 → 删除。

📁 已归档：`docs/questions/90-人设删除persona_delete权限白名单修复-20260704.md`
