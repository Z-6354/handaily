# 舰娘/桌宠全链路优化 — 设计

**日期**: 2026-07-14  
**状态**: 已落地

## 范围

仅舰娘 Cubism 桌宠 + 独立预览 + 右键菜单换模。不含 Spine↔Cubism 同窗 navigate、其它 monorepo 子项目。

## 定稿

1. Load payload 带 `model_abs_dir`；主路径 `convertFileSrc`；失败回退 `prime_model`/base64。
2. 菜单栏切舰娘：跳过 `select_character_skin` 写仓，直接 `desktop_open`。
3. `preview_open` 对齐 `lookup_skin_detail` + 非阻塞 refresh。
4. 点击时 motion 未进 definitions：短等补拉再播。
5. `pet_click_through_poll` 合并穿透 IPC。
