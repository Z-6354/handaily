---
name: xiaohan-prompts-md
description: >-
  Manages 小寒日报 AI prompt templates as Markdown files under prompts/ with
  {{variable}} placeholders. Use when adding or editing AI prompts, system
  messages, period analysis, vision screenshot instructions, or when the user
  asks to externalize hardcoded prompt strings to md files.
---

# 小寒日报 · 提示词 Markdown 规范

## 目录约定

| 路径 | 用途 |
|------|------|
| `prompts/*.md` | 仓库源文件（版本管理、PR 审查） |
| `%AppData%/xiaohan-daily/prompts/` | 运行时用户可编辑副本 |
| `src-tauri/src/prompts/mod.rs` | 加载器：`seed_user_prompts` + `render` |

## 模板语法

- 占位符：`{{variable_name}}`（双花括号，Rust 侧 `substitute` 替换）
- 纯文本 Markdown，**不要**把 YAML frontmatter 发给模型
- 文件末尾保留单个换行

## 新增提示词（检查清单）

1. 在 `prompts/your-name.md` 编写模板
2. 在 `prompts/README.md` 表格登记变量说明
3. 在 `src-tauri/src/prompts/mod.rs` 的 `DEFAULTS` 增加 `(name, include_str!(...))`
4. 业务代码调用：
   ```rust
   prompts::render(data_dir, "your-name", &[("key", "value")])
   ```
5. 传入 `data_dir`：从 `AppState::data_dir()` 或 `state.data_dir()` 获取
6. 运行 `cargo test -p xiaohan-daily prompts::` 验证

## 用户修改行为

- 首次启动：内置模板复制到用户 `prompts/`（**仅当文件不存在**）
- 用户改 `%AppData%/.../prompts/*.md` 立即生效（每次调用 `render` 读盘）
- 恢复默认：删除用户目录对应 `.md` 后重启

## 禁止

- 不要在 `adapter.rs` / `period.rs` 内联长提示词字符串
- 不要硬编码中文分类列表到多处；分类说明写在对应 `.md` 或 `work_types` 配置

## 相关 IPC

- `app_get_prompts_path` → 设置页展示路径

## 示例

`prompts/period-analysis.md`：

```markdown
工作类型只能从以下选项中选一个：{{type_list}}

活动记录：
{{activity_lines}}
```

调用：

```rust
prompts::render(data_dir, "period-analysis", &[
    ("type_list", &type_list),
    ("activity_lines", &lines),
])
```
