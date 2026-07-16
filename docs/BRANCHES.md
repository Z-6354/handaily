# 三分支架构总览

> 顶层设计在 `main`：[questions/144-日报改造与三分支架构设计-20260709.md](questions/144-日报改造与三分支架构设计-20260709.md)

| 分支 | 定位 | 状态 |
|------|------|------|
| **main** | 全功能单端 | 日报改造已落地 |
| **feat/live2d-only** | 纯桌宠 | P0–P1 完成；模块物理删除待办 |
| **feat/client-server-split** | 端云分离 | ingest 骨架 |

```bash
git checkout feat/live2d-only && npm run tauri:dev
```

详见 [BRANCH-live2d-only.md](BRANCH-live2d-only.md)。
