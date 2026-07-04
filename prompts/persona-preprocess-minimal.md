将角色文本整理为**极短** JSON。总输出不超过 **600 字符**，务必完整闭合。

**硬性上限**：
- `introduction` ≤ 80 字
- `personality` 最多 3 条，每条 ≤ 15 字
- `speech_style` ≤ 40 字
- `sample_lines` 最多 3 条，每条 ≤ 35 字
- `relationships` ≤ 40 字
- `taboos` 固定 `[]`，`extra` 固定 `{}`

**必须按此骨架输出**（只填值，不改键名）：
{"name":"","source":"","introduction":"","personality":[],"speech_style":"","sample_lines":[],"relationships":"","taboos":[],"extra":{}}

参考角色名：{{name}}
参考出处：{{source}}

原文（已截断）：
```
{{raw_text}}
```

只输出上述 JSON 一行，不要 markdown、不要其它文字。
