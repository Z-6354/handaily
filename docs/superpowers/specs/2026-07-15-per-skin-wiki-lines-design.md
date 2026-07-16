# 按皮导入 Wiki 台词 — 设计

**日期**: 2026-07-15  
**状态**: 已实现（v1）  
**来源**: 用户选 C（抓取+导入）· 匹配 C（规范化宽松）· 标识 C（回执+UI）· 实现思路 1  
**相关**: `2026-07-14-unified-character-skins-design.md`（原「本轮不做皮肤子集」由此作废）；roster browser / skin dual-columns

## 目标

1. BWIKI「舰船台词」按 **默认 / 各换装面板** 拆分，写入 Wiki DB  
2. `导入 Wiki` 将各套台词落到对应 `skin_id`，禁止再把整船台词复制到每皮  
3. 匹配失败或缺台词须可检查：导入回执 + 皮肤 `lines_status`（详情与 `/skins`）

## 非目标

- 游戏客户端 `ship_skin_words` 数据源  
- 自动重命名皮肤以对齐 Wiki  
- 音频落地  
- 改 hanpet 播放逻辑（仍读 `skin_lines`）

## Wiki 抓取

- `extractShipLines` 升级为按面板分组；每组：
  - `skin`: 标题字符串；通常/默认 → `"default"`
  - `skin_kind`: `default` | `skin` | `retrofit` | `oath` | `other`
  - `lines`: 现有 `ShipLine` 形状
- `ShipLine` 可带可选 `skin` 字段（扁平兼容）
- DB：`ships.lines_by_skin_json`（新列，默认 `[]`）；继续写扁平 `lines_json`（全组合并）供旧代码
- 重抓/ sync 后才有按皮数据；无新列时导入视为仅扁平

## 导入匹配

1. `default` → `{character_id}-default`  
2. 换装：规范化（去「换装」、书名号、空白、BD 前缀等）后与 `name_zh` / 资产 stem / `kanmusu_dir` 相等或互相包含  
3. 改造 / 誓约：有对应皮则写；否则记未匹配  
4. 命中：`upsert_skin(..., replace_lines=True)` 仅该皮；**未命中不写入任何皮**  
5. 停用：Live2D 文件夹路径里「同一 `lines_json` 复制到每 folder」；改为按皮匹配或只填 default 并标报告  

匹配结果写入 `skins.meta_json.lines_import`：

```json
{
  "status": "ready" | "empty" | "unmatched" | "stale_flat",
  "wiki_skin": "…",
  "matched_by": "default|name_zh|asset|kanmusu_dir|null",
  "updated_at": "ISO-8601"
}
```

API / 列表额外暴露计算字段 `lines_status`（优先 meta；否则依 `skin_lines` 行数推导 `ready`/`empty`）。

## 回执与 UI

**导入响应**含：

| 字段 | 含义 |
|------|------|
| `skins_lines_ok` | 按皮写入成功 |
| `skins_lines_empty` | Wiki 无该皮独立台词（常见，**非失败**） |
| `wiki_skins_unmatched` | Wiki 有套无对应库皮 |
| `roster_skins_unmatched` | 库皮未匹配到 Wiki 套 |
| `lines_report` | 短明细列表（日志需人类可读，勿裸 JSON） |

角色库日志醒目打印摘要。

**UI**：皮肤表「台词」列 — 就绪 / 无台词 / 未匹配 / 旧复制；`/skins` 同款；筛选「台词需关注」（含无台词/未匹配/旧复制）。

## 验收

1. 多换装舰：默认与至少一换装台词不同且 `skin_id` 正确  
2. 故意未匹配标题 → 不误写 + 回执/UI 未匹配  
3. 摘要数字与筛选一致  
4. 仅有旧扁平数据时行为明确（不 silent 错绑）

## 决策记录

| 项 | 选择 |
|----|------|
| 深度 | 抓取 + 导入（C） |
| 匹配 | 规范化宽松；对不上不写 |
| 标识 | 回执 + 皮肤状态（C） |
| 架构 | `lines_by_skin_json` + meta 状态 |
