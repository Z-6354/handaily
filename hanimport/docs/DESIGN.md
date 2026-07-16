# hanimport 设计概要

## 定位

**hanimport（小寒导入器）** 是 HANDAILY 项目下的开发工具应用，负责把碧蓝航线外部资源变成 hanpet 可导入的格式。与面向用户的 **hanpet** 严格分离，不进入发行版安装包。

## 问题

hanpet / MCP 流水线假设 Spine 资源已在 **`data/live2d/`**：

- `mcp/blhx-wiki` — slug 匹配、生成导入计划
- `live2d_import` — 写入 AppData / bundled pet-models

仓库内尚无 Unity AssetBundle（`.ab` / `.unity3d`）解包能力。

## 子命令规划（目标 CLI）

```
hanimport unpack   --input <bundle|game-root> --output data/live2d
hanimport plan     # 封装 live2d-plan，写 data/import/live2d-plan.json
hanimport models   # 封装 live2d_import
hanimport personas # 封装 blhx_import
hanimport roster   # 封装 roster_pack export/import
```

v1 先实现 `unpack`；`plan` / `models` / `personas` / `roster` 已通过薄包装转发（Phase 2 前半）；后续将 bin 逻辑迁入 hanimport crate。

## 输入 / 输出

| 输入 | 示例 |
|------|------|
| 游戏安装目录 | `…/AzurLane/…` |
| AssetBundle | `*.ab`, `*.unity3d`, `*.bytes` |

| 输出 | 路径 |
|------|------|
| Spine 三件套 | `data/live2d/<slug>/*.skel|.atlas|.png` |
| 导入计划 | `data/import/live2d-plan.json` |
| BWIKI 缓存 | `data/wiki/blhx.sqlite` |

slug 规则对齐 `mcp/blhx-wiki/src/live2d.ts`（拼音、`_2` 皮肤后缀）。

## 非目标（v1）

- Cubism `.moc3`（游戏「Live2D」实为 Spine）
- CRI 音频 `.acb` / `.awb`
- 图形界面
- 嵌入 hanpet 主程序

## 目标代码布局

```
hanimport/
├── Cargo.toml
├── src/
│   ├── main.rs
│   ├── cli.rs
│   └── unpack/          # AssetBundle → data/live2d
└── docs/

crates/（项目级，后续）
├── hanimport-core/      # 解包核心
└── handaily-spine-pack/ # 与 hanpet 共享的 Spine 工具
```

## 对接关系

| 组件 | 位置 | 关系 |
|------|------|------|
| hanpet | `hanpet/` | 消费导入结果 |
| BWIKI MCP | `mcp/blhx-wiki/` | 扫描 `data/live2d` |
| 工作数据 | `data/` | 解包与计划输出 |
| 现有 CLI | `hanpet/src-tauri/src/bin/` | 过渡期保留，逐步迁入 hanimport |

## 验收（v1 unpack）

- [x] CLI 骨架：`hanimport unpack --input --output --dry-run`
- [ ] 解包 1 个已知舰娘 bundle 到 `data/live2d/<slug>/`
- [ ] `blhx_scan_live2d` 可识别
- [ ] `live2d_import --dry-run` 无结构错误
