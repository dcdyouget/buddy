# Buddy — Project Instructions (Claude Code)

> ⚠️ **本文件不是权威来源，只是一层薄指针。**
>
> 权威入口是 **[`AGENTS.md`](./AGENTS.md)**。本文件曾与 `AGENTS.md` 各自维护一份 Document Index，已发生漂移（曾引用不存在的 `docs/design/prototypes/` 等路径）。为避免再次分叉，此处不再复制任何内容。

## 必读

按顺序读：

1. **[`AGENTS.md`](./AGENTS.md)** — 项目概览、**Document Index**、**Hard Constraints（1-10）**、文件组织
2. **[`docs/CONVENTIONS.md`](./docs/CONVENTIONS.md)** — 编码规则（所有 agent 必须遵守）

## 若参与 GPUI 重构（v2.0.0-gpui）

1. **[`docs/specs/RULES.md`](./docs/specs/RULES.md)** — spec 工作规则，含**设计文档退役规则**（实现完即删设计文档）
2. **[`docs/specs/README.md`](./docs/specs/README.md)** — spec 注册表与进度（**唯一权威状态源**）
3. **[`docs/tasks/v2.0.0-gpui/research-log.md`](./docs/tasks/v2.0.0-gpui/research-log.md)** — 调研证据与风险登记
4. **[`docs/specs/phase-00/`](./docs/specs/phase-00/)** — 当前要执行的 Go/No-Go Spike

## 维护规则

- **不得**在本文件添加任何 `AGENTS.md` 已有的内容
- 需要新增全局规则时，改 `AGENTS.md`；本文件保持为指针
- 规则来源见 `docs/specs/RULES.md` §11.4（禁止双入口文档）
