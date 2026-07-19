# roster 解包 / 导入 / 绑皮 — 全量解耦重写（方案 C）

**日期**: 2026-07-19  
**状态**: 待用户审阅  
**前置**: 已存档 `hanimport/scripts/roster/_archive/20260719-pre-decouple-rewrite/`  
**范围**: 重写 `roster/db.py` monolith 及相关解包→绑皮链路，使规则可独立修改  
**非目标**: 不改 hanpet 运行时；不重跑全量解包；不删 `data/pet`/`data/skin` 资源；不强制迁移用户 sqlite  schema（除非缺列）

## 1. 背景

`hanimport/scripts/roster/db.py`（~3800 行）混杂：别名、文件夹规则、CRUD、同名合并、绑皮扫描、Wiki 导入、AppData 同步、CLI。  
已有 `bind.py` / `ids.py` 等仅为 re-export。近期规则（`_2`→skin1、誓约 `_h`、younv→小 XX、`absent`）继续堆进 monolith，修改成本高。

用户选择：**方案 C — 全量重写解耦**；**先存档再重构**。

## 2. 目标结构（目标态）

```
hanimport/scripts/roster/
  folder_rules.py      # 纯规则：strip_skin / 后缀→皮槽 / META·younv·idol·oath·数字
  aliases.py           # LIVE2D_ALIASES + JSON + enrich_alias_map_from_roster
  bind_pipeline.py     # 编排：扫 pet/skin、bind_*、repair_*
  skin_probe.py        # 已有：探针 + absent（可微调 import）
  crud.py              # upsert_character / upsert_skin / connect helpers
  schema.py            # schema / paths / connect / apply_schema
  import_wiki.py       # run_import_wiki + slots/lines
  merge.py             # merge_roster_duplicates / purge_folder_like
  sync.py              # sync_appdata / publish_bundled / export
  cli.py               # argparse main
  db.py                # 兼容层：from X import * 再导出（旧测试/脚本不炸）
  _archive/…           # 只读快照，不参与运行
```

Web / 解包入口：

| 入口 | 目标依赖 |
|------|----------|
| `web/import_ab_bind.py` | `bind_pipeline` + `unpack` + `aliases` |
| `web/transfer_unpack.py` | `common` unpack + 可选回调 `bind_pipeline` |
| `roster_db.py` | `roster.cli` / `roster.db` 兼容 |

## 3. 解耦原则

1. **规则无 IO**：`folder_rules` / 别名解析不读盘不写库。  
2. **编排调 CRUD**：`bind_pipeline` 只调用 `upsert_*` / `UPDATE`，不内嵌 Wiki 抓取。  
3. **兼容层**：`db.py` 保留符号表，直至测试与调用方改完。  
4. **行为金丝雀**：存档目录 + 现有 pytest 为对照；重写不得悄悄改编号语义。

## 4. 必须保持的行为契约（金丝雀）

| 规则 | 期望 |
|------|------|
| 裸 `slug` | 默认皮 |
| `slug_2` | Wiki skin1；**无 `slug_1`** |
| `slug_h` | `{cid}-oath`，仅 `pet_model_id`，扫 `data/pet` |
| `slug_younv` | 小 XX 默认皮，禁止写成人设 |
| `slug_idol` | XX(μ兵装) 默认皮 |
| 默认/誓约舰娘 | 无 Cubism → `kanmusu_status=absent`（「不存在」） |
| 特殊文件夹 | `is_special_pet_folder` 跳过 |

## 5. 重写策略（方案 C 落地节奏）

虽为「全量重写」，仍按**可回滚切片**交付（避免一次不可测大爆炸）：

| 切片 | 交付 | 验证 |
|------|------|------|
| C0 | 存档（已完成）+ 本设计 | 目录可读 |
| C1 | 新建空模块 + 把实现**剪切**进目标文件（逻辑暂不改） | `pytest hanimport/tests -q` |
| C2 | `folder_rules` + `aliases` 去环（db 只 re-export） | 绑定类测试 |
| C3 | `bind_pipeline` 成为唯一绑皮入口；瘦身 `import_ab_bind` | repair-l2d 冒烟 + 丹佛/安克雷奇手工点验 |
| C4 | `import_wiki` / `merge` / `sync` / `cli` 实体化 | import/sync 相关测试 |
| C5 | 删掉 `db.py` 内残留实现，仅兼容 re-export；更新 `pet-folder-bind-rules.md` 索引 | 全量 pytest + `roster:repair-l2d` |

若中途行为漂移：以 `_archive/20260719-…/db.py` 对照函数。

## 6. 测试

- 保留并扩展：`test_bind_*`、`test_oath_*`、`test_variant_*`、`test_kanmusu_absent_*`  
- C1 起每次切片必须全绿再合并下一刀  
- 手工：`/roster` 安克雷奇（成人 `ankeleiqi` / 小 `ankeleiqi_younv` / 誓约 `ankeleiqi_h`）、丹佛（`danfo` / `danfo_2`）

## 7. 风险

| 风险 | 缓解 |
|------|------|
| 循环 import | 规则←不依赖 db；pipeline→crud/schema 单向 |
| 漏 re-export | `roster_db.py` + 兼容 `db.py` 符号清单测试 |
| 误改规则 | 金丝雀表 + 存档 diff |

## 8. 非本设计

- 不在本轮改 UI 视觉（除非状态字段契约变）  
- 不自动清空 transfer 批次  
- 不把 `_archive` 挂进运行路径  

---

**请审阅本文件。批准后**再写 `docs/superpowers/plans/2026-07-19-roster-decouple-rewrite.md` 并开始 C1。
