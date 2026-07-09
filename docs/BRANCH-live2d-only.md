# 纯 Live2D 分支说明 (feat/live2d-only)

> 本文件仅存在于 `feat/live2d-only` 分支。目标见 `docs/questions/144-*` 第四节。

## 定位

纯桌宠：**只做模型挂载 + Live2D 加载 + wiki 台词**。去掉日报、去掉 AI 汇报、去掉微信推送。

## 已落地

### 后端(运行时停用日报/AI/微信)
- `src-tauri/src/lib.rs`：注释停用 时间线 AI 调度、Agent HTTP、微信推送 的启动。
- `src-tauri/src/analysis/coordinator.rs`：`enqueue_segment` 直接返回，不再做语义分析/截图/vision。
- `src-tauri/src/analysis/period_scheduler.rs`：`on_app_switch` 直接返回，不再触发 AI 时段总结。
- 采集线程(tracker)仍在运行以支撑 idle/桌宠交互，但不再驱动任何 AI 或截图，资源占用大幅下降。

### 前端(导航精简为桌宠)
- `src/App.tsx`：主导航仅保留「人物」；更多菜单保留 性能检测/设置/帮助。默认进入人物页。
- 移除日报相关导航项(今日工作/生成报告/时间线/热力图/应用记录/历史报告/接入 Agent/微信绑定)。

台词来源：`pet_wiki_import_lines`(wiki 爬取) + 手动导入。

## 待办(物理瘦身，进一步减小体积)

1. **删除模块**：从 crate 移除 `analysis/ report/ ai/ wechat/`，并清理 `state.rs`、`ipc/commands.rs`、`tray.rs` 的相关引用与命令注册。
2. **移除 AI 台词 UI**：`src/components/PetActionSettings.tsx` 中的 `petAiImportLines`/AI 建议入口(第 ~327、~780 行),仅保留 wiki 导入与手动导入。
3. **删除日报页面组件**：`TodayDashboard/ReportGeneratePage/HistoryReportsPage/HeatmapPage/AppRecordsPage/TimelineView/AgentConnectPage/WeChatBindPage` 及其 lazy 引用。
4. **精简 Cargo 依赖**：去掉仅日报/AI/微信使用的依赖(如 vision 相关 image 特性、qrcode 等)。

> 说明：本轮采用「运行时停用 + 入口移除」实现纯桌宠效果，功能上已达标；物理删除留作后续，以控制单次改动风险。

## 与 main 的关系

- 方向相反，**不回并** main 的日报改造。
- 仅按需 cherry-pick `pet/`、`character/`、构建脚本等共享改进。
