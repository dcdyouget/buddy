# S06-02 Provider 卡片与新增流程

> 状态: `todo`
> Phase: 06
> 依赖: S06-01, S02-01
> 阻塞: —
> 退役设计文档: —（相关设置段落随 Phase 06 全部实现后统一核对部分退役）

## 目标

移植 Provider 预设选择、连接参数、拉取与选择模型、测速及添加后的配置持久化。

## 输入

- `src/components/settings/AddProviderPanel.tsx`
- `src/components/settings/ProviderCard.tsx`
- `src/stores/configStore.ts`
- `crates/engine/src/models/config.rs`

## 实现要点

| ID | 事项 | 说明 |
|----|------|------|
| S06-02-1 | 实现约束 | 以 v1 当前流程为准拆分小模块；engine 方法经 spawn_engine 执行。 |
| S06-02-2 | 实现约束 | URL / Key 校验与连接错误按 v1 文案显示；秘密仅写入配置 JSON。 |
| S06-02-3 | 实现约束 | 添加过程不能用中间配置切离设置页，成功后返回对话；失败保留输入。 |

## 验收标准

- [ ] 真实输入与点击覆盖预设、连接参数、拉取有效 / 无效响应、模型选择及提交。
- [ ] 沙盒真实 engine 配置往返、明文 Key 与异常边界测试及拦截验证通过。
- [ ] 本地读取真实渲染核对侧滑面板与 Provider 卡片。

## 证据

| 项 | 证据 |
|----|------|

## 决策记录

| 决策 | 选择 | 理由 |
|------|------|------|

## 完成记录

- 日期：
- commit：
- 设计文档处置：
