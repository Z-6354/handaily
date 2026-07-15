# Wiki TabContainer 定皮肤再绑台词 — 设计

**日期**: 2026-07-15  
**状态**: 已批准（思路 1 · 誓约规则 C）  
**相关**: `2026-07-15-per-skin-wiki-lines-design.md` · `2026-07-15-wiki-lines-auto-fetch-design.md`

## 目标

1. 用图鉴 **`TabContainer` / `tab_li` + `tab_con`** 确定角色皮肤清单  
2. **排除「改造」**；**空 `tab_con`（无立绘）不建皮**；誓约仅在有图/可绑台词时保留  
3. **先皮肤、后台词**：`lines_by_skin` 按皮肤顺序绑定；导入不再用散落 `assets` 堆出换装皮

## 抓取（blhx-wiki）

新增 `ShipSkinSlot`：

```ts
{ key: "default" | "skin1" | "skin2" | … | "oath"
  label: string      // 通常 / 换装1 / 誓约
  kind: "default" | "skin" | "oath"
  image_url: string | null
  image_alt: string | null }
```

- 解析第一个（或主图鉴）`TabContainer`：`tab_li` 与 `tab_con` 按序对齐  
- 丢弃：label 含「改造」  
- 丢弃：无 `img` 且无有效内容的 tab  
- 写入 `ships.skins_json`；`assets_json` 可仍保留作附件  

台词：`extractShipLinesBySkin` 结果按顺序挂到 `skins_json`（default→通常；skin 类面板按序；oath→誓约；**改造台词组丢弃**）。输出 `lines_by_skin` 的 `skin` 用 slot.label 或 key，并带 `slot_key`。

## 导入（hanimport）

- 有 `skins_json`：按槽建 `{cid}-default` / `{cid}-skin1`… / `{cid}-oath`，`name_zh=label`  
- **整角色替换**：导入后删除不在本次权威清单内的旧皮肤（含以前误建的 `*-skin57`、改造皮等）及其台词；非纯增量残留  
- 台词：按 `slot_key` / 序匹配，**不用** assets ordinal 对「柴郡换装.jpg」  
- 无 `skins_json`：旧 assets 逻辑，同样对 keep 集合外皮肤做删除

## 验收

- 柴郡类：通常+换装1–4+誓约（若有图），无改造皮  
- 台词落到对应换装序，默认 ≠ 换装文案  
- 空誓约 Tab 不出现皮  
- 再导入 Wiki 后，旧错误皮（如 `*-skin57`）被删除而非残留

**状态**: 已实现（含导入整角色替换）
