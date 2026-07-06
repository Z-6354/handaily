# 运行一段时间后桌宠碎块：禁止 hidden 后全量 reload

**日期**：2026-07-05  
**分类**：桌宠 / 稳定性

## 现象

启动后桌宠正常，运行一段时间（全屏切换、隐藏再显示、定时任务等）后出现 Spine 碎块。

## 根因

1. **每次 `show_pet` 都 `pet-reload` 全量重建** Spine（含全屏 `sync_fullscreen_visibility` 恢复路径）
2. **`visibilitychange` 可见时** 调用 `finalizeVisibleAssembly` + `configureAnimations`，与随机动作/全量 reload 竞态
3. **动作播放中 `assembleBaseIdle`** 强行拉骨架 → 永久碎块姿态

## 修复

### 双轨：reload vs resume

- `PetRuntimeState.spine_ready`：首启 `reloadPet` 成功后由 `pet_mark_spine_ready` 置位
- `show_pet`：已 ready 时发 `pet-resume`，不再 `pet-reload`
- `nudge_pet`（设置变更）仍全量 reload，并 `clear_spine_ready`

### 前端

- `pet-resume` → `resumePetFromHidden()`：仅恢复渲染 + 待机时 fit，不 dispose
- 移除 `visibilitychange` 里的 `loadConfig`/全量组装
- `assembleBaseIdle` / `finalizeVisibleAssembly`：动作播放中跳过

## 相关文件

- `src-tauri/src/pet/mod.rs`
- `src-tauri/src/ipc/commands.rs`
- `src/pet/main.ts`
- `src/pet/spinePet.ts`
