# 角色库列表筛选与排序

日期：2026-07-16  
状态：已实现

## 目标

在 `/roster` 角色网格上增加筛选与排序，便于按阵营、皮肤数量、Cubism/Spine 磁盘状态浏览，并按时间找出最近改动/导入的角色。

## 筛选

| 控件 | 参数 | 取值 |
|------|------|------|
| 阵营 | `faction` | 空=全部；否则精确匹配 `characters.faction` |
| 皮肤数量 | `skin_count` | `all` \| `none`(0) \| `some`(≥1) \| `many`(≥3) |
| Cubism | `kanmusu` | `all` \| `unbound` \| `missing` \| `ready` |
| Spine | `pet` | 同上 |

**角色级资产状态**（与皮肤表探针一致）：对该角色全部皮肤跑 `skin_probe`，取「最好一档」：

- 任一皮 `ready` → 角色 `ready`
- 否则任一 `missing` → `missing`
- 否则 → `unbound`

## 排序

| 选项 | `sort` | 规则 |
|------|--------|------|
| 默认 | `default` | 无标准名 stub 沉底 → `name_zh` → `id`（现有行为） |
| 修改时间 | `updated` | `characters.updated_at`；默认 `order=desc` |
| 最新导入文件 | `import_mtime` | 该角色皮肤绑定目录（kanmusu / pet）最新 mtime；无文件沉底；默认 desc |

`order`：`asc` \| `desc`（仅对 `updated` / `import_mtime` 生效；默认 `desc`）。

## API

扩展 `GET /api/roster/characters`：

```
?faction=&skin_count=all&kanmusu=all&pet=all&sort=default&order=desc&q=&offset=&limit=
```

新增 `GET /api/roster/factions` → `{ ok, factions: string[] }`（非空去重，排序）。

列表项可附带（便于调试/UI）：`skin_count`、`kanmusu_status`、`pet_status`、`import_mtime`（unix 或 ISO，可空）。

## UI

搜索框下增加筛选条：

- 阵营 `<select>`
- 皮肤数 `<select>`
- Cubism `<select>`
- Spine `<select>`
- 排序 `<select>`（默认 / 修改时间 / 最新导入）

变更后 `offset=0` 并重新请求。未命名 stub 在默认排序下仍沉底；卡片 `unnamed_stub` 样式保留。

## 非目标

- 不预建 `character_assets` 表（方案 B）
- 不在前端拉全量筛选（方案 C）
- 不做皮肤数精确区间输入

## 验收

- 选「白鹰」只出现该阵营
- 「无皮肤」不含有皮角色；「多皮」仅 ≥3
- Cubism=已就绪 仅含至少一套 Cubism ready 的角色
- 按修改时间 desc：最近 `updated_at` 在前
- 按导入 mtime：刚解包进目录的角色靠前
- 相关 API/UI 测试通过
