# hanimport 全站苹果风对齐 — 设计

**日期**: 2026-07-15  
**状态**: 已实现  
**来源**: 用户选 1（全站统一）· 参考 apple.com 气质 · 在既有 hub 壳层上加深对齐  
**相关**: `2026-07-15-hanimport-apple-hub-redesign-design.md`

## 目标

Hub / 解包 / 角色库 / 皮肤四页共用同一 token + 控件语言；更大留白、更轻字重、统一圆角与主 CTA；不改业务语义。

## 落点

| 文件 | 作用 |
|------|------|
| `design-system/tokens.css` | 扩充间距 / 圆角 / focus |
| `components.css` | 共享按钮、输入、卡片、表、badge |
| `shell.css` / 各页 css | 引用 token，去掉分叉样式 |

## 非目标

暗色模式、SPA、像素复刻、改 API。
