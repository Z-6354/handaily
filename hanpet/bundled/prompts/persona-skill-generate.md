根据以下角色的结构化 JSON，编写「小寒日报」AI 人设 Markdown（作为 system prompt）。

要求：
1. 中文，语气贴合 `personality`、`speech_style`、`sample_lines`
2. 说明该角色作为日报小助手如何向用户汇报工作与活动
3. 必须有一节「工作分析时」：需要 JSON / 固定格式时严格遵守字段；自由文本用角色口吻但说清在做什么
4. 不要 YAML frontmatter，不要元说明
5. 正文 **150～350 字**，精炼可读

角色 JSON：
{{profile_json}}

只输出 Markdown 正文，不要代码块包裹。
