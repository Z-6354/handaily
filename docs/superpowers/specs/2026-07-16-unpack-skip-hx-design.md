# 解包自动跳过并清理 `*_hx` 后缀

日期：2026-07-16  
状态：已实现（方案 B）

## 背景

碧蓝资源里常见 `ankeleiqi_2_hx`、`edu_3_hx` 等变体目录。这类后缀 `hx` 的 bundle 不需要解包，扫到时应直接略过，并清掉已解出的同名输出目录。

## 目标

- **匹配规则**：slug（文件名 stem，小写）以 `_hx` 结尾则跳过。  
  - 跳过：`ankeleiqi_2_hx`、`z23_hx`  
  - 不解跳：`ankeleiqi_2`、`qiye`、`foo_hx_bar`
- **覆盖范围**：网页批量解包 Job + CLI `unpack`（含显式 paths）
- **日志**：计入 `skip_count`，日志一行 `跳过(hx) {slug}`，与「跳过(已完成)」区分
- **清理**：非 dry-run 时删除 `output_root` 下已有的 `*_hx` 输出目录（含本次扫到的 hx slug 对应目录）
- **不做**：不改 roster DB

## 实现要点

1. `is_hx_slug(slug: str) -> bool` 放在 `unpack_complete.py`
2. `purge_hx_output_dirs(output_root)`：删除 `output_root` 下一层名为 `*_hx` 的目录
3. Job：发现 bundle 后分离 hx；记 `跳过(hx)`；非 dry-run 先 purge；其余照常解包
4. `unpack_one` 兜底：hx slug → 删除 out_dir（若存在）→ `skipped=True, skip_reason="hx"`
5. 测试覆盖匹配规则、purge、unpack 跳过

## 验收

- 扫到 `ankeleiqi_2_hx`：不解包，日志有 `跳过(hx) ankeleiqi_2_hx`
- `output_root/ankeleiqi_2_hx` 若已存在则被删除
- `ankeleiqi_2` 仍正常解包
