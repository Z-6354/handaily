将以下角色相关文本做**精简**结构化整理（参考文本可能来自 Wiki，台词条目较多，请择优摘录）。

原则：
- **输出必须短**：确保 JSON 完整闭合，宁可省略次要信息
- **不编造**：只使用原文信息
- **台词择优**：`sample_lines` 只保留最有角色代表性的 3～5 条，不要穷举

字段说明：
- `name`：角色名
- `source`：作品/出处
- `introduction`：背景与设定摘要（**不超过 200 字**）
- `personality`：性格要点（**最多 5 条**，每条 ≤ 25 字）
- `speech_style`：说话风格、口癖（**不超过 80 字**）
- `sample_lines`：代表性台词（**最多 5 条**，每条 ≤ 60 字，原文摘录）
- `relationships`：关系（**不超过 80 字**）
- `taboos`：禁忌（数组，无则 `[]`）
- `extra`：其它要点（**最多 2 个键**，值各 ≤ 40 字）

参考角色名：{{name}}
参考出处：{{source}}

原始文本：
```
{{raw_text}}
```

**输出格式**：只输出一个 JSON 对象，单行亦可。不要 markdown 代码块，不要其它说明。数组字段即使为空也输出 `[]`，`extra` 无内容时输出 `{}`。

**骨架参考**（键名必须一致）：
{"name":"","source":"","introduction":"","personality":[],"speech_style":"","sample_lines":[],"relationships":"","taboos":[],"extra":{}}
