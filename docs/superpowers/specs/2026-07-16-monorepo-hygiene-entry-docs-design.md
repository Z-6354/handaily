# Monorepo 工程卫生：入口统一 + 文档路径对齐

**日期**: 2026-07-16  
**状态**: 已确认（方案 A；用户快进开工）  
**范围**: 入口文档/缺漏 bat + 活文档 + `docs/questions` 正文路径改写  
**非目标**: 业务逻辑、API、UI、DB；不改写 `docs/superpowers/specs|plans` 历史设计正文（除非活文档会误导）

## 1. 目标

monorepo 拆分后，「怎么启动」与「路径写哪」一致：

- 官方入口一眼能找
- 根 README / ARCHITECTURE / 模块索引 / `scripts/README` 与真实布局一致
- `docs/questions`（约 73 篇含旧路径）正文尽量改为 `hanpet/` · `hanimport/` · `data/` 下路径，保留历史结论与日期

## 2. 官方入口（规范）

| 用途 | 官方入口 | 备注 |
|------|----------|------|
| 桌宠开发 | `npm run tauri:dev` 或 `scripts/start-dev.ps1` / **新建** `scripts/start-dev.bat` | bat 与 `start.bat` 同模式，调 ps1 |
| 桌宠已构建运行 | `scripts/start.ps1` / `start.bat` | |
| 桌宠发布包 | `npm run build:release` / `scripts/build.ps1` | |
| 导入器网页 | `npm run hanimport:serve` 或 `hanimport/启动网页版.bat` | 根 `小寒导入器.bat` → `hanimport/启动小寒导入器.bat`（薄包装，保留） |
| 传输桌面端 | `npm run hantransfer` / `scripts/start-hantransfer.ps1` | |
| 快捷通道 | `scripts/hanagent.ps1 xiaohan dev\|build\|pack\|stop` | 已指向根 `start-dev` / `build`；`pack` 用 hanpet `tauri:pack` |

**收敛规则**

- `scripts/README.md` 列出上表；删除或修正不存在的 `scripts/start-dev.bat` 引用（改为存在）
- `hanpet/scripts/start-dev.bat` 已转发到根 `scripts/start-dev.ps1` — 保留作应用内快捷方式，文档标明「推荐用仓库根 scripts」
- 不删功能脚本；若确认无引用且危险，仅在 README 标「遗留」

## 3. 活文档校对清单

| 文件 | 动作 |
|------|------|
| `README.md` | 快速启动与命令表保持 monorepo；补一行「详细入口见 scripts/README」 |
| `docs/ARCHITECTURE.md` | 已基本正确；核对树图与链接 |
| `docs/01-…/03-模块索引.md` | 已基本正确；核对入口 bat 名 |
| `scripts/README.md` | 对齐第 2 节表；补 `start-dev.bat`；hantransfer / clean 条目完整 |
| `docs/README.md` | 可选：加链到本 hygiene spec；不强制 |

## 4. questions 路径改写规则

对 `docs/questions/*.md` 做**机械替换 + 结构说明人工校对**：

| 模式（未带 `hanpet/` / `hanimport/` 前缀时） | 替换为 |
|---------------------------------------------|--------|
| `` `src-tauri/ `` · `src-tauri/` · `cd src-tauri` | `hanpet/src-tauri/…` |
| 根级前端 `` `src/ ``（非 `src-tauri/src`） | `hanpet/src/…` |
| 根级 `dist/`（前端产物语境） | `hanpet/dist/` |
| 根级 `bundled/`（prompts/roster/pet-models） | `hanpet/bundled/…` |
| 根级 `personas/`（若仍出现） | `hanpet/bundled/roster/personas/…` |
| 根级 `package.json` 叙述「在根跑 tauri」 | 保持根 npm workspace，注明 `-w hanpet` 已封装 |

**保留**

- 日期、标签、结论正文语义
- 已存在的「路径更新」注记：若与改正文重复，可缩短为「正文已按 2026-07-16 monorepo 路径改写」

**人工必过**

- `121-src与src-tauri及release构建目录说明-*`：整棵目录树改为 `HANDAILY/hanpet/…`
- 含完整仓库树 ASCII 的问答：扫一遍避免把 `src-tauri/src` 误加成 `hanpet/src-tauri/hanpet/src`

**验收**

- `rg` 在 `docs/questions` 中：裸 `src-tauri/`（前无 `hanpet/`）与「根语境下的 `cd src-tauri`」趋近于 0
- 抽查 5 篇（含 121、构建类、pet 类）路径可点开对应文件

## 5. 实现与验证

1. 新增 `scripts/start-dev.bat`
2. 更新活文档 + `scripts/README`
3. 批量改 questions（脚本或编辑器替换），再人工过 121 等
4. 提交一个（或入口/文档分两笔）commit；不改业务代码

**成功标准**: 新人只读根 README + `scripts/README` 能启动桌宠与导入器；questions 抽查路径落在 `hanpet/` 下真实文件。
