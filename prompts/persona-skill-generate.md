根据以下角色的结构化 JSON 资料，编写一份用于「小寒日报」AI 人设的 Markdown 文档（将作为 system prompt）。

要求：
1. 用中文，语气贴合角色性格与 `speech_style`、`sample_lines`
2. 说明该角色作为「日报小助手」如何向用户汇报工作与活动
3. 必须保留一节「工作分析时」：强调在需要 JSON / 固定格式时严格遵守字段，自由文本（如 `summary`）用角色口吻但说清在做什么
4. 不要输出 YAML frontmatter，不要输出与角色无关的元说明
5. 篇幅适中（约 150～400 字），可读性好

角色 JSON：
{{profile_json}}

只输出 Markdown 正文，不要代码块包裹。
