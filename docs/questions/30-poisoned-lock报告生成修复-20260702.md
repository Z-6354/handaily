# 30 · poisoned lock 导致报告生成失败

**日期**：2026-07-02  
**类型**：Bug 修复

## 现象

加载/生成时报错：`poisoned lock: another task failed inside`，之后各页面数据加载失败。

## 原因

Tauri **async command**（如 `report_generate`）在持有 `Mutex<Connection>` 时调用 `chat_text`，其内部 `Runtime::new().block_on(...)` 与已有 Tokio 运行时嵌套，可能 **panic** → 数据库锁被 poison，后续所有 `st.db.lock()` 均失败。

同时长时间持锁阻塞后台采集线程。

## 修复

1. **`PreparedTextChat`**：短临界区解析密钥/人设；`run_async()` 供 async command；`run_sync()` 供后台线程
2. **`report_generate`**：gather + prepare（持锁）→ `run_async()`（释放锁）→ save（持锁）
3. **`ai_test_persona`**：同样改为 prepare + `run_async()`
4. **时段调度**：prepare 后释放锁再 `run_sync()`
5. **`db::lock_conn`**：poison 时 `into_inner()` 尝试恢复，避免一次 panic 后永久不可用

## 使用

若当前会话已 poison：**重启应用一次**后再生成报告。

📁 已归档：`docs/questions/30-poisoned-lock报告生成修复-20260702.md`
