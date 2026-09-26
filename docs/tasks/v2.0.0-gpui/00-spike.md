# Phase 00: 可行性 Spike

> **本文件是背景与拆解依据，不承载状态、不定义编号。**
> 执行细节与状态见 `docs/specs/phase-00/`；进度见 `docs/specs/README.md`。

## 目标

**证伪优先。** 在投入任何实质迁移工作前，用最小代价确认「GPUI 能否承载 Buddy 这个产品形态」。

## 相关文档

- `docs/specs/RULES.md` — spec 工作规则（**执行必读**）
- `docs/tasks/v2.0.0-gpui/research-log.md` — 全部结论的证据基础与风险登记 R1-R9
- `AGENTS.md` — 硬约束 1-10

## 总原则

1. **按风险降序执行**，不按依赖降序
2. 每个 Spike 独立可弃，产物放 `spikes/` 目录，**不污染主工程**
3. **S00-02 或 S00-03 任一失败 → 直接终止整个 v2.0.0-gpui，回退 Tauri**
4. 结论记录写在对应 spec 文件内，不在此重复

## Spike 清单

> **证伪条件写在各自的 spec 文件内，本表不重复**（单一真相源，见 `RULES.md` §11.2）。

| Spec | 名称 | 风险 |
|------|------|------|
| [S00-01](../specs/phase-00/S00-01-extract-theme-ui.md) | 抽取 zed theme + ui 并编译 | 低 |
| [S00-02](../specs/phase-00/S00-02-floating-panel.md) | **Buddy 形态浮动面板（macOS）** | **高** |
| [S00-03](../specs/phase-00/S00-03-hotkey-tray-autostart.md) | **全局热键 + tray + autostart 集成** | **高** |
| [S00-04](../specs/phase-00/S00-04-backdrop.md) | 毛玻璃双路径验证 | 中高 |
| [S00-05](../specs/phase-00/S00-05-ime.md) | 中文 IME 验证 | 中 |
| [S00-06](../specs/phase-00/S00-06-markdown.md) | markdown vendor + patch + 流式验证 | 中 |
| [S00-07](../specs/phase-00/S00-07-list.md) | 大列表虚拟化验证 | 中 |
| [S00-08](../specs/phase-00/S00-08-engine-loop.md) | 引擎层最小闭环 | 低 |
| [S00-09](../specs/phase-00/S00-09-decision.md) | 结论汇总与 Go/No-Go 决策 | — |

## 硬性门槛

以下两个 spec **必须通过**，否则整体 No-Go（具体门槛见各 spec 的「证伪条件」）：

- **S00-02** —— 窗口外壳
- **S00-03** —— 全局热键 / tray / autostart

## 结论

结论表、风险复评与 Go/No-Go 决策**全部记录在 [S00-09](../specs/phase-00/S00-09-decision.md) 内**，本文件不再重复维护（单一真相源规则，见 `RULES.md` §11.2）。
