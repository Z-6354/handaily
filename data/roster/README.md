# roster — 角色内容库（本地个人 / 自带预览）

| 路径 | 用途 | Git |
|------|------|-----|
| `handaily-roster.sqlite` | **本地个人库**（全量，可写） | **忽略** |
| `audio/` | 本地缓存的配音文件 | 忽略 |
| `schema.sql` | 同 schema 定义 | 提交 |
| `bundled-allowlist.json` | 允许写入自带库的角色 id 清单 | 提交 |
| `seed/` | 可选 JSON 补丁（英名等） | 仅 `.gitkeep` + 小补丁可提交 |

## 命令

```bash
# 初始化本地库
python hanimport/scripts/roster_db.py init

# Wiki + unpacked → 本地个人库
python hanimport/scripts/roster_db.py import-wiki

# 本地库 → 本机 AppData（开发测试，不给用户）
python hanimport/scripts/roster_db.py sync-appdata

# 按 allowlist 写出自带库（禁止整库拷贝）
python hanimport/scripts/roster_db.py publish-bundled

# 后续：挑选子集打用户数据包
python hanimport/scripts/roster_db.py export-pack --ids cheshire,aidang -o pack.zip

python hanimport/scripts/roster_db.py verify
```

环境变量：`HANDAILY_ROSTER_DB`、`HANDAILY_DATA_DIR`、`BLHX_WIKI_DB_PATH`。
