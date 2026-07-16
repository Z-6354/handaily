# hanimport 批量解包 Job + 写入后生成 JSON — 设计

**日期**: 2026-07-15  
**状态**: 已实现  
**执行顺序**: Phase 1（本规格）→ Phase 2（角色库可视化，见 `2026-07-15-hanimport-roster-browser-design.md`）

## 目标

增强现有解包网页（`/`）：

1. **游戏源文件批量解包**（目录递归 AssetBundle），支持勾选子集
2. **写入后生成 JSON**（默认开启）：解包落盘后对输出目录跑配置生成
3. **进度条**：异步 Job + 轮询，避免一次 HTTP 卡到全部结束

## 流水线

```
扫描 / 勾选 → Job(解包 N) → Job(生成配置 M) → done
```

- 解包输出：Live2D Spine → `data/live2d/<slug>`；Cubism → `data/model/unpacked/<slug>`（沿用现有 `resolve_output`）
- 生成配置：
  - Spine：`build_model_config.build_folder_configs`
  - Cubism：`build_cubism_config`（用解包结果里的 bundle 路径 + 输出目录）
- 勾选「写入后生成 JSON」关闭时，仅解包；仍可手动「生成 JSON」对输出根目录跑配置 Job

## 进度 API

| 方法 | 路径 | 说明 |
|------|------|------|
| POST | `/api/jobs/unpack` | body: `input`, `output?`, `slugs?`, `dry_run`, `continue_on_error`, `generate_config` → `{ job_id }` |
| POST | `/api/jobs/config` | body: `input`（输出根或解包结果路径）, `force`, `dry_run` → `{ job_id }` |
| GET | `/api/jobs/{id}` | 状态快照 |

**Job 快照字段**

```json
{
  "id": "...",
  "kind": "unpack|config|unpack_then_config",
  "status": "queued|running|done|error",
  "phase": "unpack|config|",
  "current": 3,
  "total": 40,
  "current_item": "aidang",
  "ok_count": 2,
  "fail_count": 1,
  "log_tail": ["..."],
  "error": null,
  "results": []
}
```

实现：进程内 `dict` + 后台 `threading.Thread`（stdlib，与 `ThreadingHTTPServer` 一致）。不引入 Redis。

## UI（`/`）

- 扫描结果 checkbox（默认全选）
- 勾选：「写入后生成 JSON」「遇错继续」
- 进度条：`current/total` + 百分比 + `phase` + 当前 slug
- 日志区仍追加；按钮在 Job `running` 时禁用

## 非目标

- SSE/WebSocket（轮询足够）
- 同步到 roster / AppData（属 Phase 2 运维）
- 改解包内核算法

## 验收

1. 选目录扫描 → 勾选部分 → 解包 Job 进度平滑更新
2. 默认勾选生成 JSON：解包完成后自动进入配置阶段并出 JSON
3. dry-run 不写盘，进度仍可走完
4. 遇错即停 / 遇错继续 行为符合勾选
5. 现有「生成 JSON」按钮改为走 config Job + 进度条
