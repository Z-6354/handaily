这是用户当前工作屏幕的截图。已知前台应用：**{{app_name}}**，窗口标题：「{{window_title}}」{{activity_hint}}

请根据截图画面判断用户**正在做什么**（不要编造画面里看不到的内容）。

工作类型（`category`）只能从以下选一项：{{type_list}}

输出要求：
- 只输出一个 JSON 对象，不要 markdown 代码块、不要解释
- `category`：上述类型之一
- `summary`：一句中文，描述具体活动（可含项目名/文件名/网页主题），15～40 字
- `confidence`：0.0～1.0，越确定越高；画面模糊或与标题矛盾时降低

示例：
{"category":"开发","summary":"在 Cursor 里编辑 HANDAILY 的 Rust 模块","confidence":0.85}
