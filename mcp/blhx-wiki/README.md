# BLHX Wiki MCP

碧蓝航线 BWIKI 舰娘数据 MCP 服务，供 **Cursor / 其他 AI 开发者** 调用，抓取并缓存舰娘资料，便于后续导入小寒日报人物/性格。

> 本 MCP **不**接入小寒日报内置 AI；开发者通过 MCP 拉取数据后，再由 AI 协助写入 HANDAILY。

## 功能

| 工具 | 说明 |
|------|------|
| `blhx_stats` | 同步状态统计 |
| `blhx_sync_catalog` | 从 [舰船图鉴](https://wiki.biligame.com/blhx/%E8%88%B0%E8%88%B9%E5%9B%BE%E9%89%B4) 同步索引 |
| `blhx_sync_ships` | **增量**批量抓取详情（跳过已存在） |
| `blhx_fetch_ship` | 抓取单个舰娘 |
| `blhx_get_ship` | 读取本地缓存 |
| `blhx_search_ships` | 搜索已抓取舰娘 |
| `blhx_list_ships` | 列出图鉴（可按已/未抓取过滤） |
| `blhx_export_handaily` | 导出 HANDAILY 性格导入格式 |
| `blhx_scan_live2d` | 扫描 `live2d/` 下含 Spine 三件套文件夹 |
| `blhx_match_live2d` | 文件夹拼音 slug → BWIKI 舰娘匹配（含 `_2` 皮肤变体） |
| `blhx_live2d_import_plan` | 生成模型导入计划（检查人设/模型是否已存在） |

## 抓取内容

每个舰娘 Wiki 页（如 [欧根亲王](https://wiki.biligame.com/blhx/%E6%AC%A7%E6%A0%B9%E4%BA%B2%E7%8E%8B)）包括：

- 角色信息（身份、性格、关键词等）
- 情人节礼物、角色设定、角色剧情卡
- 舰船台词（含 `data-key`、语言、语音 MP3 URL）
- 头像 / 立绘 / 换装 / Q 版等资源 URL
- `personaReference` 文本（对齐 HANDAILY `persona_import_wiki` 参考格式）

## 安装

```bash
cd mcp/blhx-wiki
npm install
npm run build
```

## Cursor 配置

在项目或用户 `mcp.json` 中添加：

```json
{
  "mcpServers": {
    "blhx-wiki": {
      "command": "node",
      "args": ["D:/0HAN/HANDAILY/mcp/blhx-wiki/dist/index.js"],
      "env": {
        "BLHX_WIKI_DB_PATH": "D:/0HAN/HANDAILY/mcp/blhx-wiki/data/blhx.sqlite",
        "BLHX_WIKI_DELAY_MS": "350"
      }
    }
  }
}
```

开发时可用 `npm run dev` 代替 `node dist/index.js`。

## 典型工作流

1. `blhx_sync_catalog` — 同步 ~970 舰娘索引
2. `blhx_sync_ships` `{ "limit": 20 }` — 分批增量抓取（多次调用直至 pending=0）
3. `blhx_get_ship` `{ "name": "欧根亲王" }` — 查看完整数据
4. `blhx_export_handaily` `{ "name": "欧根亲王" }` — 获取 HANDAILY 导入包
5. 在 HANDAILY 人物页使用 Wiki/文本导入，或让 AI 写入 `personas/` 与 `characters/manifest.json`

### Live2D 批量导入（人设 → 模型 → 皮肤）

1. **批量导入人设**（需 AI 密钥，耗时较长）  
   ```powershell
   cd src-tauri
   cargo run --bin blhx_import -- --all --skip-existing --limit 50
   ```
2. **MCP 匹配 live2d 文件夹**  
   - `blhx_match_live2d` — 查看匹配结果（约 1556 个 Spine 包）  
   - `blhx_live2d_import_plan` `{ "only_with_persona": true }` — 仅已导入人设的舰娘
3. **生成计划并导入模型**  
   ```powershell
   cd mcp/blhx-wiki
   npm run live2d-plan -- --out plan.json
   cd ../../src-tauri
   cargo run --bin live2d_import -- --plan ../mcp/blhx-wiki/plan.json
   ```

文件夹名如 `adaerbote`、`adaerbote_2` 会通过拼音 slug 匹配 BWIKI 舰娘；非标准罗马音可在 `data/live2d-aliases.json` 补充。

## 环境变量

| 变量 | 默认 | 说明 |
|------|------|------|
| `BLHX_WIKI_DB_PATH` | `mcp/blhx-wiki/data/blhx.sqlite` | SQLite 路径 |
| `BLHX_WIKI_DELAY_MS` | `350` | 请求 BWIKI 间隔（毫秒） |
| `HANDAILY_LIVE2D_PATH` | 仓库 `live2d/` | Live2D 模型根目录 |
| `HANDAILY_DATA_DIR` | `%AppData%/xiaohan-daily/data` | 小寒日报数据目录 |

## 数据目录

`data/` 已 gitignore，SQLite 与下载元数据仅存本地。

## 与 HANDAILY 的关系

- 解析逻辑参考 `src-tauri/src/pet/wiki_scrape.rs`
- 导出格式兼容 `persona_import_wiki` / 文本导入
- **批量导入 CLI**：`src-tauri` 下 `cargo run --bin blhx_import -- "欧根亲王,贝尔法斯特"` 或 `--all --skip-existing`
- **Live2D 导入 CLI**：`cargo run --bin live2d_import -- --plan plan.json`
- **应用内**：人物 → 新增 →「本地 BWIKI」Tab，调用 `persona_import_blhx_local`
- 桌宠 Spine 模型仍需单独导入；MCP 提供性格、台词与图片 URL
