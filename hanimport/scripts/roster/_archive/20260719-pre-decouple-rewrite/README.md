# 存档：roster 解包/绑皮重构前快照

**日期**: 2026-07-19  
**原因**: 用户确认采用方案 C（全量重写解耦）前，冻结当前可运行实现以便对照与回滚阅读。  
**勿直接运行本目录代码** — 仅作历史参考；运行时仍以 `hanimport/scripts/roster/` 现行模块为准。

## 包含文件

| 文件 | 说明 |
|------|------|
| `db.py` | 当时的 monolith（ids / crud / merge / bind / import_wiki / sync / cli） |
| `bind.py` / `ids.py` | 薄 re-export 壳 |
| `skin_probe.py` | pet/kanmusu 探针与 `absent` 状态 |
| `import_ab_bind.py` | AB 解包→绑皮 Job |
| `transfer_unpack.py` | transfer 批次解包 |
| `pet-folder-bind-rules.md` | 当时手册副本 |

## 当时已固化的关键规则（摘要）

- 裸 `slug` → 默认皮；`slug_2` → Wiki skin1；**无 `slug_1`**
- `slug_h` → 誓约（仅 pet，扫 `data/pet`）
- `slug_younv` → 小 XX；`slug_idol` → XX(μ兵装)
- 默认/誓约舰娘列：无资源显示「不存在」(`absent`)，非「未绑定」

## 恢复提示

若需对照旧实现：打开本目录 `db.py` 对应函数，勿整文件覆盖回现行树（现行 `db.py` 可能已拆分）。  
Git：本存档随提交进入版本库；亦可用该 commit 对比。
