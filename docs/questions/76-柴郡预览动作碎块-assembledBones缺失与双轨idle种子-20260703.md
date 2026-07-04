# 柴郡预览动作仍碎块：assembledBones 缺失 + 双轨 idle 种子

**日期**: 2026-07-03  
**主题**: 设置页动作列表预览 dance/touch 仍立即碎块；点 normal 无法恢复

## 现象

双轨 + 姿态守卫方案上线后用户仍反馈「依旧不行」：点击非待机动作预览时部件散落，点击 `normal` 也无法组装回来。

## 根因

1. **运行时致命错误（主因）**  
   重构 `spinePet.ts` 时误删了 `assembledBones` / `assembledSlotAttachmentNames` 两个 `Map` 字段声明，但 `snapshotAssembledPose()` 仍调用 `.clear()`。  
   首次预览动作时在 JS 层抛错，姿态快照与 `restoreAssembledPoseGuard()` 完全失效。

2. **track 1 从空轨硬切动作**  
   即便守卫可用，`setAnimation(ACTION_TRACK, dance)` 在空 track 上会从 bind/setup pose 开始 mix，柴郡 setup 本身就是散开的。

3. **`state.apply` hook 返回值**  
   pixi-spine 3.8 的 `apply` 签名返回 `boolean`，包装函数未 `return` 导致类型错误。

## 修复（`src/pet/spinePet.ts`）

- 恢复 `assembledBones`、`assembledSlotAttachmentNames` 字段
- 删除已无用的 `detectActionStartOffsets` / `actionStartOffsets`
- `patchSpineApply`：在 `origApply` 之后执行守卫并 `return applied`
- **`playOneShot` 双轨 idle 种子**：
  - track 0：循环 `normal` 待机
  - track 1：先 `setAnimation(idle)` 并同步 `trackTime` 与 track 0
  - 再 `addAnimation(ACTION_TRACK, dance, false, 0)` + `holdPrevious` + mix
  - 动作结束后 `clearTrack(1)`，不替换 track 0
- 预览待机：`assembleBaseIdle()` 清 track 1 + pump 组装 + 重新 snapshot

## 验证步骤

1. 重启 dev（或设置页「立即更新」）确保 pet 窗口加载新 bundle
2. 加载柴郡 → 应站立（normal）
3. 设置页预览 `dance` / `touch` → 不应碎块，动作可播
4. 预览 `normal` → 恢复待机循环
5. 点击桌宠 → touch；随机 → dance/sleep

## 相关

- doc 74：setup pose 散开、idle 误选 dance
- doc 75：动作表与 IPC 预览
