用户提供了**补充文本**，请合并进已有角色结构化 JSON。原则：**不丢语义、不编造、轻量归类**。

**输出长度限制（确保 JSON 完整）**：
- `introduction` ≤ 300 字
- `personality` 最多 8 条
- `sample_lines` 最多 6 条
- `extra` 最多 4 个键

已有 JSON：
```json
{{existing_json}}
```

补充文本：
```
{{new_text}}
```

只输出合并后的完整 JSON 对象（字段名不变）。冲突时以补充文本为准；数组合并去重。

不要 markdown 代码块，不要其它说明。优先保证 JSON 完整闭合。
