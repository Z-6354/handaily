# hanimport — 小寒导入器

**开发用**碧蓝航线资源导入工具链：解包 → 配置 → 角色库（Wiki 补齐）→ 同步 / 发布。不随 hanpet 发行版打包。

## 网页（推荐）

```bat
启动网页版.bat
```

| 路径 | 作用 |
|------|------|
| `/` | Hub：解包入口 + 角色库入口 + 近期 Job |
| `/unpack` | 批量解包 + 写入后生成 JSON（异步 Job） |
| `/roster` | 角色 / 皮肤 / 台词管理；自用库 ↔ 自带库 |
| `/skins` | 全库皮肤探针（Spine / Cubism / 台词状态） |

**自用库**打开后自动跑 **Wiki 补齐**（角色 → 头像/皮肤 → 按皮台词）；无「导入 Wiki」按钮。台词摘要请读「就绪 / Wiki无该皮台词 / 未对上」，勿把「无台词」当失败。

## CLI 职责

| 能力 | hanimport 子命令 | 底层实现 |
|------|------------------|----------|
| 资源解包 | `unpack` | AssetBundle 解包（网页 Job 同路径） |
| 模型配置 | `config` | `build_model_config` / `build_cubism_config` |
| 导入计划 | `plan` | `mcp/blhx-wiki` live2d-plan |
| 模型批量导入 | `models` | `live2d_import` bin |
| BWIKI 人设导入 | `personas` | `blhx_import` bin |
| 角色包导出 | `roster export` | `roster_pack` bin |

## 快速使用

```bash
# 统一入口
npm run hanimport -- --help

# 解包（骨架 / CLI）
npm run hanimport -- unpack --input <bundle.ab> --dry-run

# 生成导入计划
npm run live2d:plan
npm run hanimport -- plan --out data/import/live2d-plan.json

# 批量生成模型配置（Spine / Cubism）
npm run hanimport -- config --input data/live2d
npm run hanimport -- config --input data/model/unpacked --force
npm run hanimport:config:cubism

# 批量导入模型 / 人设
npm run hanimport -- models --plan data/import/live2d-plan.json --dry-run
npm run hanimport -- personas -- --all --skip-existing --limit 50

# 导出角色包
npm run hanimport -- roster export
```

仓库根 `npm run blhx:import` / `live2d:import` 仍可用（直接调用 hanpet CLI bin）。

## 角色内容库（roster）

本地个人库与自带预览库：**勿**把个人库整文件拷进 bundled。

```bash
npm run roster:init
npm run roster:import    # 调试：CLI 导入（网页已全自动补齐）
npm run roster:sync      # → 本机 AppData
npm run roster:publish   # allowlist → hanpet/bundled
npm run roster:verify
```

脚本：`hanimport/scripts/roster_db.py` · 网页 API：`roster_api.py` / `serve_web.py`

## 设计文档

总进度：[docs/README.md](../docs/README.md#2026-07-15-进度已落地)

| 主题 | Spec |
|------|------|
| 双轨库 | `docs/superpowers/specs/2026-07-14-dual-roster-database-design.md` |
| 角色库页 | `…/2026-07-15-hanimport-roster-browser-design.md` |
| Wiki 全自动 | `…/2026-07-15-roster-auto-pipeline-design.md` |
| 按皮台词 | `…/2026-07-15-per-skin-wiki-lines-design.md` |
| 模块设计 | [docs/DESIGN.md](docs/DESIGN.md) |

## 环境变量

| 变量 | 默认 | 用途 |
|------|------|------|
| `HANDAILY_LIVE2D_PATH` | `data/live2d/` | Spine 模型工作目录 |
| `HANDAILY_LIVE2D_PLAN` | `data/import/live2d-plan.json` | 批量导入计划 |
| `BLHX_WIKI_DB_PATH` | `data/wiki/blhx.sqlite` | BWIKI 本地缓存 |
| `HANDAILY_DATA_DIR` | `%AppData%/xiaohan-daily/data` | hanpet 运行时数据 |
| `HANDAILY_ROSTER_DB` | `data/roster/handaily-roster.sqlite` | 本地个人角色库 |
