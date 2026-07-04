根据以下活动时间线片段，为每条写一句中文简介。

**重要：system 已注入当前 AI 人设 Skill，你必须完全按该人设的口吻、称呼和性格来写**，像给主人讲今天做了什么，轻松、有温度，不要官腔、不要机器报告体。

工作类型只能从以下选项中选一个：{{type_list}}

每条片段 JSON 字段说明：
- `time` / `duration`：时间段与停留时长
- `app` / `window_title`：应用显示名与原始窗口标题（**不要照抄 window_title 全文**）
- `app_kind`：应用类别（cursor / browser / ide / terminal / chat / other）
- `parsed_context` / `context_hints`：结构化上下文，**优先参考**
- `activity_label`：当前活动内容（项目名/页面等）
- `prior_activities_in_app`：同应用内更早的不同活动，可承接，勿重复
- `hybrid_insights`：文本/截图分析摘要，仅作参考，**不要复制其中的「开发：」前缀或窗口标题**

写作要求：
- summary 20～60 字，一句完整中文
- **禁止**输出「开发：」「文档：」等分类前缀
- **禁止**复述整段 window_title 或「窗口「…」」这类机器格式
- 自然提到项目名、文件名、网页主题即可

原始片段（JSON）：
{{segments_json}}

只输出 JSON 数组，不要 markdown 代码块：
[{"id":"2026-07-03T10:15:00+08:00","work_type":"开发","summary":"一句中文简介"}, ...]

**id 必须与输入 JSON 里每条 `id` 字段完全一致**（为 started_at 时间戳字符串）。禁止使用 0、1、2 等序号代替。
