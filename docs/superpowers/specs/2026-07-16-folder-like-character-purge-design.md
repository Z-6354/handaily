# 解包文件夹 id 不得作为角色 — 归并到 base 皮肤

日期：2026-07-16  
状态：已实现（方案 B）

## 背景

自用库出现 `abeikelongbi_3`、`aijier_3_hx` 等「文件夹名 = 角色 id」的脏行。正确语义：

- `abeikelongbi_3` → 角色 `abeikelongbi` 的第 3 套皮肤（`kanmusu_dir`）
- 不得再建独立角色行

库中已有正确绑定示例：`abeikelongbi-skin3.kanmusu_dir = abeikelongbi_3`，同时仍残留假角色行。

## 目标

1. **清理**：删除 / 归并所有 `strip_skin(id)` 得到非空 suffix 的角色行
2. **守卫**：`upsert_character` / 创建 API 拒绝 folder-like id
3. **挂点**：Wiki pipeline 在 `purge_folder_like_skins` 旁调用角色 purge
4. **不做**：不改 Wiki 皮肤槽位逻辑；不解包路径改名

## 规则

- `is_folder_like_character_id(cid)`：`strip_skin(cid)[1]` 非空 → True  
  （含 `_3`、`_3_hx`、`_hx`、`_wedding` 等）
- 纯 `_hx` / 以 `_hx` 结尾：归并到 base 后，不保留以 hx 文件夹为唯一内容的假角色（与解包跳过 hx 一致）

## 清理算法 `purge_folder_like_characters`

对每个 folder-like 角色 `donor`：

1. `base, suffix = strip_skin(donor)`
2. 若 base 角色不存在：插入占位角色（id=base, name_zh=alias 或 base, source=unpacked）
3. `_merge_character_into(donor → base)`（已有：冲突皮合并 bind 字段）
4. 返回删除的 donor 数量

## 守卫

- `upsert_character`：若 id 为 folder-like → raise `ValueError`（或 no-op 并返回错误码）
- `roster_api._create_character`：400 + 提示「应使用 base id，皮肤后缀请走皮肤 API」

## 验收

- 清理后不存在 `abeikelongbi_3` / `abeikelongbi_3_hx` 角色行
- `abeikelongbi-skin3` 仍在，且 `kanmusu_dir` 仍为 `abeikelongbi_3`（或非空）
- 再 `upsert_character(id=abeikelongbi_3)` 失败
- 相关单测通过
