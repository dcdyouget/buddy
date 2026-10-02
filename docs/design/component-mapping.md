# Component Mapping — 窗口外壳

> 聊天 / 空态 / 无 Key 页的组件映射已由 Phase 05 实现，相关段落已退役；理由与行为以 `crates/ui/src/chat/` 模块文档和对应 spec 决策记录为准。
> 设置组件由 Phase 06 的代码与对应 spec 替代（`crates/ui/src/settings/`）；热键录制、平台键帽和主题选择的理由见 S06-05 / S06-06 决策记录。本文件仅保留窗口外壳（Phase 07）。v1 参考代码仍保留，以 `src/components/` 当前实现为准。

## Remaining Component Roles

| Component | Role |
|---|---|
| `GlassPanel` | 窗口面板容器；实色与无装饰约束见 `AGENTS.md` |

## Remaining Files

```text
src/components/shared/GlassPanel.tsx
```
