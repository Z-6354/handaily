# 人物列表 UX 与双角色合并 — 设计

**日期**: 2026-07-14  
**状态**: 已批准（规范 id = 拼音 / kanmusu 目录名）

## 问题

1. 搜索框聚焦出现两个清除 ×（原生 search + 自定义）
2. 收藏角色未置顶（仅「收藏」筛选时才传 `favoriteIds`）
3. 人物页 keep-alive：去设置再返回仍停在搜索结果
4. BWIKI hash 角色与 roster 拼音角色并存（如 `p92564837` / `aijier`）
5. 皮肤：小人 Spine（`model_id`）应覆盖各皮；Cubism（`kanmusu_dir`）可选；同序号皮合并到一行

## 决策

- 规范角色 id：**拼音**（与 `kanmusu-models/{slug}`、`aijier_2` 目录一致）
- 合并：按中文名 / `wiki_title` / 别名将 hash 角色并入拼音角色；皮肤按 `skin_index`（或文件夹后缀）对齐
- 收藏、active 引用若指向旧 id，重写到规范 id

## 实现边界

- UI：`PersonaPanel` / `useCharacterRoster` / `App` 显隐
- 数据：一次性合并 AppData characters+personas；roster import/sync 去重避免复发
