# 统一角色皮肤（Wiki → AppData）— 设计

**日期**: 2026-07-14  
**状态**: 已实现（入口收拢到人物页）；后续以 [双轨角色库](2026-07-14-dual-roster-database-design.md) 为元数据源  

**翻转**: 取消独立舰娘页；Cubism/Spine/台词挂在人物皮肤上；Wiki 经本地 roster sqlite 再 sync AppData

## 目标

- 人物 → 皮肤：每皮可挂桌宠 Spine（`model_id`）与舰娘 Cubism（`kanmusu_dir`）+ `lines`
- 中文名 / 皮肤名 / 台词从 `blhx.sqlite` 导入 AppData `characters/manifest.json`
- 英名：`aliases_json` 拉丁字母项优先；缺口 `data/wiki/ship-en-names.json`

## 字段

| 层级 | 字段 | 说明 |
|------|------|------|
| character | `id` | live2d base slug（如 `aidang`） |
| character | `name` | Wiki 中文名 |
| character | `english_name` | EN |
| character | `wiki_title` | BWIKI 标题 |
| skin | `id` | Cubism slug 或 `skin-{model_id}` |
| skin | `name` | 皮肤显示名 |
| skin | `skin_index` | 后缀数字 |
| skin | `model_id` | Spine pet-models id（可空串表示无桌宠） |
| skin | `kanmusu_dir` | Cubism 目录名（可空） |
| skin | `lines` | `{ text, animation?, wiki_key? }` |

## 英名规则

1. 自 `ships.aliases_json`：含拉丁字母且非纯 CJK 的条目；偏好最长合法 EN  
2. 否则 `ship-en-names.json[slug_base]`  
3. 否则空串

## 台词

整船 `lines_json` 复制到该角色各皮（本轮不做皮肤子集）。

## UI

无独立舰娘页；人物皮肤卡片展示双模型就绪与台词编辑；上桌走 companion_engine。
