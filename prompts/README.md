# 小寒日报 · AI 提示词

本目录存放发送给大模型的提示词模板，使用 `{{变量名}}` 占位符，由 Rust `prompts` 模块渲染。

## 文件

| 文件 | 用途 | 变量 |
|------|------|------|
| `vision-screenshot.md` | 截图视觉分析 | `app_name`, `window_title` |
| `period-analysis.md` | 时段工作类型总结 | `type_list`, `activity_lines` |

## 运行时路径

应用启动时若用户目录尚无对应文件，会从内置模板复制到：

`%AppData%/xiaohan-daily/prompts/`

**修改提示词请编辑用户目录下的 `.md` 文件**（设置页可查看路径）。删除某文件后重启可恢复默认。

## 新增提示词

1. 在本目录添加 `your-prompt.md`
2. 在 `src-tauri/src/prompts/mod.rs` 的 `DEFAULTS` 注册
3. 在业务代码调用 `prompts::render(data_dir, "your-prompt", &[...])`

详见 `.cursor/skills/xiaohan-prompts-md/SKILL.md`。
